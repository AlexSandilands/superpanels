# First commits playbook

A suggested sequence for the first day's work, optimised for getting a green CI as early as possible.

1. **Commit 1 — workspace scaffold.** `Cargo.toml` workspace + `crates/superpanels-core` lib + `crates/superpanels-cli` bin + a `hello` integration test. CI green.
2. **Commit 2 — `Monitor` & `Rotation` types.** With serde derives and a round-trip JSON test. Sets the data-model foundation everything else hangs from.
3. **Commit 3 — `BezelConfig`, `CropSpec`, `Rect`, `FitMode`.** Pure data types, no logic.
4. **Commit 4 — bezel math: trivial cases.** Single monitor + two identical monitors with zero gap. Tests pin the obvious behaviour first.
5. **Commit 5 — bezel math: uniform gap, mixed PPI.** The interesting cases. Property tests added.
6. **Commit 6 — bezel math: rotation + 2×2 grid.** Closes the matrix.
7. **Commit 7 — KSscreen-doctor parser.** Tested against captured fixtures only; no live system.
8. **Commit 8 — manual override parser.** `--monitors` syntax tested with multiple shapes.
9. **Commit 9 — `superpanels detect` CLI.** First end-to-end CLI command. Ship-able.
10. **Commit 10 — `image.rs` load/scale/crop/save_temp.** Pure-Rust pipeline, integration test against a real test image.

By commit 10, you have a passing CI, a working `detect` command, a tested bezel pipeline, and a tested image pipeline. Phase 1.5 (the KDE backend) is the next 2–3 commits and the first one whose tests can't run on CI without a live KDE session — handle it via the `MockBackend`-first approach so the trait is well-defined before the real backend is written.

## Where to start, in one line

`crates/superpanels-core/src/display/kscreen.rs` against a captured fixture, with the test written first. Everything else hangs off knowing what monitors exist; everything purely arithmetic (`layout.rs`) can develop in parallel from the same `Monitor` type.
