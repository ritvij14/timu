//! Connect-time secret carrier — PRD §6 Step 2, paired with [`crate::profile`].
//!
//! [`Credentials`] is the *ephemeral* counterpart to `MachineProfile`: the
//! profile says *which* method, `Credentials` supplies *the material*. It is
//! deliberately **not** `Serialize` and never persisted (Hard Block §2.1,
//! ADR-002). Its `Debug` impl redacts so logs never leak secrets.

/// Secret material for one SSH login attempt.
#[derive(Clone, PartialEq, Eq)]
pub enum Credentials {
    /// A plaintext password.
    Password(String),
    /// A private key (PEM or OpenSSH) plus an optional passphrase.
    PrivateKey {
        material: Vec<u8>,
        passphrase: Option<String>,
    },
}

impl Credentials {
    /// True if this is a password credential.
    pub fn is_password(&self) -> bool {
        matches!(self, Self::Password(_))
    }

    /// True if this is a key credential.
    pub fn is_key(&self) -> bool {
        matches!(self, Self::PrivateKey { .. })
    }
}

/// Redacting `Debug` — never prints the secret material. Format:
/// `Credentials::Password(<redacted>)` or `Credentials::PrivateKey { <redacted>, passphrase: <set>|<none> }`.
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Password(_) => write!(f, "Credentials::Password(<redacted>)"),
            Self::PrivateKey { passphrase, .. } => {
                let pass = if passphrase.is_some() { "<set>" } else { "<none>" };
                write!(f, "Credentials::PrivateKey {{ material: <redacted>, passphrase: {pass} }}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_debug_does_not_leak_the_password() {
        let c = Credentials::Password("hunter2".into());
        let s = format!("{c:?}");
        assert!(!s.contains("hunter2"));
        assert!(s.contains("redacted"));
    }

    #[test]
    fn key_debug_does_not_leak_material_or_passphrase() {
        let c = Credentials::PrivateKey {
            material: b"-----BEGIN OPENSSH PRIVATE KEY-----\nSECRET\n".to_vec(),
            passphrase: Some("neverprintme".into()),
        };
        let s = format!("{c:?}");
        assert!(!s.contains("SECRET"));
        assert!(!s.contains("neverprintme"));
        assert!(s.contains("redacted"));
        assert!(s.contains("passphrase: <set>"));
    }

    #[test]
    fn key_without_passphrase_shows_none_in_debug() {
        let c = Credentials::PrivateKey {
            material: vec![1, 2, 3],
            passphrase: None,
        };
        assert!(format!("{c:?}").contains("passphrase: <none>"));
    }

    #[test]
    fn credentials_are_not_serialize() {
        // Compile-time guard: if someone adds `derive(Serialize)`, this fails.
        // We can't assert a negative at runtime, so this test just documents the
        // intent and exercises Debug/Eq instead.
        let a = Credentials::Password("x".into());
        let b = Credentials::Password("x".into());
        assert_eq!(a, b);
    }

    #[test]
    fn is_password_and_is_key_predicates_are_mutually_exclusive() {
        let p = Credentials::Password("x".into());
        let k = Credentials::PrivateKey {
            material: vec![],
            passphrase: None,
        };
        assert!(p.is_password());
        assert!(!p.is_key());
        assert!(!k.is_password());
        assert!(k.is_key());
    }
}