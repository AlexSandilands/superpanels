//! Criterion benchmarks for the image pipeline" — the dominant cost in's
//! `superpanels set <single image>` end-to-end < 500 ms (4K, 3-monitor)
//! budget).
//!
//! Each routine is benched at 4K (3840x2160) and 8K (7680x4320) source
//! sizes. The bench writes the source PNG to `tempfile::tempdir()` once
//! per group and reuses it across iterations so the measurement is the
//! image op, not tempdir setup.
//!
//! Numbers from these runs are pasted into the commit message as the
//! Phase 1 image baseline; Phase 6.2 compares future runs against them.
//!
//! `unwrap` is acceptable in bench code: an aborted bench is the right
//! outcome if the canonical inputs ever stop producing a valid image.
#![allow(clippy::unwrap_used)] // reason: bench inputs are static; aborting on a misconfigured fixture is fine
#![allow(clippy::expect_used)] // reason: same as above
#![allow(missing_docs)] // reason: bench harness functions are not part of the crate's public API

use std::path::PathBuf;

use criterion::{Criterion, criterion_group, criterion_main};
use image::{DynamicImage, Rgba, RgbaImage};
use std::hint::black_box;
use tempfile::TempDir;

use superpanels_core::image::{FitMode, crop, load, scale_to_fit};
use superpanels_core::layout::Rect;

const FOUR_K: (u32, u32) = (3840, 2160);
const EIGHT_K: (u32, u32) = (7680, 4320);

/// Build a synthetic, gradient-ish image of the given size. The exact
/// content doesn't matter — Lanczos3 work is dimension-bound, not
/// content-bound — but a non-uniform image keeps any future content-aware
/// fast paths honest.
fn synthetic_image(size: (u32, u32)) -> DynamicImage {
    let (w, h) = size;
    let mut img = RgbaImage::new(w, h);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        // reason: cast loses precision but the values are bounded to 0..=255
        #[allow(clippy::cast_possible_truncation)]
        let r = ((x * 255) / w.max(1)) as u8;
        #[allow(clippy::cast_possible_truncation)]
        let g = ((y * 255) / h.max(1)) as u8;
        *pixel = Rgba([r, g, 128, 255]);
    }
    DynamicImage::ImageRgba8(img)
}

/// Persist `img` as PNG into `dir`/`name` and return the path.
fn write_png(dir: &TempDir, name: &str, img: &DynamicImage) -> PathBuf {
    let path = dir.path().join(name);
    img.save(&path).unwrap();
    path
}

fn bench_load(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    let img_4k = synthetic_image(FOUR_K);
    let img_8k = synthetic_image(EIGHT_K);
    let path_4k = write_png(&dir, "src_4k.png", &img_4k);
    let path_8k = write_png(&dir, "src_8k.png", &img_8k);

    let mut group = c.benchmark_group("image_load");
    group.sample_size(20);

    group.bench_function("load_4k_png", |b| {
        b.iter(|| load(black_box(&path_4k)).unwrap());
    });
    group.bench_function("load_8k_png", |b| {
        b.iter(|| load(black_box(&path_8k)).unwrap());
    });

    group.finish();
}

fn bench_scale_to_fit(c: &mut Criterion) {
    let img_4k = synthetic_image(FOUR_K);
    let img_8k = synthetic_image(EIGHT_K);

    let mut group = c.benchmark_group("image_scale_to_fit_fill");
    group.sample_size(20);

    // 4K source → 1920x1080 target (downscale; the canvas-thumbnail case).
    group.bench_function("scale_4k_to_fhd_fill", |b| {
        b.iter(|| {
            let _ = scale_to_fit(
                black_box(&img_4k),
                black_box((1920, 1080)),
                black_box(FitMode::Fill),
            );
        });
    });

    // 4K source → 4K target (no-op-ish; Lanczos3 still runs on Fill).
    group.bench_function("scale_4k_to_4k_fill", |b| {
        b.iter(|| {
            let _ = scale_to_fit(
                black_box(&img_4k),
                black_box(FOUR_K),
                black_box(FitMode::Fill),
            );
        });
    });

    // 8K source → 4K target (the 3-monitor 4K span dominant cost).
    group.bench_function("scale_8k_to_4k_fill", |b| {
        b.iter(|| {
            let _ = scale_to_fit(
                black_box(&img_8k),
                black_box(FOUR_K),
                black_box(FitMode::Fill),
            );
        });
    });

    group.finish();
}

fn bench_crop(c: &mut Criterion) {
    let img_4k = synthetic_image(FOUR_K);
    let img_8k = synthetic_image(EIGHT_K);

    let mut group = c.benchmark_group("image_crop");
    group.sample_size(20);

    // 4K source, crop one of three roughly-equal vertical slices.
    let rect_4k = Rect {
        x: FOUR_K.0 / 3,
        y: 0,
        w: FOUR_K.0 / 3,
        h: FOUR_K.1,
    };
    group.bench_function("crop_4k_third_slice", |b| {
        b.iter(|| crop(black_box(&img_4k), black_box(rect_4k)).unwrap());
    });

    // 8K source, crop one of three slices.
    let rect_8k = Rect {
        x: EIGHT_K.0 / 3,
        y: 0,
        w: EIGHT_K.0 / 3,
        h: EIGHT_K.1,
    };
    group.bench_function("crop_8k_third_slice", |b| {
        b.iter(|| crop(black_box(&img_8k), black_box(rect_8k)).unwrap());
    });

    group.finish();
}

criterion_group!(image, bench_load, bench_scale_to_fit, bench_crop);
criterion_main!(image);
