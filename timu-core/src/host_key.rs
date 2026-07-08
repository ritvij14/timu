//! SSH host-key pinning — trust-on-first-use (TOFU), PRD §14 + Hard Block §2.2.
//!
//! On first connect to a host, we accept the server's fingerprint and pin it.
//! On every later connect, the offered fingerprint must match the pinned one or
//! the connection is rejected. This module is pure: it compares fingerprints as
//! opaque strings. The actual fingerprint bytes come from the SSH library
//! (russh exposes `PublicKey::fingerprint(HashAlg::Sha256)`), so we don't
//! re-implement hashing here.

use std::collections::HashMap;
use std::fmt;

/// A server public-key fingerprint, as produced by the SSH library
/// (base64 SHA-256 by convention). Treated as opaque and compared by string.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Fingerprint(String);

impl Fingerprint {
    /// Wrap a fingerprint string produced by the SSH library.
    pub fn new(fp: impl Into<String>) -> Self {
        Self(fp.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fingerprint({})", self.0)
    }
}

/// Outcome of verifying a server fingerprint against pinned state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKeyVerdict {
    /// No fingerprint pinned for this host yet. Caller should accept and pin.
    FirstSeen,
    /// Pinned fingerprint matches the one offered by the server.
    Matches,
    /// Pinned fingerprint differs — reject the connection (Hard Block §2.2).
    Mismatch,
}

/// In-memory store of pinned host fingerprints. V0 persists this alongside
/// machine profiles (task 10); the persistence seam is `to_map` / `from_map`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostKeyPins {
    pins: HashMap<String, Fingerprint>,
}

impl HostKeyPins {
    pub fn new() -> Self {
        Self::default()
    }

    /// Verify an offered fingerprint for `host`.
    pub fn verify(&self, host: &str, fp: &Fingerprint) -> HostKeyVerdict {
        match self.pins.get(host) {
            None => HostKeyVerdict::FirstSeen,
            Some(pinned) if pinned == fp => HostKeyVerdict::Matches,
            Some(_) => HostKeyVerdict::Mismatch,
        }
    }

    /// Pin a fingerprint for a host. Overwrites any existing pin (used by the
    /// "reset trust" flow; normal path only pins on `FirstSeen`).
    pub fn pin(&mut self, host: &str, fp: Fingerprint) {
        self.pins.insert(host.to_string(), fp);
    }

    /// Forget the pinned fingerprint for a host, if any.
    pub fn unpin(&mut self, host: &str) {
        self.pins.remove(host);
    }

    /// True if a fingerprint is pinned for this host.
    pub fn is_pinned(&self, host: &str) -> bool {
        self.pins.contains_key(host)
    }

    /// Snapshot for persistence (task 10).
    pub fn to_map(&self) -> &HashMap<String, Fingerprint> {
        &self.pins
    }

    /// Restore from a persisted snapshot.
    pub fn from_map(map: HashMap<String, Fingerprint>) -> Self {
        Self { pins: map }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp(s: &str) -> Fingerprint {
        Fingerprint::new(s)
    }

    #[test]
    fn unknown_host_returns_first_seen() {
        let pins = HostKeyPins::new();
        assert_eq!(pins.verify("my-vps", &fp("abc")), HostKeyVerdict::FirstSeen);
    }

    #[test]
    fn pinned_matching_fingerprint_returns_matches() {
        let mut pins = HostKeyPins::new();
        pins.pin("my-vps", fp("abc"));
        assert_eq!(pins.verify("my-vps", &fp("abc")), HostKeyVerdict::Matches);
    }

    #[test]
    fn pinned_mismatched_fingerprint_returns_mismatch() {
        let mut pins = HostKeyPins::new();
        pins.pin("my-vps", fp("abc"));
        assert_eq!(pins.verify("my-vps", &fp("xyz")), HostKeyVerdict::Mismatch);
    }

    #[test]
    fn pins_are_per_host() {
        let mut pins = HostKeyPins::new();
        pins.pin("a", fp("1"));
        // Different host is still first-seen.
        assert_eq!(pins.verify("b", &fp("1")), HostKeyVerdict::FirstSeen);
        // Same fingerprint on a different host does not match a's pin.
        pins.pin("b", fp("2"));
        assert_eq!(pins.verify("b", &fp("1")), HostKeyVerdict::Mismatch);
    }

    #[test]
    fn unpin_forgets_the_fingerprint() {
        let mut pins = HostKeyPins::new();
        pins.pin("h", fp("x"));
        assert!(pins.is_pinned("h"));
        pins.unpin("h");
        assert!(!pins.is_pinned("h"));
        assert_eq!(pins.verify("h", &fp("x")), HostKeyVerdict::FirstSeen);
    }

    #[test]
    fn round_trip_through_to_map_from_map_preserves_pins() {
        let mut pins = HostKeyPins::new();
        pins.pin("a", fp("1"));
        pins.pin("b", fp("2"));
        let snapshot = pins.to_map().clone();
        let restored = HostKeyPins::from_map(snapshot);
        assert_eq!(restored, pins);
        assert_eq!(restored.verify("a", &fp("1")), HostKeyVerdict::Matches);
        assert_eq!(restored.verify("a", &fp("WRONG")), HostKeyVerdict::Mismatch);
    }

    #[test]
    fn fingerprint_display_and_debug_expose_value() {
        // The UI shows fingerprints to the user on first connect, so Display
        // must surface the actual value (unlike Credentials).
        let f = fp("SHA256:abc123");
        assert_eq!(f.to_string(), "SHA256:abc123");
        assert!(format!("{f:?}").contains("SHA256:abc123"));
    }
}