//! Integration-level smoke test: the public library surface compiles and is
//! reachable from outside the crate. Real behavior tests live alongside their
//! modules (`src/<module>.rs`) and as integration tests in this directory.

use timu_core::TimuCore;

#[test]
fn core_can_be_constructed_from_outside_the_crate() {
    let _core = TimuCore::new();
}