use serde_json::Value;
use timu_pair::{PairingPayload, PayloadError};

const SCHEMA: &str =
    include_str!("../../docs/features/onboarding-cli/pairing-payload-v1.schema.json");
const VALID: &str = include_str!("../../docs/features/onboarding-cli/fixtures/valid-v1.json");
const EXPIRED: &str = include_str!("../../docs/features/onboarding-cli/fixtures/expired-v1.json");
const UNSUPPORTED: &str =
    include_str!("../../docs/features/onboarding-cli/fixtures/unsupported-v2.json");
const MALFORMED: &str =
    include_str!("../../docs/features/onboarding-cli/fixtures/malformed-v1.json");

fn qr_from_fixture(fixture: &str) -> String {
    serde_json::from_str::<PairingPayload>(fixture)
        .expect("fixture matches the Rust payload shape")
        .encode_for_qr()
        .expect("fixture encodes")
}

#[test]
fn v1_schema_requires_exactly_the_stable_pairing_fields() {
    let schema: Value = serde_json::from_str(SCHEMA).expect("schema is valid JSON");
    let required = schema["required"].as_array().expect("required is an array");
    let names: Vec<&str> = required.iter().filter_map(Value::as_str).collect();

    assert_eq!(
        names,
        [
            "version",
            "pairing_id",
            "machine_name",
            "host",
            "port",
            "username",
            "host_key_fingerprint",
            "expires_at_unix",
            "ephemeral_private_key",
        ]
    );
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["properties"]["version"]["const"], 1);
    assert_eq!(schema["properties"]["port"]["minimum"], 1);
    assert_eq!(schema["properties"]["port"]["maximum"], 65535);
}

#[test]
fn valid_v1_fixture_decodes_with_every_stable_field() {
    let payload = PairingPayload::decode_from_qr(&qr_from_fixture(VALID), 1_700_000_000)
        .expect("valid fixture decodes");

    assert_eq!(payload.version, 1);
    assert_eq!(payload.pairing_id, "pair-fixture-001");
    assert_eq!(payload.machine_name, "example-mac");
    assert_eq!(payload.host, "192.0.2.10");
    assert_eq!(payload.port, 22);
    assert_eq!(payload.username, "developer");
    assert_eq!(
        payload.host_key_fingerprint,
        "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    );
    assert_eq!(payload.expires_at_unix, 1_700_000_300);
    assert!(
        payload
            .ephemeral_private_key
            .contains("OPENSSH PRIVATE KEY")
    );
}

#[test]
fn expired_and_unsupported_fixtures_have_distinct_outcomes() {
    assert_eq!(
        PairingPayload::decode_from_qr(&qr_from_fixture(EXPIRED), 1_700_000_300),
        Err(PayloadError::Expired)
    );
    assert_eq!(
        PairingPayload::decode_from_qr(&qr_from_fixture(UNSUPPORTED), 1_700_000_000),
        Err(PayloadError::UnsupportedVersion)
    );
}

#[test]
fn malformed_fixture_cannot_deserialize_as_a_pairing_payload() {
    assert!(serde_json::from_str::<PairingPayload>(MALFORMED).is_err());
}
