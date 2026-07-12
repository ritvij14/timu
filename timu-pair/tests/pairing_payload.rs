use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use timu_pair::{PairingPayload, PayloadError, is_expired};

fn valid_payload() -> PairingPayload {
    PairingPayload {
        version: 1,
        pairing_id: "pair-123".into(),
        machine_name: "dev-mac".into(),
        host: "192.168.1.20".into(),
        port: 22,
        username: "ritvij".into(),
        host_key_fingerprint: "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into(),
        expires_at_unix: 1_800_000_000,
        ephemeral_private_key: "test-private-key".into(),
    }
}

#[test]
fn qr_payload_round_trips_every_required_pairing_field() {
    let payload = valid_payload();

    let encoded = payload.encode_for_qr().expect("valid payload encodes");
    let decoded =
        PairingPayload::decode_from_qr(&encoded, 1_700_000_000).expect("unexpired payload decodes");

    assert_eq!(decoded, payload);
    assert!(encoded.starts_with("timu://pair?data="));
}

#[test]
fn qr_payload_rejects_expired_credentials() {
    let payload = valid_payload();
    let encoded = payload.encode_for_qr().expect("valid payload encodes");

    let error = PairingPayload::decode_from_qr(&encoded, 1_900_000_000)
        .expect_err("expired payload must fail closed");

    assert_eq!(error, PayloadError::Expired);
}

#[test]
fn qr_payload_rejects_unsupported_versions_and_missing_security_fields() {
    let mut unsupported = valid_payload();
    unsupported.version = 2;
    let unsupported = unsupported.encode_for_qr().expect("payload encodes");
    assert_eq!(
        PairingPayload::decode_from_qr(&unsupported, 1_700_000_000),
        Err(PayloadError::UnsupportedVersion)
    );

    let mut missing_fingerprint = valid_payload();
    missing_fingerprint.host_key_fingerprint.clear();
    let missing_fingerprint = missing_fingerprint
        .encode_for_qr()
        .expect("payload encodes");
    assert_eq!(
        PairingPayload::decode_from_qr(&missing_fingerprint, 1_700_000_000),
        Err(PayloadError::Invalid)
    );
}

#[test]
fn qr_payload_rejects_unknown_json_fields() {
    // V1 protocol: unknown fields MUST be rejected, not silently ignored.
    let json = r#"{"version":1,"pairing_id":"pair-123","machine_name":"dev-mac","host":"192.168.1.20","port":22,"username":"ritvij","host_key_fingerprint":"SHA256:abc123","expires_at_unix":1800000000,"ephemeral_private_key":"test-key","evil":"malicious"}"#;
    let encoded = format!("timu://pair?data={}", URL_SAFE_NO_PAD.encode(json));
    assert_eq!(
        PairingPayload::decode_from_qr(&encoded, 1_700_000_000),
        Err(PayloadError::Invalid)
    );
}

#[test]
fn qr_payload_rejects_zero_port_invalid_pairing_id_and_noncanonical_host_fingerprint() {
    let mut zero_port = valid_payload();
    zero_port.port = 0;
    let zero_port = zero_port.encode_for_qr().expect("payload encodes");
    assert_eq!(
        PairingPayload::decode_from_qr(&zero_port, 1_700_000_000),
        Err(PayloadError::Invalid)
    );

    let mut invalid_pairing_id = valid_payload();
    invalid_pairing_id.pairing_id = "pair id".into();
    let invalid_pairing_id = invalid_pairing_id.encode_for_qr().expect("payload encodes");
    assert_eq!(
        PairingPayload::decode_from_qr(&invalid_pairing_id, 1_700_000_000),
        Err(PayloadError::Invalid)
    );

    let mut noncanonical_fingerprint = valid_payload();
    noncanonical_fingerprint.host_key_fingerprint = "SHA256:abc123=".into();
    let noncanonical_fingerprint = noncanonical_fingerprint
        .encode_for_qr()
        .expect("payload encodes");
    assert_eq!(
        PairingPayload::decode_from_qr(&noncanonical_fingerprint, 1_700_000_000),
        Err(PayloadError::Invalid)
    );
}

#[test]
fn expiry_boundary_is_consistent_between_decode_and_forced_command() {
    // Both decode and forced command must reject at now == expires_at.
    assert!(is_expired(1_800_000_000, 1_800_000_000));
    assert!(!is_expired(1_799_999_999, 1_800_000_000));
}
