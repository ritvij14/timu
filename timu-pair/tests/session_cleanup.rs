use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use timu_pair::{CleanupGuard, System, wait_for_completion};

static TEST_ID: AtomicU64 = AtomicU64::new(0);

fn isolated_root() -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
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

struct FakeSystem {
    now: AtomicU64,
    increment: u64,
}

impl FakeSystem {
    fn starting_at(start: u64, increment: u64) -> Self {
        Self {
            now: AtomicU64::new(start),
            increment,
        }
    }
}

impl System for FakeSystem {
    fn family(&self) -> &'static str {
        "linux"
    }

    fn command(&self, program: &str, _args: &[&str]) -> Result<timu_pair::CommandOutput, String> {
        Err(format!("unexpected command: {program}"))
    }

    fn prompt(&self, _question: &str) -> Result<String, String> {
        Err("no prompts in session cleanup tests".to_string())
    }

    fn now(&self) -> u64 {
        let current = self.now.load(Ordering::SeqCst);
        self.now.fetch_add(self.increment, Ordering::SeqCst);
        current
    }

    fn tcp_reachable(&self, _host: &str, _port: u16, _timeout: Duration) -> bool {
        false
    }

    fn route_address(&self) -> Option<String> {
        None
    }

    fn sleep(&self, _duration: Duration) {}
}

#[test]
fn completion_marker_ends_wait_before_expiry() {
    let root = isolated_root();
    let done = root.join("done");
    let system = FakeSystem::starting_at(0, 1);
    let cancelled = AtomicBool::new(false);

    fs::write(&done, b"paired\n").expect("completion marker");
    let result = wait_for_completion(&system, &done, 10, &cancelled);

    assert!(
        result.is_ok(),
        "done marker should end wait successfully: {result:?}"
    );
    fs::remove_dir_all(root).expect("remove isolated root");
}

#[test]
fn timeout_cleanup_removes_only_tagged_temporary_authorization() {
    let parent = isolated_root();
    let root = parent.join("ceremony");
    fs::create_dir(&root).expect("ceremony root");
    let authorized_keys = parent.join("authorized_keys");
    fs::write(
        &authorized_keys,
        concat!(
            "ssh-ed25519 AAAAUNRELATED laptop\n",
            "restrict ssh-ed25519 AAAATEMP timu-pair:pair-timeout\n",
            "ssh-ed25519 AAAAPERMANENT timu-device:iphone\n",
        ),
    )
    .expect("authorization fixture");

    let system = FakeSystem::starting_at(0, 5);
    let done = root.join("done");
    let cancelled = AtomicBool::new(false);

    let mut cleanup = CleanupGuard::new(root.clone(), "pair-timeout".into());
    cleanup.register_authorization(authorized_keys.clone());

    let error = wait_for_completion(&system, &done, 10, &cancelled)
        .expect_err("fake time should reach expiry");

    assert!(
        error.to_lowercase().contains("expired"),
        "error should report expiry: {error}"
    );
    drop(cleanup);

    let contents = fs::read_to_string(&authorized_keys).expect("authorization after timeout");
    assert!(contents.contains("AAAAUNRELATED"));
    assert!(contents.contains("AAAAPERMANENT"));
    assert!(!contents.contains("timu-pair:pair-timeout"));
    assert!(!root.exists());
    fs::remove_dir_all(parent).expect("remove isolated parent");
}

#[test]
fn injectable_cancellation_boundary_removes_only_tagged_temporary_authorization() {
    let parent = isolated_root();
    let root = parent.join("ceremony");
    fs::create_dir(&root).expect("ceremony root");
    let authorized_keys = parent.join("authorized_keys");
    fs::write(
        &authorized_keys,
        concat!(
            "ssh-ed25519 AAAAUNRELATED laptop\n",
            "restrict ssh-ed25519 AAAATEMP timu-pair:pair-cancel\n",
        ),
    )
    .expect("authorization fixture");

    let system = FakeSystem::starting_at(0, 1);
    let done = root.join("done");
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = Arc::clone(&cancelled);

    std::thread::spawn(move || {
        worker_cancelled.store(true, Ordering::SeqCst);
    });

    let mut cleanup = CleanupGuard::new(root.clone(), "pair-cancel".into());
    cleanup.register_authorization(authorized_keys.clone());

    let error = wait_for_completion(&system, &done, 100, &cancelled)
        .expect_err("injected cancellation should end wait");

    assert!(
        error.to_lowercase().contains("cancelled"),
        "error should report cancellation: {error}"
    );
    drop(cleanup);

    let contents = fs::read_to_string(&authorized_keys).expect("authorization after cancellation");
    assert!(contents.contains("AAAAUNRELATED"));
    assert!(!contents.contains("timu-pair:pair-cancel"));
    assert!(!root.exists());
    fs::remove_dir_all(parent).expect("remove isolated parent");
}
