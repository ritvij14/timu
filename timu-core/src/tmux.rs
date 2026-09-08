//! tmux session engine (PRD §11): create/reuse sessions, drive agents, read
//! pane output — all through [`SshTransport::run_command`].
//!
//! Pure command builders live here like `readiness_probe.rs`; the async
//! flows parameterize over the transport so `FakeSshTransport` tests the full
//! path. tmux is the backend, chat is the frontend; this module is the wire.

use crate::error::TimuError;
use crate::folder::shell_quote;
use crate::ssh::SshTransport;

/// Deterministic tmux session id for a project folder: the sanitized folder
/// basename plus a short hash of the full path, so `/a/app` and `/b/app` get
/// distinct sessions while session cards still read like the project name.
pub fn build_session_id(folder: &str) -> String {
    let basename = folder
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(folder);
    let sanitized: String = basename
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in folder.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{sanitized}-{:08x}", (hash & 0xFFFF_FFFF) as u32)
}

pub fn build_has_session_command(session_id: &str) -> String {
    format!("tmux has-session -t {}", shell_quote(session_id))
}

pub fn build_new_session_command(session_id: &str, folder: &str) -> String {
    format!(
        "tmux new-session -d -s {} -c {}",
        shell_quote(session_id),
        shell_quote(folder)
    )
}

/// Type a chat message into the session, then press Enter. `-l` keeps tmux
/// from interpreting the text as keys.
pub fn build_send_message_commands(session_id: &str, text: &str) -> Vec<String> {
    vec![
        format!(
            "tmux send-keys -t {} -l {}",
            shell_quote(session_id),
            shell_quote(text)
        ),
        format!("tmux send-keys -t {} Enter", shell_quote(session_id)),
    ]
}

pub fn build_capture_pane_command(session_id: &str) -> String {
    format!("tmux capture-pane -t {} -p", shell_quote(session_id))
}

pub fn build_list_sessions_command() -> String {
    "tmux list-sessions -F '#S'".to_string()
}

pub fn build_kill_session_command(session_id: &str) -> String {
    format!("tmux kill-session -t {}", shell_quote(session_id))
}

/// Parse `tmux list-sessions -F '#S'` output. Exit 1 with "no server running"
/// means there are no sessions, not an error.
pub fn parse_session_list(output: &crate::ssh::CommandOutput) -> Result<Vec<String>, TimuError> {
    if output.exit_code == 0 {
        Ok(output
            .stdout
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect())
    } else if output.stderr.contains("no server running") {
        Ok(Vec::new())
    } else {
        Err(map_tmux_error(output))
    }
}

/// Result of [`start_agent_session`]. `reused` tells the UI whether the
/// tmux session (and therefore any running agent) already existed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartedSession {
    pub session_id: String,
    pub reused: bool,
}

pub(crate) fn map_tmux_error(output: &crate::ssh::CommandOutput) -> TimuError {
    let stderr = output.stderr.trim();
    if stderr.contains("not found") || stderr.contains("command not found") {
        TimuError::TmuxMissing
    } else {
        TimuError::Other(if stderr.is_empty() {
            format!("tmux command failed with exit code {}", output.exit_code)
        } else {
            stderr.to_string()
        })
    }
}

async fn run_ok<T: SshTransport>(
    transport: &T,
    command: &str,
) -> Result<crate::ssh::CommandOutput, TimuError> {
    let output = transport.run_command(command).await?;
    if output.exit_code != 0 {
        return Err(map_tmux_error(&output));
    }
    Ok(output)
}

/// PRD §11 — create-or-reuse a tmux session for `folder` and launch
/// `agent_command` inside it. The session id is deterministic per folder, so
/// starting the same project twice resumes the same session.
pub async fn start_agent_session<T: SshTransport>(
    transport: &T,
    folder: &str,
    agent_command: &str,
) -> Result<StartedSession, TimuError> {
    let session_id = build_session_id(folder);
    let has = build_has_session_command(&session_id);
    let existing = transport.run_command(&has).await?;
    if existing.stderr.contains("not found") {
        return Err(TimuError::TmuxMissing);
    }
    let reused = existing.exit_code == 0;
    if !reused {
        run_ok(transport, &build_new_session_command(&session_id, folder)).await?;
    }
    for command in build_send_message_commands(&session_id, agent_command) {
        run_ok(transport, &command).await?;
    }
    Ok(StartedSession { session_id, reused })
}

/// Type a chat message into the agent's pane and press Enter (PRD §12).
pub async fn send_chat_message<T: SshTransport>(
    transport: &T,
    session_id: &str,
    text: &str,
) -> Result<(), TimuError> {
    for command in build_send_message_commands(session_id, text) {
        run_ok(transport, &command).await?;
    }
    Ok(())
}

/// Read the pane's visible content — the chat UI's view of the agent.
pub async fn capture_pane<T: SshTransport>(
    transport: &T,
    session_id: &str,
) -> Result<String, TimuError> {
    Ok(run_ok(transport, &build_capture_pane_command(session_id))
        .await?
        .stdout)
}

pub async fn list_tmux_sessions<T: SshTransport>(transport: &T) -> Result<Vec<String>, TimuError> {
    let output = transport
        .run_command(&build_list_sessions_command())
        .await?;
    parse_session_list(&output)
}

pub async fn kill_session<T: SshTransport>(
    transport: &T,
    session_id: &str,
) -> Result<(), TimuError> {
    run_ok(transport, &build_kill_session_command(session_id)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssh::{CommandOutput, FakeSshTransport};

    #[test]
    fn session_id_is_sanitized_basename_plus_short_path_hash() {
        let id = build_session_id("/home/dev/kendal-crm");
        assert!(id.starts_with("kendal-crm-"), "got {id}");
        assert_eq!(id.len(), "kendal-crm-".len() + 8);
        assert!(
            id["kendal-crm-".len()..]
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        );
    }

    #[test]
    fn session_id_is_deterministic_and_path_sensitive() {
        let a = build_session_id("/home/dev/kendal-crm");
        let b = build_session_id("/home/dev/kendal-crm");
        let c = build_session_id("/home/other/kendal-crm");
        assert_eq!(a, b);
        assert_ne!(a, c, "same basename in different folders must not collide");
    }

    #[test]
    fn session_id_sanitizes_unsafe_characters() {
        let id = build_session_id("/home/dev/My.App:2024");
        assert!(id.starts_with("My-App-2024-"), "got {id}");
        assert!(!id.contains(':'));
        assert!(!id.contains('.'));
    }

    #[test]
    fn new_session_command_creates_detached_in_folder() {
        let cmd = build_new_session_command("sess-1", "/home/dev/app");
        assert_eq!(cmd, "tmux new-session -d -s 'sess-1' -c '/home/dev/app'");
    }

    #[test]
    fn has_session_command_targets_the_session() {
        assert_eq!(
            build_has_session_command("sess-1"),
            "tmux has-session -t 'sess-1'"
        );
    }

    #[test]
    fn send_message_commands_are_literal_and_shell_quoted() {
        let cmds = build_send_message_commands("sess-1", "fix the bug && rm -rf /");
        assert_eq!(cmds.len(), 2);
        assert_eq!(
            cmds[0],
            "tmux send-keys -t 'sess-1' -l 'fix the bug && rm -rf /'"
        );
        assert_eq!(cmds[1], "tmux send-keys -t 'sess-1' Enter");
    }

    #[test]
    fn send_message_commands_single_quote_proof_the_text() {
        let cmds = build_send_message_commands("sess-1", "it's alive");
        // Embedded single quote must not break out of the quoting.
        assert_eq!(cmds[0], "tmux send-keys -t 'sess-1' -l 'it'\\''s alive'");
    }

    #[test]
    fn capture_pane_command_prints_the_pane() {
        assert_eq!(
            build_capture_pane_command("sess-1"),
            "tmux capture-pane -t 'sess-1' -p"
        );
    }

    #[test]
    fn list_sessions_command_prints_only_names() {
        assert_eq!(build_list_sessions_command(), "tmux list-sessions -F '#S'");
    }

    #[test]
    fn kill_session_command_targets_the_session() {
        assert_eq!(
            build_kill_session_command("sess-1"),
            "tmux kill-session -t 'sess-1'"
        );
    }

    #[test]
    fn list_sessions_parses_one_name_per_line() {
        let out = CommandOutput::success("kendal-crm\nwebsite\n");
        assert_eq!(
            parse_session_list(&out).unwrap(),
            vec!["kendal-crm".to_string(), "website".to_string()]
        );
    }

    #[test]
    fn list_sessions_on_server_without_tmux_server_is_empty() {
        // `tmux list-sessions` with no server: exit 1, "no server running on ..."
        let out = CommandOutput::new(String::new(), "no server running on /tmp/tmux-0/default", 1);
        assert_eq!(parse_session_list(&out).unwrap(), Vec::<String>::new());
    }

    // --- flows over the transport seam ---

    fn missing_tmux_output() -> CommandOutput {
        CommandOutput::new(String::new(), "bash: line 1: tmux: command not found", 127)
    }

    #[tokio::test]
    async fn start_creates_a_detached_session_and_launches_the_agent() {
        let mut fake = FakeSshTransport::new();
        let id = build_session_id("/home/dev/kendal-crm");
        fake.script(
            build_has_session_command(&id),
            CommandOutput::new(String::new(), String::new(), 1),
        );
        fake.script_success(
            build_new_session_command(&id, "/home/dev/kendal-crm"),
            String::new(),
        );
        for cmd in build_send_message_commands(&id, "codex") {
            fake.script_success(&cmd, String::new());
        }

        let started = start_agent_session(&fake, "/home/dev/kendal-crm", "codex")
            .await
            .expect("session starts");
        assert_eq!(started.session_id, id);
        assert!(!started.reused);
    }

    #[tokio::test]
    async fn start_reuses_an_existing_session_without_recreating_it() {
        let mut fake = FakeSshTransport::new();
        let id = build_session_id("/home/dev/kendal-crm");
        fake.script_success(build_has_session_command(&id), String::new());
        for cmd in build_send_message_commands(&id, "codex") {
            fake.script_success(&cmd, String::new());
        }
        // new-session is deliberately unscripted: calling it fails the test.

        let started = start_agent_session(&fake, "/home/dev/kendal-crm", "codex")
            .await
            .expect("session reuses");
        assert!(started.reused);
        assert_eq!(started.session_id, id);
    }

    #[tokio::test]
    async fn start_maps_missing_tmux_to_the_typed_error() {
        let mut fake = FakeSshTransport::new();
        let id = build_session_id("/home/dev/kendal-crm");
        fake.script(build_has_session_command(&id), missing_tmux_output());

        let err = start_agent_session(&fake, "/home/dev/kendal-crm", "codex")
            .await
            .expect_err("tmux missing must fail");
        assert_eq!(err.code(), "tmux_missing");
    }

    #[tokio::test]
    async fn start_surfaces_tmux_creation_failures() {
        let mut fake = FakeSshTransport::new();
        let id = build_session_id("/home/dev/kendal-crm");
        fake.script(
            build_has_session_command(&id),
            CommandOutput::new(String::new(), String::new(), 1),
        );
        fake.script(
            build_new_session_command(&id, "/home/dev/kendal-crm"),
            CommandOutput::new(String::new(), "duplicate session: kendal-crm", 1),
        );

        let err = start_agent_session(&fake, "/home/dev/kendal-crm", "codex")
            .await
            .expect_err("creation failure must surface");
        assert_eq!(err.code(), "other");
        assert!(err.to_string().contains("duplicate session"));
    }

    #[tokio::test]
    async fn send_message_types_the_text_then_presses_enter() {
        let mut fake = FakeSshTransport::new();
        for cmd in build_send_message_commands("sess-1", "run the tests") {
            fake.script_success(&cmd, String::new());
        }

        send_chat_message(&fake, "sess-1", "run the tests")
            .await
            .expect("message sent");
    }

    #[tokio::test]
    async fn capture_pane_returns_the_pane_text() {
        let mut fake = FakeSshTransport::new();
        fake.script_success(build_capture_pane_command("sess-1"), "agent: ready\n");

        let pane = capture_pane(&fake, "sess-1").await.expect("pane captured");
        assert_eq!(pane, "agent: ready\n");
    }

    #[tokio::test]
    async fn kill_session_targets_the_session() {
        let mut fake = FakeSshTransport::new();
        fake.script_success(build_kill_session_command("sess-1"), String::new());

        kill_session(&fake, "sess-1").await.expect("session killed");
    }

    #[tokio::test]
    async fn list_sessions_flows_through_the_transport() {
        let mut fake = FakeSshTransport::new();
        fake.script_success(build_list_sessions_command(), "kendal-crm\nwebsite\n");

        let names = list_tmux_sessions(&fake).await.expect("sessions listed");
        assert_eq!(names, vec!["kendal-crm".to_string(), "website".to_string()]);
    }
}
