# 8. Image processing & colour

Built on the `image` crate. No `unsafe` code in our layer.

## 8.1 Operations

```rust
fn load(path: &Path) -> Result<DynamicImage>;       // returns clear error on unsupported format
fn crop(img: &DynamicImage, rect: Rect) -> DynamicImage;
fn scale(img: &DynamicImage, target: (u32, u32), filter: ScaleFilter) -> DynamicImage;
fn rotate(img: &DynamicImage, rotation: Rotation) -> DynamicImage;
fn save_temp(img: &DynamicImage, name: &str) -> Result<PathBuf>;
```

`ScaleFilter` defaults to `Lanczos3`. `Triangle` is offered for when speed matters more than quality (preview canvas — though preview never resamples the full image, see §12.3).

## 8.2 Fit modes

- `Fill` — scale until the image fills the total physical canvas, cropping the overflow. Default.
- `Fit` — letterbox/pillarbox so the entire image is visible. The user can pick the bar colour (default: black).
- `Stretch` — distort to fill exactly. Offered for completeness; rarely useful.
- `Center` — no scaling, centre the image on the canvas, crop or pad.

## 8.3 Image position offset

When `Fill` produces a canvas larger than the image area in one axis (or vice-versa), the user can slide the image along that axis via the GUI (`offset_px` IPC parameter), or via `--offset X,Y` on the CLI. Offset is per-profile and persists.

## 8.4 Colour management

v1 assumes images are in sRGB and the compositor displays sRGB. We do not embed or strip ICC profiles; we don't transform colour spaces. This is documented as a known limitation. Wide-gamut handling is a v2+ topic.

## 8.5 Temp file lifecycle

Processed per-monitor images are written to `$XDG_CACHE_HOME/superpanels/temp/`. On every apply, the temp directory is cleared *before* new files are written. The backend always receives the temp file paths, never the originals. Filenames include a content hash so a stale file isn't silently re-used.

## 8.6 Memory caps

The `image` crate decodes lazily where possible. A single decoded `DynamicImage` for an 8K wide pano is ~190 MB at 8-bit RGBA. The library never holds more than one full-res `DynamicImage` at a time per worker; processing pipelines stream where they can.
