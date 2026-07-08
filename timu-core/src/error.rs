//! Typed error model for timu-core.
//!
//! Variants map to the actionable SSH failure states in PRD §6 ("Test
//! Connection"). The UI branches on `code()` (stable string, FFI contract) and
//! can render `Display` output directly to the user. Keep both stable: changing
//! a `code()` value is a breaking FFI change.

use std::fmt;

/// All timu-core operations surface failures through this enum.
///
/// Distinct variants exist only where the user can take a *different* corrective
/// action. Anything not actionable gets folded into [`TimuError::Other`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimuError {
    /// The hostname/IP does not resolve or does not point at an SSH server.
    WrongHost,
    /// The SSH username is rejected by the server (no such user).
    WrongUsername,
    /// The password or private key is wrong / wrong format / wrong passphrase.
    WrongCredentials,
    /// The TCP port is closed, filtered, or timing out (host reachable).
    PortUnreachable,
    /// The phone has no usable network (airplane mode, no data, DNS down).
    NetworkUnavailable,
    /// Auth material is valid but the account is not permitted to log in
    /// (e.g. `AllowUsers` restriction, root login disabled).
    PermissionDenied,
    /// Anything not covered above. Carries a short diagnostic string for logs.
    /// Never shown verbatim to the user as the primary message.
    Other(String),
}

impl TimuError {
    /// Stable, machine-readable identifier for this error. Used as the FFI
    /// discriminant — do **not** rename existing values.
    pub fn code(&self) -> &'static str {
        match self {
            Self::WrongHost => "wrong_host",
            Self::WrongUsername => "wrong_username",
            Self::WrongCredentials => "wrong_credentials",
            Self::PortUnreachable => "port_unreachable",
            Self::NetworkUnavailable => "network_unavailable",
            Self::PermissionDenied => "permission_denied",
            Self::Other(_) => "other",
        }
    }

    /// One-line, user-facing label. Safe to render directly in the UI.
    pub fn user_label(&self) -> &'static str {
        match self {
            Self::WrongHost => "Couldn't reach that host",
            Self::WrongUsername => "That username was rejected",
            Self::WrongCredentials => "Wrong password or key",
            Self::PortUnreachable => "That port isn't reachable",
            Self::NetworkUnavailable => "No network connection",
            Self::PermissionDenied => "Not allowed to log in",
            Self::Other(_) => "Something went wrong",
        }
    }
}

impl fmt::Display for TimuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Other(msg) => write!(f, "{}", msg),
            other => write!(f, "{}", other.user_label()),
        }
    }
}

impl std::error::Error for TimuError {}

#[cfg(test)]
mod tests {
    use super::TimuError;

    /// Each variant must produce a distinct, stable `code()` — these strings
    /// are the FFI contract and must never change without a migration.
    #[test]
    fn each_variant_has_a_unique_stable_code() {
        let codes = [
            TimuError::WrongHost.code(),
            TimuError::WrongUsername.code(),
            TimuError::WrongCredentials.code(),
            TimuError::PortUnreachable.code(),
            TimuError::NetworkUnavailable.code(),
            TimuError::PermissionDenied.code(),
            TimuError::Other("boom".into()).code(),
        ];
        assert_eq!(codes, [
            "wrong_host",
            "wrong_username",
            "wrong_credentials",
            "port_unreachable",
            "network_unavailable",
            "permission_denied",
            "other",
        ]);
        // No two variants share a code.
        let mut sorted = codes.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), codes.len(), "duplicate codes detected");
    }

    #[test]
    fn user_label_is_non_empty_for_every_actionable_variant() {
        for err in [
            TimuError::WrongHost,
            TimuError::WrongUsername,
            TimuError::WrongCredentials,
            TimuError::PortUnreachable,
            TimuError::NetworkUnavailable,
            TimuError::PermissionDenied,
        ] {
            assert!(!err.user_label().is_empty(), "{:?} has empty label", err);
        }
    }

    #[test]
    fn display_uses_user_label_for_actionable_variants() {
        assert_eq!(TimuError::WrongHost.to_string(), "Couldn't reach that host");
        assert_eq!(
            TimuError::WrongCredentials.to_string(),
            "Wrong password or key"
        );
    }

    #[test]
    fn display_uses_the_carried_message_for_other() {
        let err = TimuError::Other("ssh handshake timeout".into());
        assert_eq!(err.to_string(), "ssh handshake timeout");
        assert_eq!(err.code(), "other");
    }

    #[test]
    fn equality_compares_other_by_carried_message() {
        assert_eq!(
            TimuError::Other("x".into()),
            TimuError::Other("x".into())
        );
        assert_ne!(
            TimuError::Other("x".into()),
            TimuError::Other("y".into())
        );
        assert_eq!(TimuError::WrongHost, TimuError::WrongHost);
    }
}