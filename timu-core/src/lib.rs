//! timu-core: secure transport + session engine for the timu mobile app.
//!
//! Rust owns SSH, tmux, SFTP, and persistent session state. The Expo UI drives
//! this layer through an FFI boundary (planned). See `docs/prds/v0-prd.md`.

mod credentials;
#[cfg(not(target_arch = "wasm32"))]
mod connection;
mod error;
mod folder;
mod host_key;
mod profile;
mod readiness;
mod readiness_probe;
mod ssh;
#[cfg(not(target_arch = "wasm32"))]
mod ssh_russh;
mod store;
mod timu_core;

pub use credentials::Credentials;
#[cfg(not(target_arch = "wasm32"))]
pub use connection::ConnectionTestResult;
pub use error::TimuError;
pub use folder::{build_list_command, list_folders, parse_list_entries, shell_quote, FolderEntry};
pub use host_key::{Fingerprint, HostKeyPins, HostKeyVerdict};
pub use profile::{AuthMethod, MachineProfile, ProfileInvalid};
pub use readiness::{ReadinessReport, Tool, ToolStatus};
pub use readiness_probe::{build_probe_command, parse_probe_output};
pub use ssh::{CommandOutput, FakeSshTransport, SshTransport};
#[cfg(not(target_arch = "wasm32"))]
pub use ssh_russh::RusshSshTransport;
pub use store::{ProfileRecord, SessionRecord, Store};
pub use timu_core::TimuCore;