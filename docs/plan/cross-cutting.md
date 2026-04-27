# Cross-cutting concerns

These run through every phase and don't belong to any single one.

## Testing
- Unit tests in every module; coverage tracked but not gated on a threshold.
- Snapshot tests for parsers via `insta`.
- Property tests for layout via `proptest`.
- Integration tests using `MockBackend`.
- Golden-image tests for the image pipeline (post-Phase 1).
- Manual smoke checklist (`docs/release-checklist.md`) updated each phase, run before each release.

## CI / quality
- `cargo test --workspace --all-features` on every PR.
- `cargo clippy --all-features -- -D warnings`.
- `cargo fmt --check`.
- `cargo audit` weekly.
- `cargo deny check` for licence compliance.
- `cargo machete` to catch unused dependencies.
- `npm audit` weekly on the UI.

## Logging
- `tracing` from day one. Every subprocess call, every IPC request, every config load logs at `info`/`debug`.
- Structured fields, never `format!` into the message.

## Documentation hygiene
- `cargo doc` builds clean (no missing docs warnings on public items in `superpanels-core`).
- Top-level types in the public API have a one-paragraph rustdoc with an example.
- SPEC and PLAN are kept current; "spec drift" is a PR-blocking review comment.

## Security review at each phase exit
- New subprocess spawns: re-confirm the rules in SPEC §10.3.
- New file paths read/written: re-confirm scope.
- New IPC commands: re-confirm input validation.

## Performance baselines
- Phase 1: bench `compute_crop_specs` for 1, 3, 6, 9 monitors. Capture baseline.
- Phase 2: bench library scan for 100, 1k, 10k images. Capture baseline.
- Phase 4: bench canvas redraw frame time. Capture baseline.
