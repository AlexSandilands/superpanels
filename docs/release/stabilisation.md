# Stabilisation toward 1.0

The list of things that have to be true before Superpanels claims a 1.0. Updated 2026-05 to reflect what's already in the codebase vs. what's outstanding.

**Definition of done for 1.0.**

- [ ] No breaking config changes since 0.1; migration code present for older configs.
- [ ] Performance budgets enforced by CI benchmarks (regression > 10 % fails the build).
- [ ] Localisation pipeline operational; English catalogue complete.
- [ ] Accessibility audit complete (Orca + axe-core); known issues triaged.
- [ ] One external review (a Linux-power-user friend) used the app for a week and reported no blocking bugs.

## Schema freeze

- [ ] Audit `Config`, `Profile`, `resume-state.json`, `library.db` schemas; make any pending changes.
- [ ] Tag the schemas as `v1`. Future changes require migrations, not breakage.

Current state: `library.db` carries `PRAGMA user_version` and runs migrations on startup. `resume-state.json` has no version field (additive fields use serde defaults). `config.toml` has no explicit version field. Add one before freeze so we can detect downgrades.

## Performance enforcement

`criterion` benches live under `crates/superpanels-core/benches/` and currently cover:

- `layout.rs` — layout math (10k random layouts).
- `image.rs` — single-image apply (4K, 8K), `read_dimensions`, `library_thumbnail` @ 4K, `library_list` @ 5k entries, `temp::save_temp_in` (fast-PNG path).
- `library.rs` — folder scan (5k images), index load.
- `gnome.rs` — composite path.

Still to do:

- [ ] `cargo bench` runs nightly in CI; results compared to `main`; > 10 % regression fails the build.
- [ ] Tracked baseline for the 5 ms/call canvas budget and 200 ms/4K thumbnail target.

## Localisation

- [ ] All UI strings in `ui/locales/en.ftl`.
- [ ] All CLI human-readable messages in `crates/superpanels-cli/locales/en.ftl`.
- [ ] `t!("…")` macro everywhere; no string literals in UI files.
- [ ] CONTRIBUTING has a "Adding a translation" section.

Current state: no localisation. All strings are inline.

## Documentation site

- [ ] `mdbook` site under `docs/` deployed to GitHub Pages on push to `main`.
- [ ] Tutorials: "Spanning a panorama", "Per-monitor profiles", "Slideshow + schedule", "Custom backend".
- [ ] Architecture deep-dive — turn `layout-math.md` into a prose explainer with diagrams.

## 1.0 release

- [ ] Tag `v1.0.0`.
- [ ] Blog post / project announcement.
- [ ] Submit to `r/unixporn`, `r/archlinux`, HN, with the showcase screenshot.

## Outstanding UX work (not blockers, but ship-quality items)

These should be cleared by 1.0:

- Lag when opening the profile switcher.
- Monitor-gap not loaded on app start for the active profile.
- Topology-repair flow for profiles authored against a different connected set.
- Taskbar icon on KDE Plasma.

Track these as [GitHub issues](https://github.com/AlexSandilands/superpanels/issues); close them as they ship.
