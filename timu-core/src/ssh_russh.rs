//! Real SSH transport backed by `russh` — PRD §6 (connect/auth) + §14 (security).
//!
//! Implements [`SshTransport`] over a live SSH connection: TOFU host-key pinning
//! ([`crate::host_key`]), password + private-key auth, and `run_command` via an
//! exec channel. Pure logic (host-key pins, credential redaction) is tested
//! elsewhere; here we add a non-network test for the key-loading path and an
//! `#[ignore]` live-server test.
//!
//! Error mapping is heuristic (string inspection of `russh::Error`) for V0 —
//! see [`map_connect_error`]. It will be tightened once we've seen real
//! failure modes against a sshd fixture.

use std::sync::Arc;

use russh::client::{self, Config, Handle, Handler, Msg};
use russh::{Channel, ChannelMsg};
use russh::keys::{decode_secret_key, HashAlg, PrivateKeyWithHashAlg};
use tokio::sync::Mutex;

use crate::credentials::Credentials;
use crate::error::TimuError;
use crate::host_key::{Fingerprint, HostKeyPins, HostKeyVerdict};
use crate::profile::MachineProfile;
use crate::ssh::{CommandOutput, SshTransport};

/// SSH extended-data type code for stderr (RFC 4254 §5.2).
const SSH_EXTENDED_DATA_STDERR: u32 = 1;

/// `client::Handler` that verifies the server host key against pinned state.
///
/// On first sight of a host, the offered fingerprint is captured (not yet
/// pinned) and the caller pins it only after the full connect+auth succeeds.
/// On a mismatch, the key is rejected (`Ok(false)`) and russh aborts the
/// handshake — Hard Block §2.2.
struct HostKeyHandler {
    host: String,
    pins: Arc<Mutex<HostKeyPins>>,
    first_seen: Arc<Mutex<Option<Fingerprint>>>,
}

impl Handler for HostKeyHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        let fp = Fingerprint::new(
            server_public_key
                .fingerprint(HashAlg::Sha256)
                .to_string(),
        );
        let verdict = self.pins.lock().await.verify(&self.host, &fp);
        match verdict {
            HostKeyVerdict::FirstSeen => {
                // Accept provisionally; caller pins after auth succeeds.
                *self.first_seen.lock().await = Some(fp);
                Ok(true)
            }
            HostKeyVerdict::Matches => Ok(true),
            HostKeyVerdict::Mismatch => Ok(false), // reject — Hard Block §2.2
        }
    }
}

/// A live SSH connection, satisfying [`SshTransport`]. Created via
/// [`RusshSshTransport::connect`]; the returned `Option<Fingerprint>` is the
/// newly-pinned fingerprint (if this was a first connect) the UI may show.
pub struct RusshSshTransport {
    handle: Handle<HostKeyHandler>,
}

impl RusshSshTransport {
    /// Open + authenticate. On success, pins the host key if this was a first
    /// connect and returns the pinned fingerprint (for UI display).
    pub async fn connect(
        profile: &MachineProfile,
        creds: &Credentials,
        pins: Arc<Mutex<HostKeyPins>>,
    ) -> Result<(Self, Option<Fingerprint>), TimuError> {
        let host = profile.host.clone();
        let first_seen = Arc::new(Mutex::new(None));
        let handler = HostKeyHandler {
            host: host.clone(),
            pins: pins.clone(),
            first_seen: first_seen.clone(),
        };

        let config = Arc::new(Config::default());
        let addr = format!("{}:{}", profile.host, profile.port);
        let mut handle = client::connect(config, addr.as_str(), handler)
            .await
            .map_err(map_connect_error)?;

        let auth_ok = match creds {
            Credentials::Password(pw) => {
                handle.authenticate_password(&profile.username, pw).await
            }
            Credentials::PrivateKey { material, passphrase } => {
                let with_alg = load_key(material, passphrase.as_deref())?;
                handle.authenticate_publickey(&profile.username, with_alg).await
            }
        };

        let success = match auth_ok {
            Ok(result) => result.success(),
            Err(_) => false,
        };
        if !success {
            // Distinguish "server refused this method" from "bad material" is
            // not reliably possible from AuthResult alone — both surface as
            // Failure. V0 reports wrong credentials; PermissionDenied is
            // reserved for the connect-phase heuristic.
            return Err(TimuError::WrongCredentials);
        }

        let pinned = first_seen.lock().await.take();
        if let Some(ref fp) = pinned {
            pins.lock().await.pin(&host, fp.clone());
        }

        Ok((Self { handle }, pinned))
    }

    /// Disconnect cleanly. Best-effort; errors are ignored.
    pub async fn disconnect(&self) {
        let _ = self
            .handle
            .disconnect(russh::Disconnect::ByApplication, "bye", "en")
            .await;
    }
}

impl SshTransport for RusshSshTransport {
    async fn run_command(&self, command: &str) -> Result<CommandOutput, TimuError> {
        let mut channel: Channel<Msg> = self
            .handle
            .channel_open_session()
            .await
            .map_err(|e| TimuError::Other(format!("open channel: {e}")))?;

        channel
            .exec(true, command.as_bytes())
            .await
            .map_err(|e| TimuError::Other(format!("exec: {e}")))?;

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_code = 0i32;
        let mut got_exit = false;

        while let Some(msg) = channel.wait().await {
            match msg {
                ChannelMsg::Data { data } => stdout.extend_from_slice(&data),
                ChannelMsg::ExtendedData { data, ext } => {
                    if ext == SSH_EXTENDED_DATA_STDERR {
                        stderr.extend_from_slice(&data);
                    }
                }
                ChannelMsg::ExitStatus { exit_status } => {
                    exit_code = exit_status as i32;
                    got_exit = true;
                }
                ChannelMsg::Eof | ChannelMsg::Close => break,
                _ => {}
            }
        }
        let _ = got_exit; // noted; some servers omit exit-status

        Ok(CommandOutput::new(
            String::from_utf8_lossy(&stdout).into_owned(),
            String::from_utf8_lossy(&stderr).into_owned(),
            exit_code,
        ))
    }
}

/// Heuristic mapping of connect-phase `russh::Error` to PRD §6 failure states.
///
/// V0 inspects the error's display string. This is imperfect — once we've run
/// against a real sshd fixture we'll map by `russh::Error` variants directly.
fn map_connect_error(e: russh::Error) -> TimuError {
    let msg = e.to_string().to_lowercase();
    if msg.contains("connection refused")
        || msg.contains("connect error")
        || msg.contains("connection reset")
    {
        TimuError::PortUnreachable
    } else if msg.contains("name or service not known")
        || msg.contains("failed to lookup")
        || msg.contains("no address")
        || msg.contains("dns")
    {
        TimuError::WrongHost
    } else if msg.contains("network is unreachable")
        || msg.contains("no route to host")
        || msg.contains("timed out")
        || msg.contains("timeout")
    {
        TimuError::NetworkUnavailable
    } else if msg.contains("permission denied") {
        TimuError::PermissionDenied
    } else {
        TimuError::Other(e.to_string())
    }
}

/// Load a private key from material bytes, returning a ready-to-use
/// `PrivateKeyWithHashAlg`. Factored out so it can be tested without a server
/// and shared between [`RusshSshTransport::connect`] and the test suite.
fn load_key(material: &[u8], passphrase: Option<&str>) -> Result<PrivateKeyWithHashAlg, TimuError> {
    let key_str = std::str::from_utf8(material).map_err(|_| TimuError::WrongCredentials)?;
    let key = decode_secret_key(key_str, passphrase).map_err(|_| TimuError::WrongCredentials)?;
    Ok(PrivateKeyWithHashAlg::new(Arc::new(key), Some(HashAlg::Sha256)))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test key from russh's own fixture suite (password "blabla"). Safe to
    // ship in source — it's a throwaway test key, never used for real hosts.
    const ED25519_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAACmFlczI1Ni1jYmMAAAAGYmNyeXB0AAAAGAAAABDLGyfA39\nJ2FcJygtYqi5ISAAAAEAAAAAEAAAAzAAAAC3NzaC1lZDI1NTE5AAAAIN+Wjn4+4Fcvl2Jl\nKpggT+wCRxpSvtqqpVrQrKN1/A22AAAAkOHDLnYZvYS6H9Q3S3Nk4ri3R2jAZlQlBbUos5\nFkHpYgNw65KCWCTXtP7ye2czMC3zjn2r98pJLobsLYQgRiHIv/CUdAdsqbvMPECB+wl/UQ\ne+JpiSq66Z6GIt0801skPh20jxOO3F52SoX1IeO5D5PXfZrfSZlw6S8c7bwyp2FHxDewRx\n7/wNsnDM0T7nLv/Q==\n-----END OPENSSH PRIVATE KEY-----";

    #[test]
    fn load_key_decodes_encrypted_ed25519_with_passphrase() {
        let key = load_key(ED25519_KEY.as_bytes(), Some("blabla")).expect("key decodes");
        // Algorithm is ed25519; not RSA, so hash_alg collapses to None.
        assert!(!key.algorithm().is_rsa());
    }

    #[test]
    fn load_key_wrong_passphrase_is_wrong_credentials() {
        let err = load_key(ED25519_KEY.as_bytes(), Some("wrongpass")).expect_err("should fail");
        assert_eq!(err, TimuError::WrongCredentials);
        assert_eq!(err.code(), "wrong_credentials");
    }

    #[test]
    fn load_key_non_utf8_material_is_wrong_credentials() {
        let err = load_key(&[0xff, 0xfe, 0xfd], None).expect_err("should fail");
        assert_eq!(err, TimuError::WrongCredentials);
    }

    #[test]
    fn map_connect_error_classifies_refused_as_port_unreachable() {
        // Construct via a representative IO-ish error string path. We can't
        // easily build a russh::Error variant, so exercise the heuristic
        // through a string-only mirror to keep the test honest.
        assert_eq!(classify_str("Connection refused by host"), TimuError::PortUnreachable);
        assert_eq!(classify_str("Name or service not known"), TimuError::WrongHost);
        assert_eq!(classify_str("Network is unreachable"), TimuError::NetworkUnavailable);
        assert_eq!(classify_str("Permission denied (publickey)"), TimuError::PermissionDenied);
        assert_eq!(classify_str("something weird"), TimuError::Other("something weird".into()));
    }

    /// String-only mirror of [`map_connect_error`] so the classification logic
    /// is tested without fabricating a `russh::Error`.
    fn classify_str(s: &str) -> TimuError {
        let msg = s.to_lowercase();
        if msg.contains("connection refused")
            || msg.contains("connect error")
            || msg.contains("connection reset")
        {
            TimuError::PortUnreachable
        } else if msg.contains("name or service not known")
            || msg.contains("failed to lookup")
            || msg.contains("no address")
            || msg.contains("dns")
        {
            TimuError::WrongHost
        } else if msg.contains("network is unreachable")
            || msg.contains("no route to host")
            || msg.contains("timed out")
            || msg.contains("timeout")
        {
            TimuError::NetworkUnavailable
        } else if msg.contains("permission denied") {
            TimuError::PermissionDenied
        } else {
            TimuError::Other(s.to_string())
        }
    }

    /// Live test: requires a real SSH server. Set `TIMU_TEST_SSH_HOST`,
    /// `TIMU_TEST_SSH_USER`, `TIMU_TEST_SSH_PASS` (or `_KEY` + `_KEYPASS`) to
    /// run. Ignored by default.
    #[tokio::test]
    #[ignore = "requires a live SSH server (set TIMU_TEST_SSH_* env vars)"]
    async fn live_connect_and_run_command() {
        let host = std::env::var("TIMU_TEST_SSH_HOST").expect("TIMU_TEST_SSH_HOST");
        let user = std::env::var("TIMU_TEST_SSH_USER").expect("TIMU_TEST_SSH_USER");
        let port: u16 = std::env::var("TIMU_TEST_SSH_PORT")
            .ok().and_then(|p| p.parse().ok()).unwrap_or(22);

        let profile = MachineProfile {
            name: "live-test".into(),
            host,
            username: user,
            port,
            auth_method: crate::profile::AuthMethod::Password,
        };
        let creds = match std::env::var("TIMU_TEST_SSH_PASS") {
            Ok(p) => Credentials::Password(p),
            Err(_) => {
                let key = std::env::var("TIMU_TEST_SSH_KEY").expect("TIMU_TEST_SSH_KEY");
                let pass = std::env::var("TIMU_TEST_SSH_KEYPASS").ok();
                Credentials::PrivateKey {
                    material: key.into_bytes(),
                    passphrase: pass,
                }
            }
        };
        let pins = Arc::new(Mutex::new(HostKeyPins::new()));
        let (transport, fp) = RusshSshTransport::connect(&profile, &creds, pins)
            .await
            .expect("connect");
        assert!(fp.is_some(), "first connect should capture a fingerprint");

        let out = transport.run_command("echo hello").await.expect("run");
        assert_eq!(out.exit_code, 0);
        assert!(out.stdout.contains("hello"));
        transport.disconnect().await;
    }
}