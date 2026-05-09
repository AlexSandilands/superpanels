# Phase 6 — Stabilisation toward 1.0

**Goal.** Schema-frozen, perf-budget-enforced, accessibility-audited, ready for a 1.0 declaration.

**Definition of done.**
- [ ] No breaking config changes since v0.7; migration code present for older configs.
- [ ] Performance budgets from SPEC §19 enforced by CI benchmarks (regression > 10% fails the build).
- [ ] Localisation pipeline operational; English catalogue complete.
- [ ] Accessibility audit complete (Orca + axe-core for the web view); known issues triaged.
- [ ] One external review (a Linux-power-user friend) used the app for a week and reported no blocking bugs.

## 6.1 Schema freeze
- [ ] Audit `Config`, `Profile`, `state.json`, `library.db` schemas; make any pending changes.
- [ ] Tag the schema as `v1`. Future changes require migrations, not breakage.

## 6.2 Performance enforcement
- [ ] `criterion` benches for: bezel math (10k random layouts), library scan (5k images), single-image apply (4K, 8K), `read_dimensions`, `library_thumbnail` @ 4K source, `library_list` @ 5k entries, `temp::save_temp_in` (fast-PNG path). Establishes tracked baselines for SPEC §19's 5 ms/call canvas budget and 200 ms/4K thumbnail target.
- [ ] `cargo bench` runs nightly in CI; results compared to `main`; > 10% regression fails.

## 6.3 Localisation
- [ ] All UI strings in `ui/locales/en.ftl`.
- [ ] All CLI human-readable messages in `crates/superpanels-cli/locales/en.ftl`.
- [ ] `t!("…")` macro everywhere; no string literals in UI files.
- [ ] CONTRIBUTING.md has a "Adding a translation" section.

## 6.4 Documentation site
- [ ] `mdbook` site under `docs/` deployed to GitHub Pages on push to `main`.
- [ ] Tutorials: "Spanning a panorama", "Per-monitor profiles", "Slideshow + schedule", "Custom backend".
- [ ] Architecture deep-dive (the bezel math walkthrough) — turn SPEC §4 into prose with diagrams.

## 6.5 1.0 release
- [ ] Tag `v1.0.0`.
- [ ] Blog post / project announcement.
- [ ] Submit to `r/unixporn`, `r/archlinux`, HN, with the showcase screenshot.
