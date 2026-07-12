use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use timu_pair::CleanupGuard;

static TEST_ID: AtomicU64 = AtomicU64::new(0);

fn isolated_root() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let sequence = TEST_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "timu-pair-cleanup-{unique}-{sequence}-{}",
        std::process::id()
    ));
    fs::create_dir(&root).expect("isolated ceremony root");
    root
}

#[test]
fn startup_failure_before_qr_removes_acquired_ceremony_artifacts() {
    let root = isolated_root();
    fs::write(root.join("ephemeral"), "private material").expect("private key artifact");
    fs::write(root.join("ephemeral.pub"), "public material").expect("public key artifact");

    let cleanup = CleanupGuard::new(root.clone(), "pair-123".into());
    drop(cleanup);

    assert!(!root.exists());
}

#[test]
fn cancellation_cleanup_removes_only_its_temporary_authorization() {
    let parent = isolated_root();
    let root = parent.join("ceremony");
    fs::create_dir(&root).expect("ceremony root");
    let authorized_keys = parent.join("authorized_keys");
    fs::write(
        &authorized_keys,
        concat!(
            "ssh-ed25519 AAAAUNRELATED laptop\n",
            "restrict ssh-ed25519 AAAATEMP timu-pair:pair-123\n",
            "ssh-ed25519 AAAAPERMANENT timu-device:iphone\n",
            "restrict ssh-ed25519 AAAAOTHER timu-pair:pair-other\n",
        ),
    )
    .expect("authorization fixture");

    let mut cleanup = CleanupGuard::new(root.clone(), "pair-123".into());
    cleanup.register_authorization(authorized_keys.clone());
    drop(cleanup);

    let contents = fs::read_to_string(&authorized_keys).expect("authorization after cleanup");
    assert!(contents.contains("AAAAUNRELATED"));
    assert!(contents.contains("AAAAPERMANENT"));
    assert!(contents.contains("timu-pair:pair-other"));
    assert!(!contents.contains("timu-pair:pair-123"));
    assert!(!root.join("ephemeral").exists());
    fs::remove_dir_all(parent).expect("remove isolated parent");
}
