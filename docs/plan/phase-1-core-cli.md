# Phase 1 — Core CLI MVP

**Goal.** `superpanels set panorama.jpg` works on KDE Wayland and produces a correctly bezel-spliced wallpaper.

**Definition of done.**
- [ ] On a fresh CachyOS KDE install with `cargo install superpanels`, a panorama spans three monitors with bezel correction on first try. *(Bezel-correct spanning verified locally on Plasma 6.6.4 / Wayland; the `cargo install` half is gated on the crates.io publish in Phase 5.2.)*
- [x] `superpanels detect` prints the monitor table and exits zero.
- [x] All bezel-math edge cases listed in SPEC §4.5 have passing tests.
- [x] `cargo clippy --all-features -- -D warnings` is clean.
- [x] `cargo test` runs in < 30 s.

## 1.1 Workspace scaffold
- [x] `cargo new --lib superpanels-core` inside a workspace `Cargo.toml`.
- [x] Add `superpanels-cli` crate as a separate binary; `superpanels-cli` depends on `superpanels-core`.
- [x] Workspace deps: `image`, `anyhow`, `thiserror`, `serde`, `serde_json`, `toml`, `toml_edit` (round-trip-with-comments writes), `clap` (with `derive`), `tracing`, `tracing-subscriber`.
- [x] `rust-toolchain.toml` pinning to a stable Rust version (recent stable, no nightly).
- [x] `rustfmt.toml`, `clippy.toml` (lints config), `.editorconfig`.
- [x] `#![forbid(unsafe_code)]` in every crate.
- [x] `.gitignore` (target/, *.bak, dist/, ui/node_modules/, ui/dist/).
- [x] `LICENSE` (MIT or Apache-2.0/MIT dual; pick early).
- [x] Minimal `README.md` with one-line description + "still in development" badge.
- [x] CI skeleton (`.github/workflows/ci.yml`) running `cargo test`, `clippy`, `fmt --check` on push.

## 1.2 `display/` — KDE detection
- [x] `Monitor` struct per SPEC §3.1, `serde::Serialize`. Note: `physical_size_mm: Option<(u32, u32)>` and `ppi: Option<f64>` — both `None` until merged with config (§1.6).
- [x] `Rotation` enum, `serde`-friendly. Includes the KDE numeric mapping (1=None, 2=Right, 4=Inverted, 8=Left) — verify via spike fixture.
- [x] `MonitorRef { stable_id, name }` per SPEC §6.4.
- [x] `DisplayDetector` trait + `Availability` + `DetectError` per SPEC §6.1.
- [x] `KscreenDoctorDetector` — spawns subprocess with `NO_COLOR=1` env (fixture `tests/fixtures/display/kscreen-3-monitors.txt` has ANSI escapes if not stripped; setting the env avoids the regex), timeout 5 s, parses output. Extracts per-output UUID as `stable_id`. **Does not extract physical mm** — kscreen-doctor doesn't expose it.
  - Fixture already captured for the 3×27" 2560×1440 case. Add a single-monitor fixture from any other system before merging.
  - Tests run against fixtures, *not* a live system.
- [x] Manual override parser for `--monitors` per SPEC §6.2 (layout-only and full-with-mm forms both supported).
- [x] `detect()` orchestrator: try KDE detector, then manual override, then bail with the friendly error from SPEC §6.
- [x] **Config merge step:** after detection, walk `[[monitor]]` blocks from §14.1 config and populate `physical_size_mm` (matching by `stable_id` first, then `name`). Compute `ppi` for monitors that got a size.
- [x] `superpanels detect` and `superpanels detect --json` CLI wired up. Output indicates which monitors are missing physical_mm.
- [x] `--debug` flag prints attempted detectors + their stderr + which `Availability` variant each returned.

## 1.3 `layout.rs` — bezel math
- [x] `BezelConfig`, `CropSpec`, `Rect`, `FitMode`, `LayoutError` types per SPEC §3.
- [x] `compute_crop_specs(monitors, bezels, image_size, fit) -> Result<Vec<CropSpec>, LayoutError>`.
- [x] **Pre-flight check:** any monitor with `physical_size_mm: None` triggers `LayoutError::PhysicalSizeMissing { monitors: Vec<MonitorRef> }` so callers can prompt the user. Test this explicitly.
- [x] Single row, identical monitors, zero bezel — trivial test.
- [x] Single row, identical monitors, uniform bezel — the canonical test.
- [x] Single row, mixed PPI — verify reference-PPI normalisation.
- [x] Mixed orientation (one portrait monitor) — rotation handling.
- [x] 2×2 grid — vertical bezel logic.
- [x] Single-monitor degenerate case.
- [x] Property tests with `proptest`:
  - Sum of crop widths in pixel space equals image width × scaling, modulo gap.
  - No two crops overlap.
  - Every monitor receives exactly one crop.

## 1.4 `image.rs` — processing
- [x] `load(path) -> Result<DynamicImage>` with friendly error on unsupported format / bad file.
- [x] `scale_to_fit(img, target, FitMode) -> DynamicImage`. Lanczos3 default.
- [x] `crop(img, rect) -> DynamicImage` with bounds-check error.
- [x] `rotate(img, Rotation) -> DynamicImage` (uses `image::imageops::rotate90` etc.).
- [x] `save_temp(img, name) -> Result<PathBuf>` writing to `$XDG_CACHE_HOME/superpanels/temp/`.
- [x] Clear temp dir at the start of each run (atomic: remove + recreate).
- [x] Memory cap: refuse to decode if estimated decoded size > configured limit (default 512 MB).
- [x] Integration test: load a real test image, run a full `compute_crop_specs → crop → save_temp` pipeline, verify output dimensions and existence.

## 1.5 `backends/` — KDE backend
- [x] `WallpaperBackend` trait per SPEC §10.1.
- [x] `KdeBackend::availability()` checks `$KDE_FULL_SESSION` / `$XDG_CURRENT_DESKTOP` and returns `Availability::Available` or a specific `WrongEnvironment { reason }`.
- [x] `KdeBackend::apply()` uses `zbus` to call `org.kde.PlasmaShell.evaluateScript` setting per-monitor `Image` plugin source.
- [x] JS payload generated from a versioned template; image paths injected as JSON-quoted literals (never string-concatenated).
- [x] Subprocess/D-Bus rules per SPEC §10.3 (timeout, error capture).
- [x] Manual smoke test on KDE: does the wallpaper actually change? Does it survive a logout/login? Is it bezel-correct?
- [x] Test against a `MockBackend` is written *first*, then `KdeBackend` is implemented to satisfy the same trait — keeps the trait honest.

## 1.6 `config.rs` — config & profiles
- [x] `Config`, `Profile`, `BackendKind`, `MonitorConfig` types with `serde` derives.
- [x] `[[monitor]]` block per SPEC §14.1: `stable_id?`, `name?`, `physical_mm: [u32; 2]`. At least one of `stable_id`/`name` required. Round-trip stable.
- [x] `superpanels monitor configure <NAME-OR-ID> --diagonal 27in --aspect 16:9` and `--mm 597x336` CLI subcommands so users can write `[[monitor]]` blocks without hand-editing TOML.
- [x] Load from `$XDG_CONFIG_HOME/superpanels/config.toml`, write a default if missing (with comments via `toml_edit`).
- [x] Validation pass: returns `ConfigError { path, field, message }`.
- [x] Round-trip test: load → modify → save → reload → identical.
- [x] No I/O in tests; use `tempfile`.

## 1.7 CLI wiring (`superpanels-cli/src/main.rs`)
- [x] `clap::Parser`-derived command tree.
- [x] `superpanels set <IMAGE> [...flags]` per SPEC §11.1, except `--save-as` (Phase 2).
- [x] `superpanels detect`.
- [x] `superpanels config` — print resolved config (debug aid).
- [x] `--dry-run`: print computed crop specs as JSON, skip file write and backend call.
- [x] `--verbose` / `-v` increases tracing level; `--quiet` suppresses non-error output.
- [x] Exit codes per SPEC §11.6.
- [x] `--no-daemon` flag accepted (no-op in Phase 1; daemon arrives in Phase 2).

## 1.8 Documentation (Phase 1 slice)
- [x] README: install (cargo), one-line example, link to SPEC.
- [x] `docs/architecture.md`: a one-page summary of the layout from SPEC §5 with current crate graph.

**Risks for this phase.**
- `kscreen-doctor` output format may be looser than expected (locale dependence; LC_ALL=C needed). Mitigated by capturing fixtures from real systems early.
- `zbus` API churn between minor versions. Pin to a specific minor.
- Image memory bombs. Cap enforced before decode.
