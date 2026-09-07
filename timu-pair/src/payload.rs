use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::io::Write;

use crate::PayloadError;

/// Matches the completion command's stdin bound; a decompressed QR payload
/// larger than this is a decompression bomb, not a pairing payload.
const MAX_QR_PAYLOAD_JSON_BYTES: usize = 8192;

const OPENSSH_KEY_MAGIC: &[u8] = b"openssh-key-v1\0";

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
    pub fn encode_for_qr(&self) -> Result<Vec<u8>, PayloadError> {
        let json = serde_json::to_vec(self).map_err(|_| PayloadError::Invalid)?;
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(&json)
            .map_err(|_| PayloadError::Invalid)?;
        encoder.finish().map_err(|_| PayloadError::Invalid)
    }

    pub fn decode_from_qr(value: &[u8], now_unix: u64) -> Result<Self, PayloadError> {
        let mut json = Vec::new();
        ZlibDecoder::new(value)
            .take(MAX_QR_PAYLOAD_JSON_BYTES as u64 + 1)
            .read_to_end(&mut json)
            .map_err(|_| PayloadError::Invalid)?;
        if json.len() > MAX_QR_PAYLOAD_JSON_BYTES {
            return Err(PayloadError::Invalid);
        }
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
            || !is_base64_ed25519_seed(&payload.ephemeral_private_key)
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

fn is_base64_ed25519_seed(value: &str) -> bool {
    STANDARD.decode(value).is_ok_and(|bytes| bytes.len() == 32)
}

/// Extracts the 32-byte Ed25519 seed from an unencrypted OpenSSH private key
/// (`openssh-key-v1` format, the `ssh-keygen` default).
///
/// The seed is the first 32 bytes of the 64-byte private field
/// (seed || public). Reconstructing the OpenSSH keypair from this seed is the
/// phone client's responsibility; the CLI ships only the seed.
pub fn ed25519_seed_from_openssh_private_key(pem: &str) -> Result<[u8; 32], PayloadError> {
    let body: String = pem.lines().filter(|line| !line.starts_with("-----")).collect();
    let blob = STANDARD
        .decode(body.trim())
        .map_err(|_| PayloadError::Invalid)?;

    let rest = blob
        .get(OPENSSH_KEY_MAGIC.len()..)
        .filter(|_| blob.starts_with(OPENSSH_KEY_MAGIC))
        .ok_or(PayloadError::Invalid)?;
    let (_, rest) = read_ssh_string(rest).ok_or(PayloadError::Invalid)?; // cipher
    let (_, rest) = read_ssh_string(rest).ok_or(PayloadError::Invalid)?; // kdf
    let (_, rest) = read_ssh_string(rest).ok_or(PayloadError::Invalid)?; // kdfoptions
    let key_count = u32::from_be_bytes(rest.get(..4).ok_or(PayloadError::Invalid)?.try_into().ok().ok_or(PayloadError::Invalid)?);
    if key_count != 1 {
        return Err(PayloadError::Invalid);
    }
    let (_, rest) = read_ssh_string(&rest[4..]).ok_or(PayloadError::Invalid)?; // public key
    let (private_section, _) = read_ssh_string(rest).ok_or(PayloadError::Invalid)?;

    let check = u32::from_be_bytes(private_section.get(..4).ok_or(PayloadError::Invalid)?.try_into().ok().ok_or(PayloadError::Invalid)?);
    let check_repeat = u32::from_be_bytes(private_section.get(4..8).ok_or(PayloadError::Invalid)?.try_into().ok().ok_or(PayloadError::Invalid)?);
    if check != check_repeat {
        return Err(PayloadError::Invalid);
    }
    let (key_type, rest) = read_ssh_string(&private_section[8..]).ok_or(PayloadError::Invalid)?;
    if key_type != b"ssh-ed25519" {
        return Err(PayloadError::Invalid);
    }
    let (_public, rest) = read_ssh_string(rest).ok_or(PayloadError::Invalid)?;
    let (private, _) = read_ssh_string(rest).ok_or(PayloadError::Invalid)?;
    let seed: [u8; 32] = private
        .get(..32)
        .and_then(|seed| seed.try_into().ok())
        .ok_or(PayloadError::Invalid)?;
    Ok(seed)
}
