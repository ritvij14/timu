use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};

use crate::PayloadError;

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

/// Returns `true` when `now_unix` has reached or passed `expires_at_unix`.
///
/// This is the single source of truth for the expiry boundary so that the
/// QR decoder and the forced command agree on when a credential is expired.
pub fn is_expired(now_unix: u64, expires_at_unix: u64) -> bool {
    now_unix >= expires_at_unix
}

pub fn pairing_id_from_random_bytes(bytes: [u8; 16]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn validate_pairing_id(value: &str) -> Result<(), PayloadError> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(PayloadError::Invalid);
    }
    Ok(())
}

pub(crate) fn validate_single_line(value: &str) -> Result<(), PayloadError> {
    if value.is_empty() || value.contains(['\n', '\r', '"']) {
        return Err(PayloadError::Invalid);
    }
    Ok(())
}

pub(crate) fn validate_public_key(value: &str) -> Result<(), PayloadError> {
    validate_single_line(value)?;
    let mut fields = value.split_whitespace();
    if fields.next() != Some("ssh-ed25519") {
        return Err(PayloadError::Invalid);
    }
    let encoded = fields.next().ok_or(PayloadError::Invalid)?;
    if fields.any(|field| field.contains("PRIVATE") || field.contains("BEGIN")) {
        return Err(PayloadError::Invalid);
    }
    let blob = STANDARD
        .decode(encoded)
        .map_err(|_| PayloadError::Invalid)?;
    if !is_ed25519_public_key_blob(&blob) {
        return Err(PayloadError::Invalid);
    }
    Ok(())
}

fn is_ed25519_public_key_blob(blob: &[u8]) -> bool {
    let Some((kind, rest)) = read_ssh_string(blob) else {
        return false;
    };
    let Some((key, trailing)) = read_ssh_string(rest) else {
        return false;
    };
    kind == b"ssh-ed25519" && key.len() == 32 && trailing.is_empty()
}

fn read_ssh_string(bytes: &[u8]) -> Option<(&[u8], &[u8])> {
    let length = u32::from_be_bytes(bytes.get(..4)?.try_into().ok()?) as usize;
    let value = bytes.get(4..4 + length)?;
    let rest = bytes.get(4 + length..)?;
    Some((value, rest))
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
