//! Criterion benchmark for [`scan_folder`] (PLAN.md §2.4 cross-cutting
//! "Performance baselines" — `SPEC.md` §19 budget item).
#![allow(clippy::unwrap_used)] // reason: bench inputs are deterministic; aborting on misconfiguration is fine
#![allow(clippy::expect_used)] // reason: same as above
#![allow(missing_docs)] // reason: bench harness functions are not part of the crate's public API

use std::path::Path;

use criterion::{Criterion, criterion_group, criterion_main};
use image::{ImageBuffer, Rgba};
use std::hint::black_box;

use superpanels_core::library::scan_folder;

const IMAGE_COUNT: usize = 100;
const IMAGE_DIM: u32 = 8;

fn populate(root: &Path) {
    let pixel = ImageBuffer::<Rgba<u8>, _>::from_pixel(IMAGE_DIM, IMAGE_DIM, Rgba([0, 0, 0, 255]));
    for i in 0..IMAGE_COUNT {
        let path = root.join(format!("img-{i:04}.png"));
        pixel.save(&path).unwrap();
    }
}

fn bench_scan_folder(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    populate(dir.path());

    c.bench_function("scan_folder_100_pngs", |b| {
        b.iter(|| {
            let entries = scan_folder(black_box(dir.path()), black_box(false), |_| {});
            assert_eq!(entries.len(), IMAGE_COUNT);
        });
    });
}

criterion_group!(library, bench_scan_folder);
criterion_main!(library);
