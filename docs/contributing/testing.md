# Testing strategy

> What to test, where to put the tests, and what tools we use.

This doc is the practitioner's reference — what to write and where to put it.

---

## Table of contents

- [The tiers](#the-tiers)
- [Unit tests](#unit-tests)
- [Integration tests](#integration-tests)
- [Snapshot tests](#snapshot-tests-insta)
- [Property tests](#property-tests-proptest)
- [Doc tests](#doc-tests)
- [Frontend tests](#frontend-tests)
- [What we deliberately do NOT auto-test](#what-we-deliberately-do-not-auto-test)
- [Running tests locally](#running-tests-locally)
- [Naming and organising tests](#naming-and-organising-tests)
- [Writing a good test](#writing-a-good-test)

---

## The tiers

| Tier | Tool | When to use it |
|---|---|---|
| Unit | `#[test]` | Isolated logic — layout math, parsers, struct serde. Most tests live here. |
| Integration | `tests/*.rs` | Cross-module flows — e.g. "full `set` pipeline against a mock backend". |
| Snapshot | `insta` | Parser output, CLI human-readable output, JSON shapes. |
| Property | `proptest` | Algorithms with invariants — layout math. |
| Doc | `///` examples | Public API examples that must compile + run. |
| Bench | `criterion` | Hot paths — layout math, image processing, library scan. |
| Frontend unit | `vitest` | TS utility functions, store logic. |
| Frontend visual | (manual) | Canvas rendering — no auto-test. |

---

## Unit tests

The default. Live in the same file as the code they test, in a `#[cfg(test)] mod tests { ... }` block at the bottom.

```rust
// crates/superpanels-core/src/layout.rs

pub fn compute_crop_specs(/* ... */) -> Result<Vec<CropSpec>, LayoutError> {
    // ...
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schedule::monitor_key;

    fn monitor(id: u32, name: &str, w_px: u32, h_px: u32, w_mm: u32, h_mm: u32) -> Monitor {
        // helper for terse test setup
    }

    fn place(x_mm: f32, y_mm: f32) -> MonitorPlacement {
        MonitorPlacement { x_mm, y_mm }
    }

    #[test]
    fn single_monitor_image_covering_returns_full_image() {
        let monitors = vec![monitor(0, "DP-1", 1920, 1080, 480, 270)];
        let mut placements = HashMap::new();
        placements.insert(monitor_key(&monitors[0]), place(0.0, 0.0));
        let rect = ImageRectMm { x_mm: 0.0, y_mm: 0.0, w_mm: 480.0, h_mm: 270.0 };
        let crops = compute_crop_specs(&monitors, &placements, (1920, 1080), rect).unwrap();

        assert_eq!(crops.len(), 1);
        assert_eq!(crops[0].src_rect.w, 1920);
    }

    #[test]
    fn two_monitors_with_gap_skip_gap_in_source() {
        // ...
    }

    #[test]
    fn empty_monitor_list_returns_error() {
        let placements = HashMap::new();
        let rect = ImageRectMm { x_mm: 0.0, y_mm: 0.0, w_mm: 100.0, h_mm: 100.0 };
        let result = compute_crop_specs(&[], &placements, (1920, 1080), rect);
        assert!(matches!(result, Err(LayoutError::EmptyMonitorList)));
    }
}
```

### Conventions

- Test name describes the scenario AND the expected outcome:
  - ✅ `single_monitor_no_bezel_returns_full_image`
  - ❌ `test_compute_crop_specs_1`
- Helper functions for setup — `monitor()`, `place(0.0, 0.0)` — keep tests readable.
- One concept per test. If you find yourself writing "Test 1, Test 2, Test 3" assertions, split the test.
- `assert!(matches!(...))` is the idiomatic way to assert a `Result` variant.
- Use `tempfile::tempdir()` for any test that touches the filesystem. Never `/tmp` directly.

### `#[test]` not `#[tokio::test]` unless needed

Most core logic is sync. Don't reach for async in tests just because production uses tokio somewhere.

---

## Integration tests

For end-to-end flows that cross module boundaries. Live in `crates/<crate>/tests/`:

```
crates/superpanels-core/
├── src/
└── tests/
    ├── set_pipeline.rs        ← full apply flow against MockBackend
    ├── library_scan.rs        ← scan a tempdir of fake images
    └── fixtures/
        ├── display/
        │   ├── kscreen-3-monitors.txt
        │   └── xrandr-portrait.txt
        └── images/
            ├── pano-7680x2160.jpg
            └── small-1024x768.png
```

Each `tests/*.rs` is its own crate from Cargo's perspective — it can only call the *public* API. Integration tests are how you find out your public API is hard to use.

### MockBackend

Backends that touch the desktop are tested via a `MockBackend` defined in core's test support:

```rust
// crates/superpanels-core/src/backends/mock.rs (gated on #[cfg(any(test, feature = "test-support"))])

pub struct MockBackend {
    pub applied: Mutex<Vec<(MonitorId, PathBuf)>>,
}

impl WallpaperBackend for MockBackend {
    fn name(&self) -> &str { "mock" }
    fn availability(&self) -> Availability { Availability::Available }
    fn apply(&self, assignments: &[(MonitorRef, PathBuf)]) -> Result<AppliedReport, BackendError> {
        self.applied.lock().unwrap().extend(/* ... */);
        Ok(AppliedReport { monitors_set: assignments.len(), duration: Duration::ZERO, backend: "mock" })
    }
}
```

Integration tests construct one, run the pipeline, and assert on `applied`.

---

## Snapshot tests (`insta`)

For output that's tedious to write `assert_eq!` for: parser results, JSON shapes, CLI human output.

```rust
use insta::assert_debug_snapshot;

#[test]
fn kscreen_doctor_three_monitors_parses() {
    let output = include_str!("fixtures/display/kscreen-3-monitors.txt");
    let monitors = parse_kscreen_doctor(output).unwrap();
    assert_debug_snapshot!(monitors);
}
```

First run, `insta` writes a `.snap` file next to the test. You eyeball it, commit it. Future runs compare; if the output changes, the test fails and shows you the diff. If the change was intentional, run `cargo insta review` (or `INSTA_UPDATE=auto cargo test`) to accept.

**Rules:**
- Snapshots are committed to git.
- Reviewing a snapshot diff is part of code review — treat it like any other diff.
- Don't snapshot anything containing dates, paths under `/home`, or hashes (use `insta`'s redactions).

---

## Property tests (`proptest`)

For algorithms whose correctness is best stated as *invariants* over arbitrary inputs.

The layout math is the canonical example:

```rust
use proptest::prelude::*;

fn arb_monitors() -> impl Strategy<Value = Vec<Monitor>> {
    prop::collection::vec(
        (
            100u32..4096,    // resolution.0
            100u32..4096,    // resolution.1
            100u32..900,     // physical_size_mm.0
            100u32..600,     // physical_size_mm.1
        ).prop_map(|(rw, rh, mw, mh)| /* build a Monitor */),
        1..=8,  // 1 to 8 monitors
    )
}

proptest! {
    #[test]
    fn crops_never_overlap(monitors in arb_monitors()) {
        let placements = synthesise_placements(&monitors);
        let rect = cover_image_rect_mm(&monitors, (10000, 1080));
        let crops = compute_crop_specs(&monitors, &placements, (10000, 1080), rect).unwrap();
        for (i, a) in crops.iter().enumerate() {
            for b in crops.iter().skip(i + 1) {
                prop_assert!(!a.src_rect.intersects(&b.src_rect));
            }
        }
    }

    #[test]
    fn every_monitor_gets_one_crop(monitors in arb_monitors()) {
        let placements = synthesise_placements(&monitors);
        let rect = cover_image_rect_mm(&monitors, (10000, 1080));
        let crops = compute_crop_specs(&monitors, &placements, (10000, 1080), rect).unwrap();
        prop_assert_eq!(crops.len(), monitors.len());
    }
}
```

Property tests find bugs hand-written tests miss because nobody thinks to try a 27-monitor 4K-and-720p mix at 0.7× scale.

When `proptest` finds a failure it minimises automatically and saves the case to `tests/proptest-regressions/` so it gets re-run forever.

---

## Doc tests

Every example in a public-API rustdoc must compile and run. `cargo test` runs them.

```rust
/// Computes crops for the given monitors and image.
///
/// # Example
///
/// ```
/// use superpanels_core::{compute_crop_specs, cover_image_rect_mm, synthesise_placements};
///
/// let monitors   = detect_monitors();
/// let placements = synthesise_placements(&monitors);
/// let rect       = cover_image_rect_mm(&monitors, (1920, 1080));
/// let crops      = compute_crop_specs(&monitors, &placements, (1920, 1080), rect)?;
/// assert_eq!(crops.len(), monitors.len());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn compute_crop_specs(/* ... */) -> Result<Vec<CropSpec>, LayoutError> { /* ... */ }
```

Doc tests double as documentation that *can't lie* — if the API changes, the doc breaks.

`# ` hides a setup line from the rendered docs but still compiles it.

---

## Frontend tests

### Vitest for TS

Pure TS logic (stores, utilities, type-safe IPC wrappers) gets vitest unit tests:

```ts
// ui/src/lib/stores/profile.test.ts
import { describe, it, expect, vi } from 'vitest';
import { profileStore } from './profile';

describe('profileStore', () => {
  it('updates active on apply', async () => {
    // ...
  });
});
```

Run with `npm test`. CI runs them.

### Component tests

`@testing-library/svelte` for components that have meaningful interaction logic. Skip it for trivial display components — visual review covers them.

### Canvas: no auto-test

The monitor canvas's *rendering* is verified by humans; the *math* feeding it is verified by Rust tests (`compute_crop_specs`). Don't try to pixel-diff the canvas — flaky and high-maintenance.

---

## What we deliberately do NOT auto-test

- **Compositor actually painting the wallpaper.** We trust the OS. `MockBackend` proves we sent the right paths.
- **The Tauri webview rendering.** Manual smoke before each release.
- **D-Bus messages reaching KDE.** Mocked in unit tests; manually verified on a real KDE install before release.
- **System tray appearance.** Manual.

These are walked through manually on a real desktop session before tagging a release.

---

## Running tests locally

```sh
# Default — fast unit + integration tests
cargo test

# Or with nextest if installed (faster, prettier output):
cargo nextest run

# Just doc tests:
cargo test --doc

# Just one test:
cargo test single_monitor_no_bezel

# With output (println!/dbg!) shown:
cargo test -- --nocapture

# Update snapshot tests interactively:
cargo insta review

# Frontend:
cd ui && npm test
```

The pre-push hook runs `cargo test --workspace --all-features` — same as CI.

---

## Naming and organising tests

### Unit test names

Form: `<scenario>_<expected_outcome>`.

```
single_monitor_no_bezel_returns_full_image
empty_monitor_list_returns_empty_list_error
two_identical_monitors_with_uniform_bezel_split_evenly
mixed_ppi_normalises_to_max_ppi
```

Read your test names without reading the bodies. They should describe the contract.

### Integration test files

Form: `<flow>.rs`.

```
set_pipeline.rs
slideshow_resume.rs
config_round_trip.rs
```

Each file is one feature flow.

### Fixtures

Live under `tests/fixtures/<category>/<descriptive-name>.<ext>`. Name them so reading the directory listing tells the story.

---

## Writing a good test

**Arrange / Act / Assert.** Visually separate the three:

```rust
#[test]
fn slideshow_skips_recent_history() {
    // Arrange
    let images = vec![path("a.jpg"), path("b.jpg"), path("c.jpg")];
    let mut sl = Slideshow::new(images, recent_history_size: 2);
    sl.advance();  // shows a.jpg
    sl.advance();  // shows b.jpg

    // Act
    let next = sl.peek_next();

    // Assert
    assert_eq!(next, &path("c.jpg"));
}
```

**One assertion concept per test.** Multiple `assert!` lines on the same fact are fine. Multiple `assert!` lines on *different* facts means you have multiple tests pretending to be one.

**Failures should be diagnosable from the message.** `assert_eq!` does this for free; for custom assertions use `assert!(condition, "explanation: actual = {actual:?}")`.

**Tests should be deterministic.** No real time, no real filesystem outside `tempfile`, no real network, no real subprocesses (mock detectors with captured fixtures), no random except via `proptest`'s seeded RNG.

**Fast.** A single test should run in milliseconds. The whole suite stays under 30 seconds even at 1k tests.
