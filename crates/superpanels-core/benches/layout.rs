//! Criterion benchmarks for [`compute_crop_specs`].
//!
//! Captures end-to-end timings for representative monitor counts (1, 3, 6, 9)
//! against a plausible panoramic source image. Numbers from these runs are
//! pasted into the commit message as the Phase 1 baseline; Phase 6.2 compares
//! future runs against them.
//!
//! `unwrap` is acceptable in bench code: an aborted bench is the right outcome
//! if the canonical inputs ever stop producing a valid layout.
#![allow(clippy::unwrap_used)] // reason: bench inputs are static; aborting on a misconfigured fixture is fine
#![allow(clippy::expect_used)] // reason: same as above
#![allow(missing_docs)] // reason: bench harness functions are not part of the crate's public API

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

use superpanels_core::display::MonitorId;
use superpanels_core::layout::{cover_image_rect_mm, synthesise_placements};
use superpanels_core::{Monitor, Rotation, compute_crop_specs};

/// A wide panoramic source matching what a real spanned-wallpaper input looks
/// like (8K-ish across, ultrawide aspect).
const IMAGE_SIZE: (u32, u32) = (7680, 2160);

/// Build a monitor at a given grid cell. Resolutions and physical sizes are
/// uniform across the bench fixtures so the only varying input is monitor
/// count and arrangement.
fn make_monitor(id: u32, col: u32, row: u32, res: (u32, u32), physical_mm: (u32, u32)) -> Monitor {
    let (w_px, h_px) = res;
    let x = i32::try_from(col * w_px).unwrap();
    let y = i32::try_from(row * h_px).unwrap();
    Monitor {
        id: MonitorId(id),
        name: format!("DP-{id}"),
        stable_id: None,
        position: (x, y),
        resolution: res,
        physical_size_mm: Some((f64::from(physical_mm.0), f64::from(physical_mm.1))),
        scale: 1.0,
        rotation: Rotation::None,
        refresh_hz: None,
        primary: id == 0,
        ppi: None,
    }
}

/// One 1920x1080 panel, no bezel (single-monitor baseline).
fn fixture_1_monitor() -> Vec<Monitor> {
    vec![make_monitor(0, 0, 0, (1920, 1080), (480, 270))]
}

/// Three identical 2560x1440 panels in a single horizontal row.
fn fixture_3_monitors() -> Vec<Monitor> {
    (0..3)
        .map(|i| make_monitor(i, i, 0, (2560, 1440), (597, 336)))
        .collect()
}

/// Six identical 2560x1440 panels arranged 3 wide x 2 tall.
fn fixture_6_monitors() -> Vec<Monitor> {
    let mut out = Vec::with_capacity(6);
    for row in 0..2 {
        for col in 0..3 {
            let id = row * 3 + col;
            out.push(make_monitor(id, col, row, (2560, 1440), (597, 336)));
        }
    }
    out
}

/// Nine identical 2560x1440 panels arranged 3x3.
fn fixture_9_monitors() -> Vec<Monitor> {
    let mut out = Vec::with_capacity(9);
    for row in 0..3 {
        for col in 0..3 {
            let id = row * 3 + col;
            out.push(make_monitor(id, col, row, (2560, 1440), (597, 336)));
        }
    }
    out
}

fn bench_compute_crop_specs(c: &mut Criterion) {
    let m1 = fixture_1_monitor();
    let m3 = fixture_3_monitors();
    let m6 = fixture_6_monitors();
    let m9 = fixture_9_monitors();
    let p1 = synthesise_placements(&m1);
    let p3 = synthesise_placements(&m3);
    let p6 = synthesise_placements(&m6);
    let p9 = synthesise_placements(&m9);
    let r1 = cover_image_rect_mm(&m1, IMAGE_SIZE);
    let r3 = cover_image_rect_mm(&m3, IMAGE_SIZE);
    let r6 = cover_image_rect_mm(&m6, IMAGE_SIZE);
    let r9 = cover_image_rect_mm(&m9, IMAGE_SIZE);

    c.bench_function("1_monitor_cover", |b| {
        b.iter(|| {
            compute_crop_specs(
                black_box(&m1),
                black_box(&p1),
                black_box(IMAGE_SIZE),
                black_box(r1),
            )
            .unwrap()
        });
    });

    c.bench_function("3_monitors_cover", |b| {
        b.iter(|| {
            compute_crop_specs(
                black_box(&m3),
                black_box(&p3),
                black_box(IMAGE_SIZE),
                black_box(r3),
            )
            .unwrap()
        });
    });

    c.bench_function("6_monitors_cover", |b| {
        b.iter(|| {
            compute_crop_specs(
                black_box(&m6),
                black_box(&p6),
                black_box(IMAGE_SIZE),
                black_box(r6),
            )
            .unwrap()
        });
    });

    c.bench_function("9_monitors_cover", |b| {
        b.iter(|| {
            compute_crop_specs(
                black_box(&m9),
                black_box(&p9),
                black_box(IMAGE_SIZE),
                black_box(r9),
            )
            .unwrap()
        });
    });
}

criterion_group!(layout, bench_compute_crop_specs);
criterion_main!(layout);
