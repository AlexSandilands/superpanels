//! Integration smoke test: the core crate's `VERSION` constant is reachable
//! from a binary-side caller. Proves the workspace's `cli → core` dep edge.

#[test]
fn core_version_is_reachable_from_cli_crate() {
    assert!(!superpanels_core::VERSION.is_empty());
}
