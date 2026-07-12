use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

const QR_PREFIX: &str = "timu://pair?data=";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairingPayload {
    pub version: u8,
    pub pairing_id: String,
    pub machine_name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub host_key_fingerprint: String,
    pub expires_at_unix: u64,
    pub ephemeral_private_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayloadError {
    Invalid,
    Expired,
    UnsupportedVersion,
}

impl fmt::Display for PayloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid => formatter.write_str("invalid pairing payload"),
            Self::Expired => formatter.write_str("pairing payload has expired"),
            Self::UnsupportedVersion => formatter.write_str("unsupported pairing payload version"),
        }
    }
}

impl std::error::Error for PayloadError {}

pub struct CleanupGuard {
    root: PathBuf,
    pairing_id: String,
    authorized_keys: Option<PathBuf>,
}

impl CleanupGuard {
    pub fn new(root: PathBuf, pairing_id: String) -> Self {
        Self {
            root,
            pairing_id,
            authorized_keys: None,
        }
    }

    pub fn register_authorization(&mut self, authorized_keys: PathBuf) {
        self.authorized_keys = Some(authorized_keys);
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        if let Some(path) = &self.authorized_keys {
            let _ = remove_tagged_authorization(path, &self.pairing_id);
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn remove_tagged_authorization(path: &Path, pairing_id: &str) -> Result<(), PayloadError> {
    validate_pairing_id(pairing_id)?;
    mutate_authorized_keys(path, |current| {
        let marker = format!("timu-pair:{pairing_id}");
        let retained = current
            .lines()
            .filter(|line| line.split_whitespace().last() != Some(marker.as_str()))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(if retained.is_empty() {
            String::new()
        } else {
            format!("{retained}\n")
        })
    })
}

pub fn append_authorized_key_line(path: &Path, line: &str) -> Result<(), PayloadError> {
    validate_single_line(line)?;
    mutate_authorized_keys(path, |current| {
        let separator = if current.is_empty() || current.ends_with('\n') {
            ""
        } else {
            "\n"
        };
        Ok(format!("{current}{separator}{line}\n"))
    })
}

pub fn replace_temporary_authorized_key_in_file(
    path: &Path,
    pairing_id: &str,
    permanent_public_key: &str,
) -> Result<(), PayloadError> {
    mutate_authorized_keys(path, |current| {
        replace_temporary_authorized_key(current, pairing_id, permanent_public_key)
    })
}

fn mutate_authorized_keys<F>(path: &Path, mutate: F) -> Result<(), PayloadError>
where
    F: FnOnce(&str) -> Result<String, PayloadError>,
{
    reject_unsafe_authorized_keys_path(path)?;
    let directory = path.parent().ok_or(PayloadError::Invalid)?;
    let lock_path = directory.join(".timu-pair-authorized-keys.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(lock_path)
        .map_err(|_| PayloadError::Invalid)?;
    lock.lock().map_err(|_| PayloadError::Invalid)?;
    let result = (|| {
        reject_unsafe_authorized_keys_path(path)?;
        let current = fs::read_to_string(path).map_err(|_| PayloadError::Invalid)?;
        let updated = mutate(&current)?;
        atomic_write_authorized_keys(path, updated.as_bytes())
    })();
    let _ = lock.unlock();
    result
}

fn atomic_write_authorized_keys(path: &Path, contents: &[u8]) -> Result<(), PayloadError> {
    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let directory = path.parent().ok_or(PayloadError::Invalid)?;
    for _ in 0..32 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = directory.join(format!(
            ".timu-pair-authorized-keys-{}-{sequence}.tmp",
            std::process::id()
        ));
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(PayloadError::Invalid),
        };
        if file.write_all(contents).is_err() || file.sync_all().is_err() {
            let _ = fs::remove_file(temporary);
            return Err(PayloadError::Invalid);
        }
        if fs::rename(&temporary, path).is_err() {
            let _ = fs::remove_file(temporary);
            return Err(PayloadError::Invalid);
        }
        return Ok(());
    }
    Err(PayloadError::Invalid)
}

pub fn reject_unsafe_authorized_keys_path(path: &Path) -> Result<(), PayloadError> {
    let directory = path.parent().ok_or(PayloadError::Invalid)?;
    reject_symlink(directory)?;
    reject_symlink(path)
}

fn reject_symlink(path: &Path) -> Result<(), PayloadError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(PayloadError::Invalid),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(PayloadError::Invalid),
    }
}

/// Returns `true` when `now_unix` has reached or passed `expires_at_unix`.
///
/// This is the single source of truth for the expiry boundary so that the
/// QR decoder and the forced command agree on when a credential is expired.
pub fn is_expired(now_unix: u64, expires_at_unix: u64) -> bool {
    now_unix >= expires_at_unix
}

/// Captured result of a command run through [`System::command`].
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

/// Chooses the address the phone should use to reach this machine.
///
/// A single candidate is returned automatically. Multiple candidates are
/// presented through `prompt`, which receives the full menu text and must
/// return the user's answer.
pub fn choose_address<P>(candidates: Vec<AddressCandidate>, mut prompt: P) -> Result<String, String>
where
    P: FnMut(&str) -> Result<String, String>,
{
    if candidates.len() == 1 {
        return Ok(candidates[0].address.clone());
    }
    let mut menu = String::from("How should your phone reach this machine?\n\n");
    for (index, item) in candidates.iter().enumerate() {
        let kind = match item.kind {
            AddressKind::Wifi => "Wi-Fi",
            AddressKind::Ethernet => "Ethernet",
            AddressKind::Tailscale => "Tailscale",
        };
        menu.push_str(&format!("{}. {:<10} {}\n", index + 1, kind, item.address));
    }
    menu.push_str("\nEnter option number: ");
    let answer = prompt(&menu)?;
    let index = answer
        .trim()
        .parse::<usize>()
        .map_err(|_| "enter a valid option number".to_string())?;
    candidates
        .get(index.saturating_sub(1))
        .map(|item| item.address.clone())
        .ok_or_else(|| "option number is out of range".into())
}

/// Discovers Wi-Fi, Ethernet, and Tailscale address candidates using the
/// injectable [`System`] seam so tests can supply synthetic command output.
pub fn discover_addresses(system: &dyn System) -> Result<Vec<AddressCandidate>, String> {
    let mut found = if system.family() == "macos" {
        discover_macos_lan_addresses(system)
    } else {
        system
            .route_address()
            .map(|address| vec![AddressCandidate::new("eth0", &address)])
            .unwrap_or_default()
    };
    if let Ok(output) = system.command("tailscale", &["ip", "-4"]) {
        if output.status == 0 {
            for line in output.stdout.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    found.push(AddressCandidate::new("tailscale0", trimmed));
                }
            }
        }
    }
    found.sort_by(|a, b| a.address.cmp(&b.address));
    found.dedup_by(|a, b| a.address == b.address);
    if found.is_empty() {
        Err("no Wi-Fi, Ethernet, or Tailscale address found; use --host".into())
    } else {
        Ok(found)
    }
}

fn discover_macos_lan_addresses(system: &dyn System) -> Vec<AddressCandidate> {
    let Ok(output) = system.command("networksetup", &["-listallhardwareports"]) else {
        return Vec::new();
    };
    if output.status != 0 {
        return Vec::new();
    }
    let mut kind = None;
    let mut found = Vec::new();
    for line in output.stdout.lines() {
        if let Some(port) = line.strip_prefix("Hardware Port: ") {
            kind = if port.contains("Wi-Fi") {
                Some(AddressKind::Wifi)
            } else if port.contains("Ethernet") {
                Some(AddressKind::Ethernet)
            } else {
                None
            };
        } else if let Some(device) = line.strip_prefix("Device: ") {
            if let Some(_kind) = kind {
                if let Ok(address) = system.command("ipconfig", &["getifaddr", device]) {
                    if address.status == 0 {
                        let value = address.stdout.trim().to_string();
                        if !value.is_empty() {
                            found.push(AddressCandidate::new(device, &value));
                        }
                    }
                }
            }
        }
    }
    found
}

impl PairingPayload {
    pub fn encode_for_qr(&self) -> Result<String, PayloadError> {
        let json = serde_json::to_vec(self).map_err(|_| PayloadError::Invalid)?;
        Ok(format!("{QR_PREFIX}{}", URL_SAFE_NO_PAD.encode(json)))
    }

    pub fn decode_from_qr(value: &str, now_unix: u64) -> Result<Self, PayloadError> {
        let encoded = value.strip_prefix(QR_PREFIX).ok_or(PayloadError::Invalid)?;
        let json = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| PayloadError::Invalid)?;
        let payload: Self = serde_json::from_slice(&json).map_err(|_| PayloadError::Invalid)?;
        if payload.version != 1 {
            return Err(PayloadError::UnsupportedVersion);
        }
        if payload.port == 0
            || validate_pairing_id(&payload.pairing_id).is_err()
            || !is_canonical_host_key_fingerprint(&payload.host_key_fingerprint)
            || payload.machine_name.is_empty()
            || payload.host.is_empty()
            || payload.username.is_empty()
            || payload.host_key_fingerprint.is_empty()
            || payload.ephemeral_private_key.is_empty()
        {
            return Err(PayloadError::Invalid);
        }
        if is_expired(now_unix, payload.expires_at_unix) {
            return Err(PayloadError::Expired);
        }
        Ok(payload)
    }
}

pub fn build_temporary_authorized_key(
    pairing_id: &str,
    helper_path: &str,
    public_key: &str,
) -> Result<String, PayloadError> {
    validate_pairing_id(pairing_id)?;
    validate_single_line(helper_path)?;
    validate_public_key(public_key)?;
    Ok(format!(
        "command=\"{helper_path} {pairing_id}\",restrict,no-port-forwarding,no-agent-forwarding,no-X11-forwarding,no-pty {public_key} timu-pair:{pairing_id}"
    ))
}

pub fn replace_temporary_authorized_key(
    authorized_keys: &str,
    pairing_id: &str,
    permanent_public_key: &str,
) -> Result<String, PayloadError> {
    validate_pairing_id(pairing_id)?;
    validate_public_key(permanent_public_key)?;
    let marker = format!("timu-pair:{pairing_id}");
    let mut found = false;
    let mut output = String::new();
    for line in authorized_keys.lines() {
        if line.split_whitespace().last() == Some(marker.as_str()) {
            if found {
                return Err(PayloadError::Invalid);
            }
            found = true;
            output.push_str(permanent_public_key);
        } else {
            output.push_str(line);
        }
        output.push('\n');
    }
    if !found {
        return Err(PayloadError::Invalid);
    }
    Ok(output)
}

fn validate_pairing_id(value: &str) -> Result<(), PayloadError> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(PayloadError::Invalid);
    }
    Ok(())
}

fn is_canonical_host_key_fingerprint(value: &str) -> bool {
    let Some(encoded) = value.strip_prefix("SHA256:") else {
        return false;
    };
    let Ok(digest) = base64::engine::general_purpose::STANDARD_NO_PAD.decode(encoded) else {
        return false;
    };
    digest.len() == 32 && base64::engine::general_purpose::STANDARD_NO_PAD.encode(digest) == encoded
}

fn validate_single_line(value: &str) -> Result<(), PayloadError> {
    if value.is_empty() || value.contains(['\n', '\r', '"']) {
        return Err(PayloadError::Invalid);
    }
    Ok(())
}

fn validate_public_key(value: &str) -> Result<(), PayloadError> {
    validate_single_line(value)?;
    let mut fields = value.split_whitespace();
    if fields.next() != Some("ssh-ed25519") || fields.next().is_none() {
        return Err(PayloadError::Invalid);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOptions {
    pub host: Option<String>,
    pub username: Option<String>,
    pub port: u16,
}

impl CliOptions {
    pub fn parse<I, S>(args: I) -> Result<Self, PayloadError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let args: Vec<String> = args.into_iter().map(Into::into).collect();
        let mut options = Self {
            host: None,
            username: None,
            port: 22,
        };
        let mut index = 0;
        while index < args.len() {
            let target = match args[index].as_str() {
                "--host" => &mut options.host,
                "--user" => &mut options.username,
                "--port" => {
                    index += 1;
                    let port = args
                        .get(index)
                        .ok_or(PayloadError::Invalid)?
                        .parse::<u16>()
                        .map_err(|_| PayloadError::Invalid)?;
                    if port == 0 {
                        return Err(PayloadError::Invalid);
                    }
                    options.port = port;
                    index += 1;
                    continue;
                }
                _ => return Err(PayloadError::Invalid),
            };
            index += 1;
            let value = args.get(index).ok_or(PayloadError::Invalid)?;
            if value.is_empty() {
                return Err(PayloadError::Invalid);
            }
            *target = Some(value.clone());
            index += 1;
        }
        Ok(options)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressKind {
    Wifi,
    Ethernet,
    Tailscale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressCandidate {
    pub interface: String,
    pub address: String,
    pub kind: AddressKind,
}

impl AddressCandidate {
    pub fn new(interface: &str, address: &str) -> Self {
        Self {
            interface: interface.into(),
            address: address.into(),
            kind: classify_address(interface, address).unwrap_or(AddressKind::Ethernet),
        }
    }
}

pub fn select_address_candidates<I>(candidates: I) -> Vec<AddressCandidate>
where
    I: IntoIterator<Item = AddressCandidate>,
{
    candidates
        .into_iter()
        .filter_map(|mut candidate| {
            candidate.kind = classify_address(&candidate.interface, &candidate.address)?;
            Some(candidate)
        })
        .collect()
}

fn classify_address(interface: &str, address: &str) -> Option<AddressKind> {
    if address.starts_with("127.")
        || interface.starts_with("lo")
        || interface.starts_with("docker")
        || interface.starts_with("bridge")
        || interface.starts_with("veth")
    {
        return None;
    }
    if interface.to_ascii_lowercase().contains("tailscale") || is_tailscale_ipv4(address) {
        return Some(AddressKind::Tailscale);
    }
    if interface == "en0" || interface.starts_with("wl") {
        return Some(AddressKind::Wifi);
    }
    if interface.starts_with("en") || interface.starts_with("eth") {
        return Some(AddressKind::Ethernet);
    }
    None
}

fn is_tailscale_ipv4(address: &str) -> bool {
    let octets: Vec<u8> = address
        .split('.')
        .filter_map(|part| part.parse::<u8>().ok())
        .collect();
    octets.len() == 4 && octets[0] == 100 && (64..=127).contains(&octets[1])
}
