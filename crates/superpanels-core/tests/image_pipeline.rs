//! Integration test for the §1.4 pipeline.
//!
//! Builds a real PNG on disk, runs `compute_crop_specs` against a
//! 2-monitor layout, then exercises `load → crop → save_temp` end-to-end
//! and confirms each output's dimensions.

#![allow(clippy::unwrap_used)] // reason: integration test fails loudly on errors

use std::path::PathBuf;

use image::{DynamicImage, RgbaImage};
use tempfile::tempdir;

use superpanels_core::display::{Monitor, MonitorId, Rotation};
use superpanels_core::image::{clear_temp_dir_at, crop, load, save_temp_in};
use superpanels_core::layout::{ImageRectMm, compute_crop_specs, synthesise_placements};

fn solid_image(w: u32, h: u32) -> DynamicImage {
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(
        w,
        h,
        image::Rgba([200, 200, 200, 255]),
    ))
}

fn monitor(id: u32, name: &str, x: i32, w_px: u32, h_px: u32, w_mm: u32, h_mm: u32) -> Monitor {
    Monitor {
        id: MonitorId(id),
        name: name.to_owned(),
        stable_id: None,
        position: (x, 0),
        resolution: (w_px, h_px),
        physical_size_mm: Some((f64::from(w_mm), f64::from(h_mm))),
        scale: 1.0,
        rotation: Rotation::None,
        refresh_hz: None,
        ppi: None,
    }
}

#[test]
fn full_compute_crop_save_pipeline_writes_per_monitor_files() {
    // Arrange — write a real PNG and build a 2-monitor layout.
    let dir = tempdir().unwrap();
    let src_path: PathBuf = dir.path().join("source.png");
    let source = solid_image(3840, 1080);
    source.save(&src_path).unwrap();
    let temp = dir.path().join("temp");
    clear_temp_dir_at(&temp).unwrap();

    let monitors = vec![
        monitor(0, "DP-1", 0, 1920, 1080, 480, 270),
        monitor(1, "DP-2", 1920, 1920, 1080, 480, 270),
    ];
    let placements = synthesise_placements(&monitors);

    // Act — compute, then crop + save each monitor's slice.
    // Image rect spans both monitors at 1:1 mm (480 + 480 = 960 mm wide).
    let rect = ImageRectMm {
        x_mm: 0.0,
        y_mm: 0.0,
        w_mm: 960.0,
        h_mm: 270.0,
    };
    let crops = compute_crop_specs(&monitors, &placements, (3840, 1080), rect).unwrap();
    let loaded = load(&src_path).unwrap();
    let mut written = Vec::new();
    for c in &crops {
        let piece = crop(&loaded, c.src_rect).unwrap();
        let name = format!("monitor-{}.png", c.monitor_id.0);
        let out = save_temp_in(&piece, &name, &temp).unwrap();
        written.push((c.dst_size, out));
    }

    // Assert — each output exists and has the expected source dimensions.
    assert_eq!(written.len(), 2);
    for ((dst_w, dst_h), path) in &written {
        assert!(path.exists(), "missing temp file at {}", path.display());
        let reread = load(path).unwrap();
        // crop() preserves source-image pixel dims; resampling to dst is a
        // separate later step. Confirm the crop produced the expected width.
        assert_eq!(reread.width(), 1920);
        assert_eq!(reread.height(), 1080);
        assert_eq!(*dst_w, 1920);
        assert_eq!(*dst_h, 1080);
    }
}
