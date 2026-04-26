//! Integration smoke test: the core crate's `VERSION` constant is reachable
//! and non-empty from a binary-side caller.

#[test]
fn core_version_constant_is_non_empty() {
    assert!(!superpanels_core::VERSION.is_empty());
}
