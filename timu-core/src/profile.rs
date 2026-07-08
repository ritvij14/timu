//! Machine connection profile — PRD §6 "Connect a Machine".
//!
//! A [`MachineProfile`] is the *persistable description* of a target machine:
//! where it is, who you log in as, and which auth method you chose. It does
//! **not** hold the credentials themselves (password, private key bytes,
//! passphrase) — those live in platform secure storage and are supplied at
//! connect time. This keeps the profile safe to persist and sync (PRD §14).

use serde::{Deserialize, Serialize};

/// Which authentication method the user picked for this machine.
///
/// The method kind only. Secret material is handled out-of-band.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMethod {
    /// Log in with a password (stored in secure storage, not here).
    Password,
    /// User pasted a private key into the app.
    KeyPaste,
    /// User imported a private key file from the device.
    KeyFile,
}

impl Default for AuthMethod {
    fn default() -> Self {
        Self::Password
    }
}

/// A saved SSH target. Persistable; carries no secrets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineProfile {
    /// Friendly name shown in the session/machine list.
    pub name: String,
    /// Hostname or IP, e.g. `123.45.67.89`, `192.168.1.20`, `my-mac.local`.
    pub host: String,
    /// SSH username.
    pub username: String,
    /// SSH port. Default 22 per PRD §6.
    pub port: u16,
    /// How the user authenticates.
    pub auth_method: AuthMethod,
}

impl Default for MachineProfile {
    fn default() -> Self {
        Self {
            name: String::new(),
            host: String::new(),
            username: String::new(),
            port: 22,
            auth_method: AuthMethod::default(),
        }
    }
}

/// Which field failed validation, and why. Kept as a tiny enum (not folded into
/// [`crate::TimuError`]) because validation failures are not SSH failures and
/// the UI wants to highlight the offending field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileInvalid {
    EmptyName,
    EmptyHost,
    EmptyUsername,
    PortZero,
}

impl MachineProfile {
    /// Returns the default profile (port 22, password auth, empty strings).
    pub fn new() -> Self {
        Self::default()
    }

    /// Field-level validation. The UI calls this before saving/connecting.
    pub fn validate(&self) -> Result<(), ProfileInvalid> {
        if self.name.trim().is_empty() {
            return Err(ProfileInvalid::EmptyName);
        }
        if self.host.trim().is_empty() {
            return Err(ProfileInvalid::EmptyHost);
        }
        if self.username.trim().is_empty() {
            return Err(ProfileInvalid::EmptyUsername);
        }
        if self.port == 0 {
            return Err(ProfileInvalid::PortZero);
        }
        Ok(())
    }
}

impl std::fmt::Display for ProfileInvalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::EmptyName => "Machine name is required",
            Self::EmptyHost => "Hostname or IP is required",
            Self::EmptyUsername => "SSH username is required",
            Self::PortZero => "Port must be greater than 0",
        };
        write!(f, "{msg}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> MachineProfile {
        MachineProfile {
            name: "My VPS".into(),
            host: "123.45.67.89".into(),
            username: "root".into(),
            port: 22,
            auth_method: AuthMethod::Password,
        }
    }

    #[test]
    fn default_profile_has_port_22_and_password_auth() {
        let p = MachineProfile::new();
        assert_eq!(p.port, 22);
        assert_eq!(p.auth_method, AuthMethod::Password);
    }

    #[test]
    fn valid_profile_passes_validation() {
        assert_eq!(valid().validate(), Ok(()));
    }

    #[test]
    fn rejects_empty_name() {
        let mut p = valid();
        p.name = "  ".into();
        assert_eq!(p.validate(), Err(ProfileInvalid::EmptyName));
    }

    #[test]
    fn rejects_empty_host() {
        let mut p = valid();
        p.host = String::new();
        assert_eq!(p.validate(), Err(ProfileInvalid::EmptyHost));
    }

    #[test]
    fn rejects_empty_username() {
        let mut p = valid();
        p.username = String::new();
        assert_eq!(p.validate(), Err(ProfileInvalid::EmptyUsername));
    }

    #[test]
    fn rejects_port_zero() {
        let mut p = valid();
        p.port = 0;
        assert_eq!(p.validate(), Err(ProfileInvalid::PortZero));
    }

    #[test]
    fn accepts_all_non_default_ports_in_u16_range() {
        let mut p = valid();
        for port in [1u16, 2222, 65535] {
            p.port = port;
            assert_eq!(p.validate(), Ok(()), "port {port} should be valid");
        }
    }

    #[test]
    fn auth_method_default_is_password() {
        assert_eq!(AuthMethod::default(), AuthMethod::Password);
    }

    #[test]
    fn profile_round_trips_through_serde_json() {
        let p = valid();
        let json = serde_json::to_string(&p).expect("serialize");
        let back: MachineProfile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(p, back);
    }

    #[test]
    fn auth_method_round_trips_through_serde_json() {
        for method in [
            AuthMethod::Password,
            AuthMethod::KeyPaste,
            AuthMethod::KeyFile,
        ] {
            let json = serde_json::to_string(&method).expect("serialize");
            let back: AuthMethod = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(method, back);
        }
    }

    #[test]
    fn profile_does_not_carry_secrets() {
        // The persisted shape must never include password/key/passphrase fields.
        let p = valid();
        let json = serde_json::to_string(&p).expect("serialize");
        assert!(!json.contains("password"));
        assert!(!json.contains("private_key"));
        assert!(!json.contains("passphrase"));
    }

    #[test]
    fn profile_invalid_display_is_human_readable() {
        assert_eq!(ProfileInvalid::EmptyHost.to_string(), "Hostname or IP is required");
    }
}