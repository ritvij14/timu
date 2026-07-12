use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const VALID_ED25519_PUBLIC_KEY: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4f";

static TEST_ID: AtomicU64 = AtomicU64::new(0);

fn isolated_pairing() -> (PathBuf, PathBuf, PathBuf) {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let sequence = TEST_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "timu-pair-test-{}-{unique}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    let authorized_keys = root.join("authorized_keys");
    let done = root.join("done");
    fs::write(
        &authorized_keys,
        "ssh-ed25519 AAAAOLD laptop\nrestrict ssh-ed25519 AAAATEMP timu-pair:pair-123\n",
    )
    .unwrap();
    (root, authorized_keys, done)
}

fn complete(authorized_keys: &Path, done: &Path, expires_at: &str, device_key: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_timu-pair"))
        .args([
            "--complete",
            "pair-123",
            authorized_keys.to_str().unwrap(),
            done.to_str().unwrap(),
            expires_at,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(device_key.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn scanning_a_valid_qr_installs_the_iphone_key_without_touching_existing_access() {
    let (root, authorized_keys, done) = isolated_pairing();

    let output = complete(
        &authorized_keys,
        &done,
        "4102444800",
        &format!("{VALID_ED25519_PUBLIC_KEY} timu-device:iphone\n"),
    );

    assert!(output.status.success());
    let contents = fs::read_to_string(&authorized_keys).unwrap();
    assert!(contents.contains("ssh-ed25519 AAAAOLD laptop"));
    assert!(contents.contains(&format!("{VALID_ED25519_PUBLIC_KEY} timu-device:iphone")));
    assert!(!contents.contains("timu-pair:pair-123"));
    assert!(done.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scanning_an_expired_qr_cannot_change_ssh_access() {
    let (root, authorized_keys, done) = isolated_pairing();
    let before = fs::read_to_string(&authorized_keys).unwrap();

    let output = complete(
        &authorized_keys,
        &done,
        "1",
        &format!("{VALID_ED25519_PUBLIC_KEY} timu-device:iphone\n"),
    );

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("pairing credential has expired"));
    assert_eq!(fs::read_to_string(&authorized_keys).unwrap(), before);
    assert!(!done.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn malformed_device_keys_fail_without_removing_temporary_access() {
    let (root, authorized_keys, done) = isolated_pairing();
    let before = fs::read_to_string(&authorized_keys).unwrap();

    let output = complete(
        &authorized_keys,
        &done,
        "4102444800",
        "ssh-rsa AAAAUNEXPECTED timu-device:iphone\n",
    );

    assert!(!output.status.success());
    assert_eq!(fs::read_to_string(&authorized_keys).unwrap(), before);
    assert!(!done.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_pairing_credential_is_single_use_and_cannot_install_a_second_key() {
    let (root, authorized_keys, done) = isolated_pairing();
    assert!(
        complete(
            &authorized_keys,
            &done,
            "4102444800",
            &format!("{VALID_ED25519_PUBLIC_KEY} timu-device:iphone\n"),
        )
        .status
        .success()
    );
    let after_first = fs::read_to_string(&authorized_keys).unwrap();

    let replay = complete(
        &authorized_keys,
        &done,
        "4102444800",
        &format!("{VALID_ED25519_PUBLIC_KEY} timu-device:attacker\n"),
    );

    assert!(!replay.status.success());
    assert_eq!(fs::read_to_string(&authorized_keys).unwrap(), after_first);
    assert!(!after_first.contains("attacker"));
    fs::remove_dir_all(root).unwrap();
}
