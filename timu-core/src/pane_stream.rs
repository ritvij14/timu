//! Live pane streaming (PRD §12): translate tmux pane snapshots into an event
//! stream, tested over [`crate::FakeSshTransport`]. Design:
//! `docs/features/timu-core.md` §6 — snapshot-diff polling over the existing
//! [`SshTransport`] seam; attach-is-catch-up; bounded scrollback on misalign.

use crate::error::TimuError;
use crate::folder::shell_quote;
use crate::ssh::{CommandOutput, SshTransport};
use crate::tmux::{build_capture_pane_command, map_tmux_error};

/// How far back the bounded catch-up capture reaches when the visible pane
/// cannot be aligned against the accumulated buffer (output scrolled faster
/// than the poll interval).
pub const CATCH_UP_SCROLLBACK_LINES: usize = 500;

/// Streaming events for one tmux pane. Kept minimal by design — see the
/// feature doc for what is deliberately deferred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneEvent {
    /// Replace the UI's view with this full pane text (attach, reconnect, or
    /// in-place redraw catch-up). Chunking into chat bubbles is a UI concern.
    History(String),
    /// Lines newly appended to the pane since the last tick.
    OutputAppended(String),
    /// tmux reports the session no longer exists. Terminal for the watcher.
    SessionEnded,
}

pub fn build_capture_pane_full_command(session_id: &str) -> String {
    format!("tmux capture-pane -t {} -p -S -", shell_quote(session_id))
}

pub fn build_capture_pane_window_command(session_id: &str, lines: usize) -> String {
    format!(
        "tmux capture-pane -t {} -p -S -{}",
        shell_quote(session_id),
        lines
    )
}

/// Number of trailing lines of `next` that are new, assuming the rest already
/// sits at the tail of `accumulated`. `None` means the pane cannot be aligned
/// (in-place redraw or faster-than-poll scrolling) and the caller must fall
/// back to a bounded catch-up capture.
pub fn count_new_lines(accumulated: &[String], next: &[String]) -> Option<usize> {
    if next.is_empty() {
        return Some(0);
    }
    if accumulated.is_empty() {
        return Some(next.len());
    }
    for skipped in 0..next.len() {
        let overlap = next.len() - skipped;
        if accumulated.len() >= overlap
            && accumulated[accumulated.len() - overlap..] == next[..overlap]
        {
            return Some(skipped);
        }
    }
    None
}

/// Watches one tmux session's pane and turns captures into [`PaneEvent`]s.
pub struct PaneWatcher<T: SshTransport> {
    transport: T,
    session_id: String,
    accumulated: Vec<String>,
}

impl<T: SshTransport> PaneWatcher<T> {
    pub fn new(transport: T, session_id: impl Into<String>) -> Self {
        Self {
            transport,
            session_id: session_id.into(),
            accumulated: Vec::new(),
        }
    }

    /// Capture the full pane (visible + scrollback) and emit it as
    /// [`PaneEvent::History`]. Attach is catch-up: reconnecting after the app
    /// was backgrounded is just attaching again (PRD §13.1). A missing session
    /// yields [`PaneEvent::SessionEnded`].
    pub async fn attach(&mut self) -> Result<PaneEvent, TimuError> {
        let output = self
            .transport
            .run_command(&build_capture_pane_full_command(&self.session_id))
            .await?;
        if session_gone(&output) {
            return Ok(PaneEvent::SessionEnded);
        }
        if output.exit_code != 0 {
            return Err(map_tmux_error(&output));
        }
        self.accumulated = pane_lines(&output.stdout);
        Ok(PaneEvent::History(output.stdout))
    }

    /// One poll step: capture the visible pane and diff it against the
    /// accumulated buffer. `Ok(None)` = nothing changed.
    pub async fn tick(&mut self) -> Result<Option<PaneEvent>, TimuError> {
        let output = self
            .transport
            .run_command(&build_capture_pane_command(&self.session_id))
            .await?;
        if session_gone(&output) {
            return Ok(Some(PaneEvent::SessionEnded));
        }
        if output.exit_code != 0 {
            return Err(map_tmux_error(&output));
        }
        let next = pane_lines(&output.stdout);
        match count_new_lines(&self.accumulated, &next) {
            Some(0) => Ok(None),
            Some(skipped) => {
                let appended: Vec<String> = next[next.len() - skipped..].to_vec();
                self.accumulated.extend(appended.iter().cloned());
                Ok(Some(PaneEvent::OutputAppended(appended.join("\n"))))
            }
            None => {
                // In-place redraw or fast scroll: bounded catch-up capture
                // replaces the whole view so no lines are ever lost.
                let window = self
                    .transport
                    .run_command(&build_capture_pane_window_command(
                        &self.session_id,
                        CATCH_UP_SCROLLBACK_LINES,
                    ))
                    .await?;
                if session_gone(&window) {
                    return Ok(Some(PaneEvent::SessionEnded));
                }
                if window.exit_code != 0 {
                    return Err(map_tmux_error(&window));
                }
                self.accumulated = pane_lines(&window.stdout);
                Ok(Some(PaneEvent::History(window.stdout)))
            }
        }
    }

    /// Drive attach + ticks into `sender` forever (until [`PaneEvent::SessionEnded`],
    /// a transport error, or the subscriber going away). Spawn on a tokio task.
    pub async fn run(
        mut self,
        interval: std::time::Duration,
        sender: tokio::sync::mpsc::Sender<PaneEvent>,
    ) -> Result<(), TimuError> {
        let first = self.attach().await?;
        let ended = first == PaneEvent::SessionEnded;
        if sender.send(first).await.is_err() {
            return Ok(());
        }
        if ended {
            return Ok(());
        }
        // Attach's full capture already contains the visible pane, so skip
        // the interval's immediate first tick and poll from here on.
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        ticker.tick().await;
        loop {
            ticker.tick().await;
            let Some(event) = self.tick().await? else {
                continue;
            };
            let ended = event == PaneEvent::SessionEnded;
            if sender.send(event).await.is_err() {
                return Ok(());
            }
            if ended {
                return Ok(());
            }
        }
    }
}

fn pane_lines(stdout: &str) -> Vec<String> {
    stdout.lines().map(str::to_string).collect()
}

fn session_gone(output: &CommandOutput) -> bool {
    output.exit_code != 0 && output.stderr.contains("can't find session")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssh::FakeSshTransport;
    use std::time::Duration;

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // --- pure diff ---

    #[test]
    fn diff_counts_new_lines_when_the_pane_grows_without_scrolling() {
        let acc = lines(&["a", "b"]);
        assert_eq!(count_new_lines(&acc, &lines(&["a", "b", "c"])), Some(1));
        assert_eq!(
            count_new_lines(&acc, &lines(&["a", "b", "c", "d"])),
            Some(2)
        );
    }

    #[test]
    fn diff_counts_scrolled_lines_when_the_pane_slides() {
        let acc = lines(&["a", "b"]);
        assert_eq!(count_new_lines(&acc, &lines(&["b", "c"])), Some(1));
    }

    #[test]
    fn diff_counts_nothing_when_the_pane_is_unchanged() {
        let acc = lines(&["a", "b"]);
        assert_eq!(count_new_lines(&acc, &lines(&["a", "b"])), Some(0));
    }

    #[test]
    fn diff_counts_everything_as_new_into_an_empty_accumulator() {
        assert_eq!(count_new_lines(&[], &lines(&["a", "b"])), Some(2));
    }

    #[test]
    fn diff_reports_misalignment_when_the_pane_is_redrawn_in_place() {
        // Same shape, different content: e.g. a spinner frame or a cleared screen.
        let acc = lines(&["prompt", "working..."]);
        assert_eq!(count_new_lines(&acc, &lines(&["prompt", "done!"])), None);
        assert_eq!(count_new_lines(&acc, &lines(&["fresh"])), None);
    }

    #[test]
    fn diff_treats_an_empty_pane_as_no_change() {
        let acc = lines(&["a"]);
        assert_eq!(count_new_lines(&acc, &[]), Some(0));
    }

    // --- command builders ---

    #[test]
    fn full_capture_command_requests_all_scrollback() {
        assert_eq!(
            build_capture_pane_full_command("sess-1"),
            "tmux capture-pane -t 'sess-1' -p -S -"
        );
    }

    #[test]
    fn window_capture_command_bounds_the_scrollback() {
        assert_eq!(
            build_capture_pane_window_command("sess-1", 500),
            "tmux capture-pane -t 'sess-1' -p -S -500"
        );
    }

    // --- flows over the transport seam ---

    #[tokio::test]
    async fn attach_emits_the_full_pane_as_history() {
        let mut fake = FakeSshTransport::new();
        let id = "sess-1";
        fake.script_success(
            build_capture_pane_full_command(id),
            "earlier reply\nagent: working\n",
        );

        let mut watcher = PaneWatcher::new(fake, id);
        let event = watcher.attach().await.expect("attach");
        assert_eq!(
            event,
            PaneEvent::History("earlier reply\nagent: working\n".into())
        );
    }

    #[tokio::test]
    async fn attach_on_a_missing_session_ends_the_watcher() {
        let mut fake = FakeSshTransport::new();
        let id = "sess-1";
        fake.script(
            build_capture_pane_full_command(id),
            CommandOutput::new(String::new(), "can't find session: sess-1", 1),
        );

        let mut watcher = PaneWatcher::new(fake, id);
        assert_eq!(
            watcher.attach().await.expect("attach"),
            PaneEvent::SessionEnded
        );
    }

    #[tokio::test]
    async fn attach_surfaces_transport_errors() {
        let mut fake = FakeSshTransport::new();
        let id = "sess-1";
        fake.script_error(
            build_capture_pane_full_command(id),
            crate::error::TimuError::PortUnreachable,
        );

        let mut watcher = PaneWatcher::new(fake, id);
        let err = watcher.attach().await.expect_err("must fail");
        assert_eq!(err.code(), "port_unreachable");
    }

    #[tokio::test]
    async fn tick_emits_nothing_when_the_pane_is_unchanged() {
        let mut fake = FakeSshTransport::new();
        let id = "sess-1";
        fake.script_success(build_capture_pane_full_command(id), "a\nb\n");
        fake.script_success(build_capture_pane_command(id), "a\nb\n");

        let mut watcher = PaneWatcher::new(fake, id);
        watcher.attach().await.expect("attach");
        assert_eq!(watcher.tick().await.expect("tick"), None);
    }

    #[tokio::test]
    async fn tick_emits_appended_lines_and_accumulates_them() {
        let mut fake = FakeSshTransport::new();
        let id = "sess-1";
        fake.script_success(build_capture_pane_full_command(id), "a\nb\n");
        fake.script_sequence(
            build_capture_pane_command(id),
            vec![
                CommandOutput::success("a\nb\nc\nd\n"),
                CommandOutput::success("a\nb\nc\nd\n"),
            ],
        );

        let mut watcher = PaneWatcher::new(fake, id);
        watcher.attach().await.expect("attach");
        assert_eq!(
            watcher.tick().await.expect("tick"),
            Some(PaneEvent::OutputAppended("c\nd".into()))
        );

        // The next identical pane must now be a no-change tick.
        assert_eq!(watcher.tick().await.expect("tick"), None);
    }

    #[tokio::test]
    async fn tick_falls_back_to_a_bounded_history_when_the_pane_cannot_be_aligned() {
        let mut fake = FakeSshTransport::new();
        let id = "sess-1";
        fake.script_success(build_capture_pane_full_command(id), "a\nb\n");
        // In-place redraw: same shape, different content.
        fake.script(
            build_capture_pane_command(id),
            CommandOutput::success("x\ny\n"),
        );
        fake.script_success(
            build_capture_pane_window_command(id, CATCH_UP_SCROLLBACK_LINES),
            "older\nx\ny\n",
        );

        let mut watcher = PaneWatcher::new(fake, id);
        watcher.attach().await.expect("attach");
        assert_eq!(
            watcher.tick().await.expect("tick"),
            Some(PaneEvent::History("older\nx\ny\n".into()))
        );
    }

    #[tokio::test]
    async fn tick_ends_the_watcher_when_the_session_disappears() {
        let mut fake = FakeSshTransport::new();
        let id = "sess-1";
        fake.script_success(build_capture_pane_full_command(id), "a\n");
        fake.script(
            build_capture_pane_command(id),
            CommandOutput::new(String::new(), "can't find session: sess-1", 1),
        );

        let mut watcher = PaneWatcher::new(fake, id);
        watcher.attach().await.expect("attach");
        assert_eq!(
            watcher.tick().await.expect("tick"),
            Some(PaneEvent::SessionEnded)
        );
    }

    #[tokio::test]
    async fn run_streams_history_then_appends_until_the_session_ends() {
        let mut fake = FakeSshTransport::new();
        let id = "sess-1";
        fake.script_success(build_capture_pane_full_command(id), "a\n");
        fake.script_sequence(
            build_capture_pane_command(id),
            vec![
                CommandOutput::success("a\n"),
                CommandOutput::success("a\nb\n"),
                CommandOutput::new(String::new(), "can't find session: sess-1", 1),
            ],
        );

        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        let handle = tokio::spawn(PaneWatcher::new(fake, id).run(Duration::from_millis(1), tx));

        let mut events = Vec::new();
        while let Some(event) = rx.recv().await {
            let done = event == PaneEvent::SessionEnded;
            events.push(event);
            if done {
                break;
            }
        }
        assert_eq!(
            events,
            vec![
                PaneEvent::History("a\n".into()),
                PaneEvent::OutputAppended("b".into()),
                PaneEvent::SessionEnded,
            ]
        );
        handle.await.expect("run task").expect("run ok");
    }
}
