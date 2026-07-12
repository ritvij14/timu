use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use timu_pair::replace_temporary_authorized_key_in_file;

const VALID_ED25519_PUBLIC_KEY: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4f";

static ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn isolated_root() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let sequence = ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "timu-pair-concurrent-{unique}-{sequence}-{}",
        std::process::id()
    ));
    fs::create_dir(&root).expect("isolated root");
    root
}

#[test]
fn concurrent_handoffs_preserve_unrelated_and_each_other_authorizations() {
    for _ in 0..30 {
        let root = isolated_root();
        let authorized_keys = root.join("authorized_keys");
        fs::write(
            &authorized_keys,
            concat!(
                "ssh-ed25519 AAAAUNRELATED laptop\n",
                "restrict ssh-ed25519 AAAATEMP_A timu-pair:pair-a\n",
                "restrict ssh-ed25519 AAAATEMP_B timu-pair:pair-b\n",
            ),
        )
        .expect("authorization fixture");
        let barrier = Arc::new(Barrier::new(3));
        let first_path = authorized_keys.clone();
        let first_barrier = Arc::clone(&barrier);
        let first = thread::spawn(move || {
            first_barrier.wait();
            replace_temporary_authorized_key_in_file(
                &first_path,
                "pair-a",
                &format!("{VALID_ED25519_PUBLIC_KEY} timu-device:a"),
            )
        });
        let second_path = authorized_keys.clone();
        let second_barrier = Arc::clone(&barrier);
        let second = thread::spawn(move || {
            second_barrier.wait();
            replace_temporary_authorized_key_in_file(
                &second_path,
                "pair-b",
                &format!("{VALID_ED25519_PUBLIC_KEY} timu-device:b"),
            )
        });
        barrier.wait();
        first.join().expect("first thread").expect("first handoff");
        second
            .join()
            .expect("second thread")
            .expect("second handoff");

        let contents = fs::read_to_string(&authorized_keys).expect("authorization after handoffs");
        assert!(contents.contains("AAAAUNRELATED"));
        assert!(contents.contains("timu-device:a"));
        assert!(contents.contains("timu-device:b"));
        assert!(!contents.contains("timu-pair:pair-a"));
        assert!(!contents.contains("timu-pair:pair-b"));
        fs::remove_dir_all(root).expect("remove isolated root");
    }
}

#[test]
fn concurrent_temporary_authorizations_preserve_each_ceremony() {
    let root = isolated_root();
    let authorized_keys = root.join("authorized_keys");
    fs::write(&authorized_keys, "ssh-ed25519 AAAAUNRELATED laptop\n")
        .expect("authorization fixture");
    let barrier = Arc::new(Barrier::new(3));
    let first_path = authorized_keys.clone();
    let first_barrier = Arc::clone(&barrier);
    let first = thread::spawn(move || {
        first_barrier.wait();
        timu_pair::append_authorized_key_line(
            &first_path,
            "restrict ssh-ed25519 AAAATEMP_A timu-pair:pair-a",
        )
    });
    let second_path = authorized_keys.clone();
    let second_barrier = Arc::clone(&barrier);
    let second = thread::spawn(move || {
        second_barrier.wait();
        timu_pair::append_authorized_key_line(
            &second_path,
            "restrict ssh-ed25519 AAAATEMP_B timu-pair:pair-b",
        )
    });
    barrier.wait();
    first.join().expect("first thread").expect("first append");
    second
        .join()
        .expect("second thread")
        .expect("second append");

    let contents = fs::read_to_string(&authorized_keys).expect("authorization after appends");
    assert!(contents.contains("AAAAUNRELATED"));
    assert!(contents.contains("AAAATEMP_A"));
    assert!(contents.contains("AAAATEMP_B"));
    fs::remove_dir_all(root).expect("remove isolated root");
}

#[test]
fn duplicate_live_pairing_id_is_rejected_without_appending_another_authorization() {
    let root = isolated_root();
    let authorized_keys = root.join("authorized_keys");
    fs::write(
        &authorized_keys,
        concat!(
            "ssh-ed25519 AAAAUNRELATED laptop\n",
            "restrict ssh-ed25519 AAAATEMP_A timu-pair:pair-a\n",
        ),
    )
    .expect("authorization fixture");

    let error = timu_pair::append_authorized_key_line(
        &authorized_keys,
        "restrict ssh-ed25519 AAAATEMP_B timu-pair:pair-a",
    )
    .expect_err("duplicate live pairing ID should fail");

    assert_eq!(error.to_string(), "invalid pairing payload");
    let contents = fs::read_to_string(&authorized_keys).expect("authorization after duplicate");
    assert!(contents.contains("AAAAUNRELATED"));
    assert_eq!(contents.matches("timu-pair:pair-a").count(), 1);
    assert!(!contents.contains("AAAATEMP_B"));
    fs::remove_dir_all(root).expect("remove isolated root");
}
