//! Criterion benchmark for the GNOME composite path (`SPEC.md` §10.4, §19).
//!
//! Exercises `composite_band` + `downscale_if_needed` for a 3-monitor 4K
//! span. `SPEC §19` budgets `superpanels set <single image>` to under
//! 500 ms for that exact shape; this bench is the upper-bound microbench
//! for the GNOME path's compositing portion (decode + backend `set` are
//! covered by `image.rs` and integration tests respectively).
#![allow(clippy::unwrap_used)] // reason: bench inputs are deterministic; aborting on misconfiguration is fine
#![allow(clippy::expect_used)] // reason: same as above
#![allow(missing_docs)] // reason: bench harness functions are not part of the crate's public API

use criterion::{Criterion, criterion_group, criterion_main};
use image::{DynamicImage, Rgba, RgbaImage};
use std::hint::black_box;

use superpanels_core::backends::gnome::{MAX_LONG_EDGE, composite_band, downscale_if_needed};

const FOUR_K_W: u32 = 3840;
const FOUR_K_H: u32 = 2160;
const MONITORS: usize = 3;

fn synthetic_crop(width: u32, height: u32, seed: u8) -> DynamicImage {
    let mut img = RgbaImage::new(width, height);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        // reason: cast loses precision but values are bounded to 0..=255
        #[allow(clippy::cast_possible_truncation)]
        let r = ((x.wrapping_add(u32::from(seed)) * 13) & 0xFF) as u8;
        #[allow(clippy::cast_possible_truncation)]
        let g = ((y * 17) & 0xFF) as u8;
        *pixel = Rgba([r, g, seed, 255]);
    }
    DynamicImage::ImageRgba8(img)
}

fn three_monitor_crops() -> Vec<DynamicImage> {
    (0..MONITORS)
        .map(|i| {
            // reason: bench fixture; seed is a small index, no truncation possible.
            #[allow(clippy::cast_possible_truncation)]
            let seed = i as u8;
            synthetic_crop(FOUR_K_W, FOUR_K_H, seed)
        })
        .collect()
}

fn bench_composite_band(c: &mut Criterion) {
    let crops = three_monitor_crops();

    let mut group = c.benchmark_group("gnome_composite");
    group.sample_size(10);

    group.bench_function("composite_band_3x4k", |b| {
        b.iter(|| {
            let canvas = composite_band(black_box(&crops)).unwrap();
            black_box(canvas);
        });
    });

    group.finish();
}

fn bench_downscale(c: &mut Criterion) {
    // The composited 3x4K canvas is 11_520 wide — over the 8K cap, so
    // downscale_if_needed actually resamples here.
    let crops = three_monitor_crops();
    let composited = composite_band(&crops).unwrap();

    let mut group = c.benchmark_group("gnome_downscale");
    group.sample_size(10);

    group.bench_function("no_op_under_cap", |b| {
        // RgbaImage of 1x1 — well under any cap; should short-circuit.
        let small = RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 255]));
        b.iter(|| {
            let out = downscale_if_needed(black_box(small.clone()), black_box(MAX_LONG_EDGE));
            black_box(out);
        });
    });

    group.bench_function("active_downscale_3x4k_to_cap", |b| {
        b.iter(|| {
            let out = downscale_if_needed(black_box(composited.clone()), black_box(MAX_LONG_EDGE));
            black_box(out);
        });
    });

    group.finish();
}

criterion_group!(gnome, bench_composite_band, bench_downscale);
criterion_main!(gnome);
