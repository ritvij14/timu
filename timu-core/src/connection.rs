//! Connection test — PRD §6 "Test Connection".
//!
//! [`test_connection`] opens + authenticates an SSH connection (via
//! [`crate::RusshSshTransport`]) and reports a typed [`ConnectionTestResult`].
//! The success/failure mapping is pure ([`ConnectionTestResult::from_outcome`])
//! and unit-tested; the live connect path is exercised by the `#[ignore]` test
//! in `ssh_russh`.

use std::sync::Arc;

use tokio::sync::Mutex;

use crate::credentials::Credentials;
use crate::error::TimuError;
use crate::host_key::{Fingerprint, HostKeyPins};
use crate::profile::MachineProfile;
use crate::RusshSshTransport;

/// Outcome the UI renders on the "Test connection" screen (PRD §6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionTestResult {
    /// SSH connect + auth succeeded. `fingerprint` is the newly-pinned host-key
    /// fingerprint, if this was a first connect (None when it matched a pin).
    Connected { fingerprint: Option<Fingerprint> },
    /// Connect or auth failed; `error` carries the actionable PRD §6 reason.
    Failed { error: TimuError },
}

impl ConnectionTestResult {
    /// Map a connect outcome into a UI result. Pure — tested directly.
    pub fn from_outcome(outcome: Result<Option<Fingerprint>, TimuError>) -> Self {
        match outcome {
            Ok(fingerprint) => Self::Connected { fingerprint },
            Err(error) => Self::Failed { error },
        }
    }

    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    pub fn failed(&self) -> Option<&TimuError> {
        match self {
            Self::Failed { error } => Some(error),
            _ => None,
        }
    }
}

/// Run a PRD §6 connection test: connect, authenticate, disconnect, report.
pub async fn test_connection(
    profile: &MachineProfile,
    creds: &Credentials,
    pins: Arc<Mutex<HostKeyPins>>,
) -> ConnectionTestResult {
    match RusshSshTransport::connect(profile, creds, pins).await {
        Ok((transport, fingerprint)) => {
            transport.disconnect().await;
            ConnectionTestResult::Connected { fingerprint }
        }
        Err(error) => ConnectionTestResult::Failed { error },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_key::Fingerprint;

    #[test]
    fn successful_connect_with_first_seen_fingerprint_is_connected() {
        let fp = Some(Fingerprint::new("SHA256:abc"));
        let r = ConnectionTestResult::from_outcome(Ok(fp));
        assert!(r.is_connected());
        assert_eq!(r.failed(), None);
        assert!(matches!(
            r,
            ConnectionTestResult::Connected { fingerprint } if fingerprint.as_ref().unwrap().as_str() == "SHA256:abc"
        ));
    }

    #[test]
    fn successful_connect_returning_none_fingerprint_is_still_connected() {
        // None means the host key matched an existing pin (not a first connect).
        let r = ConnectionTestResult::from_outcome(Ok(None));
        assert!(r.is_connected());
        assert!(matches!(r, ConnectionTestResult::Connected { fingerprint: None }));
    }

    #[test]
    fn each_actionable_error_surfaces_as_failed_with_its_code() {
        for err in [
            TimuError::WrongHost,
            TimuError::WrongUsername,
            TimuError::WrongCredentials,
            TimuError::PortUnreachable,
            TimuError::NetworkUnavailable,
            TimuError::PermissionDenied,
            TimuError::Other("boom".into()),
        ] {
            let expected_code = err.code();
            let r = ConnectionTestResult::from_outcome(Err(err.clone()));
            assert!(!r.is_connected(), "{:?} should not be connected", err);
            let failed = r.failed().expect("should have a failure");
            assert_eq!(failed, &err);
            assert_eq!(failed.code(), expected_code);
        }
    }

    #[test]
    fn failed_result_is_not_connected_and_exposes_the_error() {
        let r = ConnectionTestResult::from_outcome(Err(TimuError::PortUnreachable));
        assert!(!r.is_connected());
        assert_eq!(r.failed().unwrap().code(), "port_unreachable");
    }

    #[test]
    fn connected_result_has_no_failure() {
        let r = ConnectionTestResult::from_outcome(Ok(None));
        assert_eq!(r.failed(), None);
    }
}