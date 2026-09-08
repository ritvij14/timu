//! timu-core: secure transport + session engine for the timu mobile app.
//!
//! Rust owns SSH, tmux, SFTP, and persistent session state. The Expo UI drives
//! this layer through an FFI boundary (planned). See `docs/prds/v0-prd.md`.

#[cfg(not(target_arch = "wasm32"))]
mod connection;
mod credentials;
mod error;
mod folder;
mod host_key;
mod pane_stream;
mod profile;
mod readiness;
mod readiness_probe;
mod ssh;
#[cfg(not(target_arch = "wasm32"))]
mod ssh_russh;
mod store;
mod timu_core;
mod tmux;

#[cfg(not(target_arch = "wasm32"))]
pub use connection::ConnectionTestResult;
pub use credentials::Credentials;
pub use error::TimuError;
pub use folder::{FolderEntry, build_list_command, list_folders, parse_list_entries, shell_quote};
pub use host_key::{Fingerprint, HostKeyPins, HostKeyVerdict};
pub use pane_stream::{
    CATCH_UP_SCROLLBACK_LINES, PaneEvent, PaneWatcher, build_capture_pane_full_command,
    build_capture_pane_window_command, count_new_lines,
};
pub use profile::{AuthMethod, MachineProfile, ProfileInvalid};
pub use readiness::{ReadinessReport, Tool, ToolStatus};
pub use readiness_probe::{build_probe_command, parse_probe_output};
pub use ssh::{CommandOutput, FakeSshTransport, SshTransport};
#[cfg(not(target_arch = "wasm32"))]
pub use ssh_russh::RusshSshTransport;
pub use store::{ProfileRecord, SessionRecord, Store};
pub use timu_core::TimuCore;
pub use tmux::{
    StartedSession, build_capture_pane_command, build_has_session_command,
    build_kill_session_command, build_list_sessions_command, build_new_session_command,
    build_send_message_commands, build_session_id, capture_pane, kill_session, list_tmux_sessions,
    parse_session_list, send_chat_message, start_agent_session,
};
