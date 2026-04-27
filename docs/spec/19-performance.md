# 19. Performance targets

| Operation | Target | Measured on |
|---|---|---|
| `superpanels set <single image>` end-to-end (excl. compositor redraw) | < 500 ms | 4K image, 3-monitor setup, NVMe |
| `superpanels detect` | < 200 ms | KDE Wayland on Ryzen 5600 |
| Canvas drag → redraw frame | < 8 ms (≥ 120 fps) | Ryzen 5600 / iGPU |
| Library scan, 5,000 images | < 10 s cold | NVMe |
| Library scan, 5,000 images | < 1 s warm (cached metadata) | NVMe |
| Thumbnail generation, single 4K image | < 200 ms | Ryzen 5600 |
| Daemon idle CPU | < 0.1% | Any |
| Daemon resident memory, idle, 1k library | < 60 MB | Any |

Performance regressions are tracked in `criterion` benchmarks under `crates/superpanels-core/benches/`.
