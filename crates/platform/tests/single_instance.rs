//! Single-instance guard tests (Windows named mutex).

#![cfg(windows)]

use platform::{PlatformError, SingleInstanceGuard};

fn unique(tag: &str) -> String {
    format!("Global\\v2rayN-R-t13-{tag}-{}", std::process::id())
}

#[test]
fn second_acquire_is_rejected_then_allowed_after_drop() {
    let name = unique("dup");
    let first = SingleInstanceGuard::acquire(&name).expect("first acquire");
    assert_eq!(first.name(), name);

    let err = SingleInstanceGuard::acquire(&name).expect_err("second must fail");
    assert!(matches!(err, PlatformError::AlreadyRunning(_)));

    drop(first);
    let third = SingleInstanceGuard::acquire(&name).expect("after drop");
    assert_eq!(third.name(), name);
}

#[test]
fn distinct_names_coexist() {
    let a = SingleInstanceGuard::acquire(&unique("a")).expect("a");
    let b = SingleInstanceGuard::acquire(&unique("b")).expect("b");
    assert_ne!(a.name(), b.name());
}

#[test]
fn empty_name_is_invalid() {
    assert!(matches!(
        SingleInstanceGuard::acquire(""),
        Err(PlatformError::Invalid(_))
    ));
}
