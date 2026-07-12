use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub status: i32,
}

/// Narrow seam around command execution, user prompts, time, and the local
/// network so that system-dependent CLI behavior can be faked in tests.
pub trait System {
    fn family(&self) -> &'static str;
    fn command(&self, program: &str, args: &[&str]) -> Result<CommandOutput, String>;
    fn prompt(&self, question: &str) -> Result<String, String>;
    fn now(&self) -> u64;
    fn tcp_reachable(&self, host: &str, port: u16, timeout: Duration) -> bool;
    fn route_address(&self) -> Option<String>;
    fn sleep(&self, duration: Duration);
    fn file_exists(&self, path: &str) -> bool;
}

/// Reads the SHA-256 fingerprint of the first available SSH host key.
///
/// Tries ed25519, ecdsa, then rsa — the host may not have all key types
/// generated, especially right after Remote Login is enabled on macOS.
pub fn host_key_fingerprint(system: &dyn System) -> Result<String, String> {
    for key_file in [
        "/etc/ssh/ssh_host_ed25519_key.pub",
        "/etc/ssh/ssh_host_ecdsa_key.pub",
        "/etc/ssh/ssh_host_rsa_key.pub",
    ] {
        if !system.file_exists(key_file) {
            continue;
        }
        let output = system
            .command("ssh-keygen", &["-lf", key_file, "-E", "sha256"])?;
        if output.status == 0 {
            return output
                .stdout
                .split_whitespace()
                .nth(1)
                .map(str::to_string)
                .ok_or_else(|| "invalid SSH host-key fingerprint".into());
        }
    }
    Err("could not read the SSH host-key fingerprint".into())
}

/// Verifies that an SSH listener is reachable on `127.0.0.1:port`.
///
/// On macOS, if the listener is not reachable the user is asked whether to
/// enable Remote Login. A "no" response fails without invoking `sudo`, and
/// a failed OS authorization fails without creating pairing credentials.
pub fn ensure_ssh_available(system: &dyn System, port: u16) -> Result<(), String> {
    if system.tcp_reachable("127.0.0.1", port, Duration::from_millis(300)) {
        return Ok(());
    }
    if system.family() != "macos" {
        return Err(format!("SSH is not listening on port {port}"));
    }
    let answer = system
        .prompt("macOS Remote Login appears disabled. Enable it now? [y/N] ")?
        .trim()
        .to_ascii_lowercase();
    if !matches!(answer.as_str(), "y" | "yes") {
        return Err("Remote Login is required for pairing".into());
    }
    let output = system.command("sudo", &["systemsetup", "-setremotelogin", "on"])?;
    if output.status != 0 {
        return Err("failed to enable Remote Login".into());
    }
    Ok(())
}

/// Waits for the pairing completion marker `done` to appear, the credential to
/// expire, or an injected cancellation signal to fire.
///
/// This is the injectable boundary behind the CLI's wait loop so tests can
/// drive timeout and interrupt cleanup without real sleeps or signals.
pub fn wait_for_completion(
    system: &dyn System,
    done: &std::path::Path,
    expires_at: u64,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    while system.now() < expires_at && !cancelled.load(Ordering::SeqCst) {
        if done.exists() {
            return Ok(());
        }
        system.sleep(Duration::from_millis(250));
    }
    if cancelled.load(Ordering::SeqCst) {
        Err("pairing cancelled; temporary access removed".into())
    } else {
        Err("pairing expired; temporary access removed".into())
    }
}
