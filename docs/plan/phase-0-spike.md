# Phase 0 — Spike: derisk the hard bits (1–2 days)

**Goal.** Validate the two hardest assumptions in the spec before committing to architecture.

**Deliverable.** A *throwaway* standalone Rust binary (its own scratch directory outside this repo, or a `rust-script` single-file program — the workspace doesn't exist yet) that:
1. Reads `kscreen-doctor -o` and parses it into `Vec<Monitor>` including physical sizes in mm.
2. Computes `Vec<CropSpec>` for an arbitrary input image and the detected layout.
3. Prints the result in JSON.

This validates: the parser is feasible; physical-mm data is actually present; the bezel math gives sensible numbers on a real layout. **No GUI, no D-Bus, no apply.**

> Note: `examples/spike.rs` would require an existing Cargo workspace. We don't have one yet — Phase 1.1 creates it. Keep the spike entirely separate so no spike code can drift into Phase 1.

**Definition of done.**
- [ ] kscreen-doctor parser handles a real 3-monitor KDE layout from the dev machine.
- [ ] Bezel math run by hand (calculator) for that layout matches the program's output.
- [ ] The spike code is *deleted* (or moved to `examples/`) before Phase 1 starts. Do not let spike code become production code.

**Outputs that survive into Phase 1.**
- The captured `kscreen-doctor` output sample → moves to `tests/fixtures/display/` for the real parser.
- The list of monitor-layout edge cases the spike surfaced → opens a GitHub issue list.
