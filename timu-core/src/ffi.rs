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
use crate::folder::FolderEntry;
use crate::host_key::{Fingerprint, HostKeyPins};
use crate::pane_stream::{PaneEvent, PaneWatcher};
use crate::profile::MachineProfile;
use crate::readiness::ReadinessReport;
use crate::readiness_probe::run_readiness_probe;
use crate::ssh::SshTransport;
use crate::store::{ProfileRecord, SessionRecord};
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

/// One persisted host-key pin (host → fingerprint) as stored in SQLite.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HostKeyPin {
    pub host: String,
    pub fingerprint: String,
}

/// SQLite failures are not per-variant actionable (no different corrective
/// action than "something's wrong with local storage") — fold into `Other`.
fn map_store_error(error: rusqlite::Error) -> TimuError {
    TimuError::Other(format!("store error: {error}"))
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

    /// App boot: inject pins loaded from the SQLite store so the first
    /// connect verifies against existing pins instead of re-running TOFU.
    pub async fn load_pins(&self, pins: Vec<HostKeyPin>) {
        let mut map = std::collections::HashMap::new();
        for pin in pins {
            map.insert(pin.host, Fingerprint::new(pin.fingerprint));
        }
        let core = self.inner.clone();
        runtime_handle()
            .spawn(async move {
                core.set_host_key_pins(HostKeyPins::from_map(map)).await;
            })
            .await
            .expect("timu-core runtime task");
    }

    /// Current in-memory pins, for persisting new ones after a connect.
    pub async fn pins(&self) -> Vec<HostKeyPin> {
        let core = self.inner.clone();
        let pins = runtime_handle()
            .spawn(async move { core.host_key_pins().await })
            .await
            .expect("timu-core runtime task");
        pins.to_map()
            .iter()
            .map(|(host, fingerprint)| HostKeyPin {
                host: host.clone(),
                fingerprint: fingerprint.as_str().to_string(),
            })
            .collect()
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

/// SQLite-backed persistent state (PRD §13): machine profiles, sessions,
/// recent/favorite folders, host-key pins. Holds no secrets (ADR-009).
/// rusqlite's connection is not `Sync`, so calls lock the single store.
#[derive(uniffi::Object)]
pub struct Store {
    inner: std::sync::Mutex<crate::store::Store>,
}

#[uniffi::export]
impl Store {
    #[uniffi::constructor]
    pub fn open(path: String) -> Result<Arc<Self>, TimuError> {
        let store =
            crate::store::Store::open(std::path::Path::new(&path)).map_err(map_store_error)?;
        Ok(Arc::new(Self {
            inner: Mutex::new(store),
        }))
    }

    pub fn save_profile(&self, profile: MachineProfile) -> Result<i64, TimuError> {
        self.inner
            .lock()
            .expect("store mutex")
            .save_profile(&profile)
            .map_err(map_store_error)
    }

    pub fn list_profiles(&self) -> Result<Vec<ProfileRecord>, TimuError> {
        self.inner
            .lock()
            .expect("store mutex")
            .list_profiles()
            .map_err(map_store_error)
    }

    pub fn get_profile(&self, id: i64) -> Result<Option<ProfileRecord>, TimuError> {
        self.inner
            .lock()
            .expect("store mutex")
            .get_profile(id)
            .map_err(map_store_error)
    }

    pub fn delete_profile(&self, id: i64) -> Result<bool, TimuError> {
        self.inner
            .lock()
            .expect("store mutex")
            .delete_profile(id)
            .map_err(map_store_error)
    }

    pub fn touch_profile(&self, id: i64) -> Result<(), TimuError> {
        self.inner
            .lock()
            .expect("store mutex")
            .touch_profile(id)
            .map_err(map_store_error)
    }

    pub fn save_session(&self, session: SessionRecord) -> Result<i64, TimuError> {
        self.inner
            .lock()
            .expect("store mutex")
            .save_session(&session)
            .map_err(map_store_error)
    }

    pub fn list_sessions(&self, profile_id: i64) -> Result<Vec<SessionRecord>, TimuError> {
        self.inner
            .lock()
            .expect("store mutex")
            .list_sessions(profile_id)
            .map_err(map_store_error)
    }

    pub fn add_recent_folder(&self, entry: FolderEntry) -> Result<(), TimuError> {
        self.inner
            .lock()
            .expect("store mutex")
            .add_recent_folder(&entry)
            .map_err(map_store_error)
    }

    pub fn list_recent_folders(&self) -> Result<Vec<FolderEntry>, TimuError> {
        self.inner
            .lock()
            .expect("store mutex")
            .list_recent_folders()
            .map_err(map_store_error)
    }

    pub fn add_favorite(&self, entry: FolderEntry) -> Result<(), TimuError> {
        self.inner
            .lock()
            .expect("store mutex")
            .add_favorite(&entry)
            .map_err(map_store_error)
    }

    pub fn list_favorites(&self) -> Result<Vec<FolderEntry>, TimuError> {
        self.inner
            .lock()
            .expect("store mutex")
            .list_favorites()
            .map_err(map_store_error)
    }

    pub fn remove_favorite(&self, path: String) -> Result<bool, TimuError> {
        self.inner
            .lock()
            .expect("store mutex")
            .remove_favorite(&path)
            .map_err(map_store_error)
    }

    pub fn save_host_key_pin(&self, host: String, fingerprint: String) -> Result<(), TimuError> {
        self.inner
            .lock()
            .expect("store mutex")
            .save_host_key_pin(&host, &Fingerprint::new(fingerprint))
            .map_err(map_store_error)
    }

    pub fn load_host_key_pins(&self) -> Result<Vec<HostKeyPin>, TimuError> {
        let pins = self
            .inner
            .lock()
            .expect("store mutex")
            .load_host_key_pins()
            .map_err(map_store_error)?;
        Ok(pins
            .to_map()
            .iter()
            .map(|(host, fingerprint)| HostKeyPin {
                host: host.clone(),
                fingerprint: fingerprint.as_str().to_string(),
            })
            .collect())
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
    use crate::folder::FolderEntry;
    use crate::pane_stream::build_capture_pane_full_command;
    use crate::profile::AuthMethod;
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

    fn temp_store_path(name: &str) -> String {
        let dir =
            std::env::temp_dir().join(format!("timu-ffi-store-{}-{}", std::process::id(), name));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir.join("store.db").to_string_lossy().to_string()
    }

    #[test]
    fn store_save_and_get_profile_round_trips() {
        let store = Store::open(temp_store_path("profiles")).expect("store opens");
        let mut profile = MachineProfile {
            name: "my-vps".to_string(),
            host: "203.0.113.7".to_string(),
            username: "dev".to_string(),
            port: 2200,
            auth_method: AuthMethod::KeyPaste,
        };
        let id = store.save_profile(profile.clone()).expect("saved");
        profile.name = "renamed".to_string(); // saved value is a copy, not a reference

        let loaded = store.get_profile(id).expect("get").expect("exists");
        assert_eq!(loaded.name, "my-vps");
        assert_eq!(loaded.host, "203.0.113.7");
        assert_eq!(loaded.username, "dev");
        assert_eq!(loaded.port, 2200);
        assert_eq!(loaded.auth_method, AuthMethod::KeyPaste);
    }

    #[test]
    fn store_missing_profile_reads_none() {
        let store = Store::open(temp_store_path("missing")).expect("store opens");
        assert!(store.get_profile(999).expect("get").is_none());
    }

    #[test]
    fn store_list_and_delete_profile() {
        let store = Store::open(temp_store_path("list-delete")).expect("store opens");
        let first = store
            .save_profile(MachineProfile::default())
            .expect("saved");
        let second = store
            .save_profile(MachineProfile::default())
            .expect("saved");
        assert_ne!(first, second);
        assert_eq!(store.list_profiles().expect("list").len(), 2);
        assert!(store.delete_profile(first).expect("delete"));
        assert!(!store.delete_profile(first).expect("delete again"));
        assert_eq!(store.list_profiles().expect("list").len(), 1);
    }

    #[test]
    fn store_touch_profile_succeeds() {
        let store = Store::open(temp_store_path("touch")).expect("store opens");
        let id = store
            .save_profile(MachineProfile::default())
            .expect("saved");
        store.touch_profile(id).expect("touch");
    }

    #[test]
    fn store_save_session_inserts_then_updates_in_place() {
        let store = Store::open(temp_store_path("sessions")).expect("store opens");
        let profile_id = store
            .save_profile(MachineProfile::default())
            .expect("profile saved");
        let session = SessionRecord {
            id: 0,
            profile_id,
            agent: "codex".to_string(),
            folder: "/home/me/proj".to_string(),
            tmux_session_id: "proj-abc".to_string(),
            status: "active".to_string(),
        };
        let id = store.save_session(session.clone()).expect("inserted");
        assert!(id > 0);

        let updated = SessionRecord {
            id,
            status: "closed".to_string(),
            ..session
        };
        let again = store.save_session(updated).expect("updated");
        assert_eq!(again, id, "update must not insert a second row");

        let rows = store.list_sessions(profile_id).expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "closed");
        assert_eq!(rows[0].tmux_session_id, "proj-abc");
    }

    #[test]
    fn store_recent_folders_order_by_last_use_and_favorites_round_trip() {
        let store = Store::open(temp_store_path("folders")).expect("store opens");
        let entry = |path: &str| FolderEntry {
            path: path.to_string(),
            name: path.rsplit('/').next().unwrap().to_string(),
            is_git_repo: true,
        };
        store.add_recent_folder(entry("/home/a")).expect("add");
        store.add_recent_folder(entry("/home/b")).expect("add");
        store
            .add_recent_folder(entry("/home/a"))
            .expect("re-add bumps a");

        let recents = store.list_recent_folders().expect("list");
        assert_eq!(recents.len(), 2);
        assert_eq!(recents[0].path, "/home/a", "most recently used first");

        store.add_favorite(entry("/home/b")).expect("favorite");
        assert_eq!(store.list_favorites().expect("list").len(), 1);
        assert!(
            store
                .remove_favorite("/home/b".to_string())
                .expect("remove")
        );
        assert!(
            !store
                .remove_favorite("/home/b".to_string())
                .expect("remove again")
        );
        assert!(store.list_favorites().expect("list").is_empty());
    }

    #[test]
    fn store_host_key_pins_round_trip() {
        let store = Store::open(temp_store_path("pins")).expect("store opens");
        store
            .save_host_key_pin("my-vps".to_string(), "SHA256:abc".to_string())
            .expect("saved");
        let pins = store.load_host_key_pins().expect("loaded");
        assert_eq!(
            pins,
            vec![HostKeyPin {
                host: "my-vps".to_string(),
                fingerprint: "SHA256:abc".to_string(),
            }]
        );
    }

    #[tokio::test]
    async fn ffi_core_pins_round_trip_through_the_store() {
        let store = Store::open(temp_store_path("pin-inject")).expect("store opens");
        store
            .save_host_key_pin("my-vps".to_string(), "SHA256:abc".to_string())
            .expect("saved");

        let core = FfiCore::new();
        core.load_pins(store.load_host_key_pins().expect("loaded"))
            .await;
        assert_eq!(
            core.pins().await,
            vec![HostKeyPin {
                host: "my-vps".to_string(),
                fingerprint: "SHA256:abc".to_string(),
            }]
        );
    }

    #[test]
    fn store_open_failure_maps_to_typed_error() {
        // Arc<Store> isn't Debug, so match instead of expect_err.
        match Store::open("/nonexistent-timu-dir/nope/timu.db".to_string()) {
            Err(err) => assert_eq!(err.code(), "other"),
            Ok(_) => panic!("bad path must fail"),
        }
    }

    #[test]
    fn store_persisted_profile_survives_reopening_the_database() {
        let path = temp_store_path("reopen");
        let id = {
            let store = Store::open(path.clone()).expect("first open");
            store
                .save_profile(MachineProfile {
                    name: "vps".to_string(),
                    ..MachineProfile::default()
                })
                .expect("saved")
        };
        let store = Store::open(path).expect("second open");
        let loaded = store.get_profile(id).expect("get").expect("exists");
        assert_eq!(loaded.name, "vps");
    }
}
