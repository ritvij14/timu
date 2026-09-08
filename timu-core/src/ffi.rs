//! UniFFI bridge (feature `ffi`) — the surface the Expo app drives.
//!
//! Proc-macro mode: exported objects ([`FfiCore`], [`Connection`],
//! [`PaneStreamHandle`]), the [`PaneEventSink`] callback interface, and
//! uniffi-compatible derives on the core data types. Async methods are safe to
//! call from any foreign thread: each body hops to a timu-core-owned tokio
//! runtime via [`runtime_handle`] so russh and tokio timers always run inside
//! a real runtime context (the caller's thread has none).
//!
//! FFI needs concrete types (no generics across the bridge), so
//! [`BridgeTransport`] dispatches to the real russh transport — or the fake,
//! in tests. Session/stream flows delegate to the same core functions the core
//! tests exercise; this layer adds only type conversion and task wiring.

use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::RusshSshTransport;
use crate::connection::ConnectionTestResult;
use crate::credentials::Credentials;
use crate::error::TimuError;
use crate::pane_stream::{PaneEvent, PaneWatcher};
use crate::profile::MachineProfile;
use crate::readiness::ReadinessReport;
use crate::readiness_probe::run_readiness_probe;
use crate::ssh::SshTransport;
use crate::timu_core::TimuCore;
use crate::tmux::{self, StartedSession};

/// The app-facing outcome of a PRD §6 connection test. `fingerprint` is the
/// newly-pinned host-key fingerprint on first connect (shown to the user);
/// `None` when it matched an existing pin.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum ConnectionTestOutcome {
    Connected { fingerprint: Option<String> },
    Failed { error: TimuError },
}

/// Map the core result into the FFI shape (fingerprint as a plain string).
fn to_outcome(result: ConnectionTestResult) -> ConnectionTestOutcome {
    match result {
        ConnectionTestResult::Connected { fingerprint } => ConnectionTestOutcome::Connected {
            fingerprint: fingerprint.map(|fp| fp.as_str().to_string()),
        },
        ConnectionTestResult::Failed { error } => ConnectionTestOutcome::Failed { error },
    }
}

/// The dedicated runtime for FFI-driven work (russh needs a tokio context;
/// foreign threads have none). Lazy so `cargo test` binaries without FFI calls
/// never pay for it.
static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("timu-core tokio runtime")
});

/// Prefer the caller's runtime context (e.g. a `#[tokio::test]`); otherwise
/// the dedicated FFI runtime.
fn runtime_handle() -> tokio::runtime::Handle {
    tokio::runtime::Handle::try_current().unwrap_or_else(|_| RUNTIME.handle().clone())
}

/// Concrete transport behind every FFI SSH flow. Real in production; the fake
/// exists (test-only) so the whole bridge is testable with no network.
enum BridgeTransport {
    Real(RusshSshTransport),
    #[cfg(test)]
    Fake(crate::ssh::FakeSshTransport),
}

impl SshTransport for BridgeTransport {
    async fn run_command(&self, command: &str) -> Result<crate::ssh::CommandOutput, TimuError> {
        match self {
            Self::Real(transport) => transport.run_command(command).await,
            #[cfg(test)]
            Self::Fake(transport) => transport.run_command(command).await,
        }
    }
}

impl BridgeTransport {
    async fn disconnect(&self) {
        match self {
            Self::Real(transport) => transport.disconnect().await,
            #[cfg(test)]
            Self::Fake(_) => {}
        }
    }
}

/// App-facing core: connection test (PRD §6) + live connections (PRD §11).
#[derive(uniffi::Object)]
pub struct FfiCore {
    inner: Arc<TimuCore>,
}

#[uniffi::export]
impl FfiCore {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Arc::new(TimuCore::new()),
        })
    }

    pub async fn test_connection(
        &self,
        profile: MachineProfile,
        creds: Credentials,
    ) -> ConnectionTestOutcome {
        let core = self.inner.clone();
        let result = runtime_handle()
            .spawn(async move { core.test_connection(&profile, &creds).await })
            .await
            .expect("timu-core runtime task");
        to_outcome(result)
    }

    /// Open + authenticate a live connection for session work. The first
    /// connect's fingerprint is surfaced by `test_connection` (PRD §6 flow);
    /// the pin is stored either way (TOFU, Hard Block §2.2).
    pub async fn connect(
        &self,
        profile: MachineProfile,
        creds: Credentials,
    ) -> Result<Arc<Connection>, TimuError> {
        let core = self.inner.clone();
        let (transport, _) = runtime_handle()
            .spawn(async move { core.connect(&profile, &creds).await })
            .await
            .expect("timu-core runtime task")?;
        Ok(Arc::new(Connection {
            transport: Arc::new(BridgeTransport::Real(transport)),
        }))
    }
}

/// Foreign sink for pane events. Called from the streaming task; keep it fast.
/// Declared before the objects that use it — uniffi macros expand in source
/// order and need the callback converter first.
#[uniffi::export(callback_interface)]
pub trait PaneEventSink: Send + Sync {
    fn on_event(&self, event: PaneEvent);
    fn on_error(&self, message: String);
}

/// A live SSH connection: readiness, tmux session lifecycle, chat, streaming.
/// Reconnect after backgrounding = build a fresh `Connection` (PRD §13.1).
#[derive(uniffi::Object)]
pub struct Connection {
    transport: Arc<BridgeTransport>,
}

#[uniffi::export]
impl Connection {
    pub async fn readiness(&self) -> Result<Arc<ReadinessReport>, TimuError> {
        let transport = self.transport.clone();
        let report = runtime_handle()
            .spawn(async move { run_readiness_probe(&transport).await })
            .await
            .expect("timu-core runtime task")?;
        Ok(Arc::new(report))
    }

    pub async fn start_agent_session(
        &self,
        folder: String,
        agent_command: String,
    ) -> Result<StartedSession, TimuError> {
        let transport = self.transport.clone();
        runtime_handle()
            .spawn(
                async move { tmux::start_agent_session(&transport, &folder, &agent_command).await },
            )
            .await
            .expect("timu-core runtime task")
    }

    pub async fn send_chat_message(
        &self,
        session_id: String,
        text: String,
    ) -> Result<(), TimuError> {
        let transport = self.transport.clone();
        runtime_handle()
            .spawn(async move { tmux::send_chat_message(&transport, &session_id, &text).await })
            .await
            .expect("timu-core runtime task")
    }

    pub async fn capture_pane(&self, session_id: String) -> Result<String, TimuError> {
        let transport = self.transport.clone();
        runtime_handle()
            .spawn(async move { tmux::capture_pane(&transport, &session_id).await })
            .await
            .expect("timu-core runtime task")
    }

    pub async fn list_tmux_sessions(&self) -> Result<Vec<String>, TimuError> {
        let transport = self.transport.clone();
        runtime_handle()
            .spawn(async move { tmux::list_tmux_sessions(&transport).await })
            .await
            .expect("timu-core runtime task")
    }

    pub async fn kill_session(&self, session_id: String) -> Result<(), TimuError> {
        let transport = self.transport.clone();
        runtime_handle()
            .spawn(async move { tmux::kill_session(&transport, &session_id).await })
            .await
            .expect("timu-core runtime task")
    }

    /// Start streaming the pane's output to `sink`. Attach emits `History`
    /// (catch-up); ticks emit `OutputAppended`; a missing session ends the
    /// stream. Transport errors surface via `sink.on_error`. Returns a handle
    /// whose `stop()` cancels the watcher (app backgrounds the screen).
    pub async fn start_pane_stream(
        &self,
        session_id: String,
        interval_ms: u64,
        sink: Box<dyn PaneEventSink>,
    ) -> Result<Arc<PaneStreamHandle>, TimuError> {
        let watcher = PaneWatcher::new(self.transport.clone(), session_id);
        let interval = Duration::from_millis(interval_ms);
        let watcher_task = runtime_handle().spawn(async move {
            let (tx, mut rx) = mpsc::channel::<PaneEvent>(16);
            let mut run = Box::pin(watcher.run(interval, tx));
            // Forward events as they arrive; the run future owns the only
            // sender, so the channel closes exactly when it finishes.
            let outcome = loop {
                tokio::select! {
                    res = &mut run => break res,
                    event = rx.recv() => match event {
                        Some(event) => sink.on_event(event),
                        None => break Ok(()),
                    },
                }
            };
            // Drain whatever was buffered before reporting a failure, so the
            // sink never sees on_error ahead of earlier events.
            while let Some(event) = rx.recv().await {
                sink.on_event(event);
            }
            if let Err(error) = outcome {
                sink.on_error(error.to_string());
            }
        });
        Ok(Arc::new(PaneStreamHandle {
            watcher: Mutex::new(Some(watcher_task)),
        }))
    }

    pub async fn disconnect(&self) {
        let transport = self.transport.clone();
        runtime_handle()
            .spawn(async move { transport.disconnect().await })
            .await
            .expect("timu-core runtime task");
    }
}

#[cfg(test)]
impl Connection {
    pub(crate) fn for_test(fake: crate::ssh::FakeSshTransport) -> Self {
        Self {
            transport: Arc::new(BridgeTransport::Fake(fake)),
        }
    }
}

/// Cancels a pane stream started with [`Connection::start_pane_stream`].
#[derive(uniffi::Object)]
pub struct PaneStreamHandle {
    watcher: Mutex<Option<JoinHandle<()>>>,
}

#[uniffi::export]
impl PaneStreamHandle {
    /// Stop the watcher. Idempotent; safe after the stream ended on its own.
    /// Buffered events already captured are still delivered, then the
    /// forwarding task exits on its own.
    pub fn stop(&self) {
        if let Some(watcher) = self.watcher.lock().expect("stream mutex").take() {
            watcher.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane_stream::build_capture_pane_full_command;
    use crate::readiness::Tool;
    use crate::readiness_probe::build_probe_command;
    use crate::ssh::{CommandOutput, FakeSshTransport};
    use crate::tmux::{
        build_capture_pane_command, build_has_session_command, build_kill_session_command,
        build_list_sessions_command, build_new_session_command, build_send_message_commands,
    };

    fn fake_connection(fake: FakeSshTransport) -> Arc<Connection> {
        Arc::new(Connection::for_test(fake))
    }

    #[derive(Default)]
    struct CollectingSink {
        events: Mutex<Vec<PaneEvent>>,
        errors: Mutex<Vec<String>>,
    }

    impl PaneEventSink for Arc<CollectingSink> {
        fn on_event(&self, event: PaneEvent) {
            self.events.lock().expect("sink events mutex").push(event);
        }
        fn on_error(&self, message: String) {
            self.errors.lock().expect("sink errors mutex").push(message);
        }
    }

    async fn wait_until(deadline_ms: u64, mut predicate: impl FnMut() -> bool) {
        let start = std::time::Instant::now();
        while !predicate() && start.elapsed() < Duration::from_millis(deadline_ms) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(predicate(), "condition not met within {deadline_ms}ms");
    }

    #[test]
    fn connection_test_outcome_maps_connected_with_fingerprint_string() {
        let outcome = to_outcome(ConnectionTestResult::Connected {
            fingerprint: Some(crate::host_key::Fingerprint::new("SHA256:abc")),
        });
        assert_eq!(
            outcome,
            ConnectionTestOutcome::Connected {
                fingerprint: Some("SHA256:abc".to_string())
            }
        );
    }

    #[test]
    fn connection_test_outcome_maps_connected_without_new_pin_to_none() {
        let outcome = to_outcome(ConnectionTestResult::Connected { fingerprint: None });
        assert_eq!(
            outcome,
            ConnectionTestOutcome::Connected { fingerprint: None }
        );
    }

    #[test]
    fn connection_test_outcome_maps_failed_with_typed_error() {
        let outcome = to_outcome(ConnectionTestResult::Failed {
            error: TimuError::WrongCredentials,
        });
        assert_eq!(
            outcome,
            ConnectionTestOutcome::Failed {
                error: TimuError::WrongCredentials
            }
        );
    }

    #[tokio::test]
    async fn connection_readiness_reports_parsed_tool_status() {
        let mut fake = FakeSshTransport::new();
        fake.script_success(build_probe_command(), "tmux:missing\ngit:ready\n");
        let conn = fake_connection(fake);

        let report = conn.readiness().await.expect("readiness runs");
        assert_eq!(
            report.get(Tool::Tmux),
            crate::readiness::ToolStatus::Missing
        );
        assert_eq!(report.get(Tool::Git), crate::readiness::ToolStatus::Ready);
        assert!(report.tmux_is_missing());
    }

    #[tokio::test]
    async fn connection_readiness_surfaces_transport_errors() {
        let conn = fake_connection(FakeSshTransport::new());
        let err = conn.readiness().await.expect_err("unscripted probe fails");
        assert_eq!(err.code(), "other");
    }

    #[tokio::test]
    async fn connection_start_agent_session_creates_and_launches() {
        let session_id = tmux::build_session_id("/home/me/proj");
        let mut fake = FakeSshTransport::new();
        fake.script(
            build_has_session_command(&session_id),
            CommandOutput::new("", "", 1),
        );
        fake.script_success(build_new_session_command(&session_id, "/home/me/proj"), "");
        for command in build_send_message_commands(&session_id, "codex") {
            fake.script_success(command, "");
        }
        let conn = fake_connection(fake);

        let started = conn
            .start_agent_session("/home/me/proj".to_string(), "codex".to_string())
            .await
            .expect("session starts");
        assert_eq!(started.session_id, session_id);
        assert!(!started.reused);
    }

    #[tokio::test]
    async fn connection_start_agent_session_reuses_existing_session() {
        let session_id = tmux::build_session_id("/home/me/proj");
        let mut fake = FakeSshTransport::new();
        fake.script_success(build_has_session_command(&session_id), "");
        for command in build_send_message_commands(&session_id, "codex") {
            fake.script_success(command, "");
        }
        let conn = fake_connection(fake);

        let started = conn
            .start_agent_session("/home/me/proj".to_string(), "codex".to_string())
            .await
            .expect("session starts");
        assert!(started.reused);
    }

    #[tokio::test]
    async fn connection_start_agent_session_maps_tmux_missing() {
        let session_id = tmux::build_session_id("/home/me/proj");
        let mut fake = FakeSshTransport::new();
        fake.script(
            build_has_session_command(&session_id),
            CommandOutput::new("", "command not found: tmux not found", 127),
        );
        let conn = fake_connection(fake);

        let err = conn
            .start_agent_session("/home/me/proj".to_string(), "codex".to_string())
            .await
            .expect_err("tmux missing");
        assert_eq!(err.code(), "tmux_missing");
    }

    #[tokio::test]
    async fn connection_send_chat_message_types_the_message() {
        let mut fake = FakeSshTransport::new();
        for command in build_send_message_commands("proj-1", "hello agent") {
            fake.script_success(command, "");
        }
        let conn = fake_connection(fake);

        conn.send_chat_message("proj-1".to_string(), "hello agent".to_string())
            .await
            .expect("message sent");
    }

    #[tokio::test]
    async fn connection_capture_pane_returns_pane_text() {
        let mut fake = FakeSshTransport::new();
        fake.script_success(build_capture_pane_command("proj-1"), "hello\nworld\n");
        let conn = fake_connection(fake);

        let pane = conn
            .capture_pane("proj-1".to_string())
            .await
            .expect("capture");
        assert_eq!(pane, "hello\nworld\n");
    }

    #[tokio::test]
    async fn connection_list_tmux_sessions_returns_session_ids() {
        let mut fake = FakeSshTransport::new();
        fake.script_success(build_list_sessions_command(), "proj-1\nproj-2\n");
        let conn = fake_connection(fake);

        let sessions = conn.list_tmux_sessions().await.expect("list runs");
        assert_eq!(sessions, vec!["proj-1".to_string(), "proj-2".to_string()]);
    }

    #[tokio::test]
    async fn connection_kill_session_kills_the_session() {
        let mut fake = FakeSshTransport::new();
        fake.script_success(build_kill_session_command("proj-1"), "");
        let conn = fake_connection(fake);

        conn.kill_session("proj-1".to_string())
            .await
            .expect("kill ok");
    }

    #[tokio::test]
    async fn pane_stream_forwards_history_then_appends_then_session_ended() {
        let mut fake = FakeSshTransport::new();
        fake.script_sequence(
            build_capture_pane_full_command("s"),
            vec![CommandOutput::success("hello\n")],
        );
        fake.script_sequence(
            build_capture_pane_command("s"),
            vec![
                CommandOutput::success("hello\n"),
                CommandOutput::success("hello\nworld\n"),
                CommandOutput::new("", "can't find session: tmux", 1),
            ],
        );
        let conn = fake_connection(fake);
        let sink = Arc::new(CollectingSink::default());

        let handle = conn
            .start_pane_stream("s".to_string(), 5, Box::new(sink.clone()))
            .await
            .expect("stream starts");
        let expected = [
            PaneEvent::History("hello\n".to_string()),
            PaneEvent::OutputAppended("world".to_string()),
            PaneEvent::SessionEnded,
        ];
        wait_until(2000, || {
            sink.events.lock().expect("sink mutex").as_slice() == expected
        })
        .await;
        handle.stop();
    }

    #[tokio::test]
    async fn pane_stream_surfaces_transport_errors_to_the_sink() {
        let mut fake = FakeSshTransport::new();
        fake.script_success(build_capture_pane_full_command("s"), "hello\n");
        // Visible capture deliberately unscripted → transport error on tick 1.
        let conn = fake_connection(fake);
        let sink = Arc::new(CollectingSink::default());

        let handle = conn
            .start_pane_stream("s".to_string(), 5, Box::new(sink.clone()))
            .await
            .expect("stream starts");
        wait_until(2000, || {
            let events = sink.events.lock().expect("sink mutex");
            let errors = sink.errors.lock().expect("sink mutex");
            errors.len() == 1 && events.contains(&PaneEvent::History("hello\n".to_string()))
        })
        .await;
        handle.stop();
    }

    #[tokio::test]
    async fn pane_stream_stop_cancels_further_events() {
        let mut fake = FakeSshTransport::new();
        fake.script_sequence(
            build_capture_pane_full_command("s"),
            vec![CommandOutput::success("a\n")],
        );
        fake.script_sequence(
            build_capture_pane_command("s"),
            vec![
                CommandOutput::success("a\n"),
                CommandOutput::success("a\nb\n"),
                CommandOutput::success("a\nb\nc\n"),
                CommandOutput::success("a\nb\nc\nd\n"),
            ],
        );
        let conn = fake_connection(fake);
        let sink = Arc::new(CollectingSink::default());

        let handle = conn
            .start_pane_stream("s".to_string(), 5, Box::new(sink.clone()))
            .await
            .expect("stream starts");
        wait_until(2000, || {
            sink.events
                .lock()
                .expect("sink mutex")
                .contains(&PaneEvent::OutputAppended("b".to_string()))
        })
        .await;

        handle.stop();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let after_stop = sink.events.lock().expect("sink mutex").len();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(
            sink.events.lock().expect("sink mutex").len(),
            after_stop,
            "no further events after stop"
        );
    }
}
