use timu_pair::{build_temporary_authorized_key, replace_temporary_authorized_key};

const VALID_ED25519_PUBLIC_KEY: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4f";

#[test]
fn temporary_authorization_is_forced_and_disables_ssh_forwarding_and_pty() {
    let line = build_temporary_authorized_key(
        "pair-123",
        "/private/tmp/timu-pair-helper",
        VALID_ED25519_PUBLIC_KEY,
    )
    .expect("valid temporary key");

    assert!(line.contains("command=\"'/private/tmp/timu-pair-helper' 'pair-123'\""));
    assert!(line.contains("restrict"));
    assert!(line.contains("no-port-forwarding"));
    assert!(line.contains("no-agent-forwarding"));
    assert!(line.contains("no-X11-forwarding"));
    assert!(line.contains("no-pty"));
    assert!(line.ends_with("timu-pair:pair-123"));
}

#[test]
fn temporary_authorization_shell_quotes_the_forced_command() {
    let line = build_temporary_authorized_key(
        "pair-123",
        "/tmp/timu pair; touch /tmp/pwn/helper",
        VALID_ED25519_PUBLIC_KEY,
    )
    .expect("shell metacharacters are quoted, not rejected");

    assert!(line.contains("command=\"'/tmp/timu pair; touch /tmp/pwn/helper' 'pair-123'\""));
}

#[test]
fn temporary_authorization_with_command_quotes_can_be_appended_to_authorized_keys() {
    let root = std::env::temp_dir().join(format!(
        "timu-pair-test-quote-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let authorized_keys = root.join("authorized_keys");
    std::fs::write(&authorized_keys, "ssh-ed25519 AAAAEXISTING laptop\n").unwrap();

    let line = build_temporary_authorized_key(
        "pair-123",
        "/tmp/timu-pair-helper",
        VALID_ED25519_PUBLIC_KEY,
    )
    .expect("valid temporary key");

    assert!(line.contains("command=\""));

    timu_pair::append_authorized_key_line(&authorized_keys, &line)
        .expect("line with command quotes should be appendable");

    let contents = std::fs::read_to_string(&authorized_keys).unwrap();
    assert!(contents.contains("ssh-ed25519 AAAAEXISTING laptop"));
    assert!(contents.contains("timu-pair:pair-123"));
    assert!(contents.contains("command=\""));

    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn append_rejects_multiline_input() {
    let root = std::env::temp_dir().join(format!(
        "timu-pair-test-multiline-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let authorized_keys = root.join("authorized_keys");
    std::fs::write(&authorized_keys, "ssh-ed25519 AAAAEXISTING laptop\n").unwrap();

    let error = timu_pair::append_authorized_key_line(
        &authorized_keys,
        "ssh-ed25519 AAAAKEY timu-pair:pair-x\nssh-ed25519 AAAAINJECTED attacker",
    )
    .expect_err("multiline input should be rejected");

    assert_eq!(error.to_string(), "invalid pairing payload");

    let contents = std::fs::read_to_string(&authorized_keys).unwrap();
    assert!(!contents.contains("INJECTED"));

    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn permanent_key_handoff_replaces_only_the_matching_temporary_entry() {
    let existing = concat!(
        "ssh-ed25519 AAAAEXISTING laptop\n",
        "restrict ssh-ed25519 AAAATEMP timu-pair:pair-123\n",
        "restrict ssh-ed25519 AAAAOTHER timu-pair:pair-other\n",
    );

    let updated = replace_temporary_authorized_key(
        existing,
        "pair-123",
        &format!("{VALID_ED25519_PUBLIC_KEY} timu-device:iphone"),
    )
    .expect("valid device key replaces pairing entry");

    assert!(updated.contains("ssh-ed25519 AAAAEXISTING laptop\n"));
    assert!(updated.contains(&format!("{VALID_ED25519_PUBLIC_KEY} timu-device:iphone\n")));
    assert!(updated.contains("timu-pair:pair-other\n"));
    assert!(!updated.contains("timu-pair:pair-123"));
}

#[test]
fn permanent_key_handoff_rejects_injected_lines_and_missing_pair_ids() {
    assert!(
        replace_temporary_authorized_key(
            "restrict ssh-ed25519 AAAATEMP timu-pair:pair-123\n",
            "pair-123",
            "ssh-ed25519 AAAADEVICE ok\ncommand=\"sh\" ssh-rsa BAD",
        )
        .is_err()
    );

    assert!(
        replace_temporary_authorized_key(
            "ssh-ed25519 AAAAEXISTING laptop\n",
            "pair-123",
            &format!("{VALID_ED25519_PUBLIC_KEY} timu-device:iphone"),
        )
        .is_err()
    );
}

#[test]
fn permanent_key_handoff_rejects_invalid_ed25519_key_material_without_consuming_pairing() {
    let existing = "restrict ssh-ed25519 AAAATEMP timu-pair:pair-123\n";

    let error = replace_temporary_authorized_key(
        existing,
        "pair-123",
        "ssh-ed25519 not-a-valid-key timu-device:iphone",
    )
    .expect_err("invalid device public-key blob must fail closed");

    assert_eq!(error.to_string(), "invalid pairing payload");
}

#[test]
fn permanent_key_handoff_rejects_private_key_material_in_comment() {
    let existing = "restrict ssh-ed25519 AAAATEMP timu-pair:pair-123\n";

    assert!(
        replace_temporary_authorized_key(
            existing,
            "pair-123",
            &format!("{VALID_ED25519_PUBLIC_KEY} -----BEGIN OPENSSH PRIVATE KEY-----"),
        )
        .is_err()
    );
}
