use std::fs;
use std::io::Write;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

const VALID_ED25519_PUBLIC_KEY: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4f";
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_ID: AtomicU64 = AtomicU64::new(0);

fn isolated_root() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let sequence = TEST_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "timu-pair-filesystem-security-{}-{unique}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("isolated root");
    root
}

fn complete(authorized_keys: &Path, done: &Path) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_timu-pair"))
        .args([
            "--complete",
            "pair-123",
            authorized_keys
                .to_str()
                .expect("UTF-8 authorized_keys path"),
            done.to_str().expect("UTF-8 completion path"),
            "4102444800",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("completion subprocess");
    child
        .stdin
        .take()
        .expect("completion stdin")
        .write_all(format!("{VALID_ED25519_PUBLIC_KEY} timu-device:iphone\n").as_bytes())
        .expect("device key input");
    child.wait_with_output().expect("completion result")
}

fn temporary_authorization() -> &'static str {
    "ssh-ed25519 AAAAOLD laptop\nrestrict ssh-ed25519 AAAATEMP timu-pair:pair-123\n"
}

#[test]
fn pairing_rejects_a_symlinked_authorized_keys_file_without_modifying_its_target() {
    let root = isolated_root();
    let target = root.join("linked-authorized-keys");
    let authorized_keys = root.join("authorized_keys");
    let done = root.join("done");
    fs::write(&target, temporary_authorization()).expect("linked target contents");
    symlink(&target, &authorized_keys).expect("authorized_keys symlink");
    let before = fs::read_to_string(&target).expect("target before");

    let output = complete(&authorized_keys, &done);

    assert!(!output.status.success());
    assert_eq!(fs::read_to_string(&target).expect("target after"), before);
    assert!(!done.exists());
    fs::remove_dir_all(root).expect("remove isolated root");
}

#[test]
fn pairing_rejects_a_symlinked_ssh_directory_without_modifying_its_target() {
    let root = isolated_root();
    let linked_ssh_target = root.join("linked-ssh");
    let home = root.join("home");
    let ssh_directory = home.join(".ssh");
    let authorized_keys = ssh_directory.join("authorized_keys");
    let done = root.join("done");
    fs::create_dir_all(&linked_ssh_target).expect("linked ssh target");
    let target = linked_ssh_target.join("authorized_keys");
    fs::write(&target, temporary_authorization()).expect("target contents");
    fs::create_dir_all(&home).expect("home");
    symlink(&linked_ssh_target, &ssh_directory).expect("ssh directory symlink");
    let before = fs::read_to_string(&target).expect("target before");

    let output = complete(&authorized_keys, &done);

    assert!(!output.status.success());
    assert_eq!(fs::read_to_string(&target).expect("target after"), before);
    assert!(!done.exists());
    fs::remove_dir_all(root).expect("remove isolated root");
}

#[test]
fn pairing_handoff_preserves_authorized_keys_permissions() {
    let root = isolated_root();
    let authorized_keys = root.join("authorized_keys");
    let done = root.join("done");
    fs::write(&authorized_keys, temporary_authorization()).expect("authorization fixture");
    fs::set_permissions(&authorized_keys, fs::Permissions::from_mode(0o640))
        .expect("fixture permissions");

    let output = complete(&authorized_keys, &done);

    assert!(output.status.success());
    let mode = fs::metadata(&authorized_keys)
        .expect("authorized_keys metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o640);
    fs::remove_dir_all(root).expect("remove isolated root");
}

#[test]
fn pairing_rejects_group_or_world_writable_authorized_keys() {
    let root = isolated_root();
    let authorized_keys = root.join("authorized_keys");
    let done = root.join("done");
    fs::write(&authorized_keys, temporary_authorization()).expect("authorization fixture");
    fs::set_permissions(&authorized_keys, fs::Permissions::from_mode(0o666))
        .expect("unsafe fixture permissions");
    let before = fs::read_to_string(&authorized_keys).expect("authorized_keys before");

    let output = complete(&authorized_keys, &done);

    assert!(!output.status.success());
    assert_eq!(
        fs::read_to_string(&authorized_keys).expect("authorized_keys after"),
        before
    );
    assert!(!done.exists());
    fs::remove_dir_all(root).expect("remove isolated root");
}
