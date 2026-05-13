//! Criterion benchmark for [`scan_folder`] and [`load_index`].
//!
//! `scan_folder` is parameterised over `[100, 1_000, 10_000]` inputs to expose
//! the walk + header-parse cost at realistic library sizes. The 10k case
//! reuses one tiny PNG copied per file rather than encoding 10k images, so
//! the bench measures directory walk + extension filter + header read, not
//! synthetic encode.
//!
//! `load_index` (warm cache) is benched on the persisted JSON shape produced
//! by `scan_folder`, so future improvements to the on-disk index format are
//! covered.
#![allow(clippy::unwrap_used)] // reason: bench inputs are deterministic; aborting on misconfiguration is fine
#![allow(clippy::expect_used)] // reason: same as above
#![allow(missing_docs)] // reason: bench harness functions are not part of the crate's public API

use std::path::Path;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use image::{ImageBuffer, Rgba};
use std::hint::black_box;

use superpanels_core::library::{load_index, persist_index, scan_folder};

const SIZES: [usize; 3] = [100, 1_000, 10_000];
const IMAGE_DIM: u32 = 8;

/// Write `count` distinct png files into `root`. The image is encoded once and the
/// resulting bytes are reused per file — sufficient for bench inputs because
/// `scan_folder` only reads PNG headers, not pixel data.
fn populate(root: &Path, count: usize) {
    let pixel = ImageBuffer::<Rgba<u8>, _>::from_pixel(IMAGE_DIM, IMAGE_DIM, Rgba([0, 0, 0, 255]));
    let mut bytes = std::io::Cursor::new(Vec::<u8>::new());
    image::DynamicImage::ImageRgba8(pixel)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    let bytes = bytes.into_inner();
    for i in 0..count {
        let path = root.join(format!("img-{i:06}.png"));
        std::fs::write(&path, &bytes).unwrap();
    }
}

fn bench_scan_folder(c: &mut Criterion) {
    let mut group = c.benchmark_group("scan_folder");
    group.sample_size(10);

    for size in SIZES {
        let dir = tempfile::tempdir().unwrap();
        populate(dir.path(), size);
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let entries = scan_folder(black_box(dir.path()), black_box(false), |_| {});
                assert_eq!(entries.len(), size);
            });
        });
    }

    group.finish();
}

fn bench_load_index(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_index_warm");
    group.sample_size(20);

    for size in SIZES {
        let dir = tempfile::tempdir().unwrap();
        populate(dir.path(), size);
        let entries = scan_folder(dir.path(), false, |_| {});
        let index_path = dir.path().join("library-index.json");
        persist_index(&entries, &index_path).unwrap();
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let loaded = load_index(black_box(&index_path)).unwrap();
                assert_eq!(loaded.len(), size);
            });
        });
    }

    group.finish();
}

criterion_group!(library, bench_scan_folder, bench_load_index);
criterion_main!(library);
