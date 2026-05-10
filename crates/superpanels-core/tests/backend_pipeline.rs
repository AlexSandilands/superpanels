//! End-to-end test: `compute_crop_specs` → image pipeline → `MockBackend`.
//!
//! Proves that the `WallpaperBackend` trait is exercised the same way the
//! real backends will be — assignments are `(MonitorRef, PathBuf)` pairs
//! pointing at the temp files written by `save_temp_in`.

#![allow(clippy::unwrap_used)] // reason: integration test fails loudly on errors

use std::path::PathBuf;

use image::{DynamicImage, RgbaImage};
use tempfile::tempdir;

use superpanels_core::backends::{MockBackend, WallpaperBackend};
use superpanels_core::display::{Monitor, MonitorId, MonitorRef, Rotation};
use superpanels_core::image::{clear_temp_dir_at, crop, load, save_temp_in};
use superpanels_core::layout::{FitMode, compute_crop_specs, synthesise_placements};

fn solid_image(w: u32, h: u32) -> DynamicImage {
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(w, h, image::Rgba([180, 90, 45, 255])))
}

fn monitor(id: u32, name: &str, x: i32) -> Monitor {
    Monitor {
        id: MonitorId(id),
        name: name.to_owned(),
        stable_id: Some(format!("{name}-uuid")),
        position: (x, 0),
        resolution: (1920, 1080),
        physical_size_mm: Some((480.0, 270.0)),
        scale: 1.0,
        rotation: Rotation::None,
        refresh_hz: None,
        primary: id == 0,
        ppi: None,
    }
}

#[test]
fn pipeline_dispatches_per_monitor_pairs_to_mock_backend() {
    // Arrange
    let dir = tempdir().unwrap();
    let src_path: PathBuf = dir.path().join("source.png");
    solid_image(3840, 1080).save(&src_path).unwrap();
    let temp = dir.path().join("temp");
    clear_temp_dir_at(&temp).unwrap();

    let monitors = vec![monitor(0, "DP-1", 0), monitor(1, "DP-2", 1920)];
    let placements = synthesise_placements(&monitors);
    let crops = compute_crop_specs(&monitors, &placements, FitMode::Fill, (3840, 1080)).unwrap();
    let loaded = load(&src_path).unwrap();
    let mut assignments = Vec::new();
    for (m, c) in monitors.iter().zip(crops.iter()) {
        let piece = crop(&loaded, c.src_rect).unwrap();
        let path = save_temp_in(&piece, &format!("m-{}.png", m.id.0), &temp).unwrap();
        assignments.push((
            MonitorRef {
                stable_id: m.stable_id.clone().unwrap_or_default(),
                name: m.name.clone(),
            },
            path,
        ));
    }

    let backend = MockBackend::new();

    // Act
    let report = backend.apply(&assignments).unwrap();

    // Assert
    assert_eq!(report.monitors_set, 2);
    assert_eq!(report.backend, "mock");
    let recorded = backend.recorded();
    assert_eq!(recorded.len(), 2);
    assert_eq!(recorded[0].0.name, "DP-1");
    assert_eq!(recorded[1].0.name, "DP-2");
    assert!(recorded[0].1.exists());
    assert!(recorded[1].1.exists());
}
