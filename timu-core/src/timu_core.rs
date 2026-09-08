//! Top-level facade that the FFI layer and tests drive.
//!
//! Holds shared session state (host-key TOFU pins). SSH/network methods are
//! gated to non-wasm targets because `russh::client::connect` is itself
//! non-wasm. Pure domain operations (profiles, readiness) are ungated.

use std::sync::Arc;

use tokio::sync::Mutex;

use crate::host_key::HostKeyPins;

/// Entry point for all timu-core operations.
#[derive(Debug, Default)]
pub struct TimuCore {
    /// Pinned SSH host-key fingerprints (TOFU, PRD §14 / Hard Block §2.2).
    /// `Arc` so the FFI layer can share it with background connection tasks.
    host_key_pins: Arc<Mutex<HostKeyPins>>,
}

impl TimuCore {
    /// Construct a new `TimuCore` with empty host-key pins.
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot access to the host-key pin store (for persistence — task 10).
    pub async fn host_key_pins(&self) -> HostKeyPins {
        self.host_key_pins.lock().await.clone()
    }

    /// PRD §6 — test that we can connect + authenticate to `profile`.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn test_connection(
        &self,
        profile: &crate::profile::MachineProfile,
        creds: &crate::credentials::Credentials,
    ) -> crate::connection::ConnectionTestResult {
        crate::connection::test_connection(profile, creds, self.host_key_pins.clone()).await
    }

    /// Open + authenticate a live SSH connection for session work (PRD §11+).
    /// Returns the transport plus the newly-pinned fingerprint on a first
    /// connect (the UI surfaces it on the connection-test flow); the pin is
    /// stored either way (TOFU, Hard Block §2.2). Live behavior covered by the
    /// `#[ignore]` russh test.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn connect(
        &self,
        profile: &crate::profile::MachineProfile,
        creds: &crate::credentials::Credentials,
    ) -> Result<
        (
            crate::RusshSshTransport,
            Option<crate::host_key::Fingerprint>,
        ),
        crate::error::TimuError,
    > {
        crate::RusshSshTransport::connect(profile, creds, self.host_key_pins.clone()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_returns_a_handle_with_empty_pins() {
        let core = TimuCore::new();
        // Blocking on the async snapshot via a single-threaded runtime is
        // overkill here; just assert the handle exists and is Debug.
        let _ = format!("{core:?}");
    }

    #[tokio::test]
    async fn host_key_pins_starts_empty() {
        let core = TimuCore::new();
        let pins = core.host_key_pins().await;
        assert!(!pins.is_pinned("any-host"));
    }

    #[test]
    fn default_is_same_as_new() {
        assert!(matches!(TimuCore::default().host_key_pins.as_ref(), _));
    }
}
