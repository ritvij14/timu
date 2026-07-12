use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::PayloadError;
use crate::payload::{validate_pairing_id, validate_public_key, validate_single_line};

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
        if let Some(marker) = pairing_marker(line)
            && current.contains(marker)
        {
            return Err(PayloadError::Invalid);
        }
        let separator = if current.is_empty() || current.ends_with('\n') {
            ""
        } else {
            "\n"
        };
        Ok(format!("{current}{separator}{line}\n"))
    })
}

fn pairing_marker(line: &str) -> Option<&str> {
    let start = line.find("timu-pair:")?;
    line[start..].split_whitespace().next()
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
    let mode = fs::metadata(path)
        .map_err(|_| PayloadError::Invalid)?
        .permissions()
        .mode()
        & 0o777;
    for _ in 0..32 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = directory.join(format!(
            ".timu-pair-authorized-keys-{}-{sequence}.tmp",
            std::process::id()
        ));
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
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
    reject_symlink(path)?;
    if let Ok(metadata) = fs::metadata(path)
        && metadata.permissions().mode() & 0o022 != 0
    {
        return Err(PayloadError::Invalid);
    }
    Ok(())
}

fn reject_symlink(path: &Path) -> Result<(), PayloadError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(PayloadError::Invalid),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(PayloadError::Invalid),
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
    let command = format!("{} {}", shell_quote(helper_path), shell_quote(pairing_id));
    Ok(format!(
        "command=\"{command}\",restrict,no-port-forwarding,no-agent-forwarding,no-X11-forwarding,no-pty {public_key} timu-pair:{pairing_id}"
    ))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
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
