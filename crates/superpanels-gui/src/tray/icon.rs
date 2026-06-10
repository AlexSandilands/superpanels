//! Tray icon style (white glyph vs. blue app icon), persisted to `tray.json`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TrayIconStyle {
    #[default]
    White,
    Blue,
}

impl TrayIconStyle {
    pub(crate) fn parse(s: &str) -> Option<Self> {
        match s {
            "white" => Some(Self::White),
            "blue" => Some(Self::Blue),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::White => "white",
            Self::Blue => "blue",
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct TrayState {
    style: TrayIconStyle,
}

const FILE_NAME: &str = "tray.json";

fn style_path() -> Option<PathBuf> {
    crate::window_state::state_dir().map(|d| d.join(FILE_NAME))
}

pub(crate) fn load_style() -> TrayIconStyle {
    style_path().map(|p| load_from(&p)).unwrap_or_default()
}

pub(crate) fn save_style(style: TrayIconStyle) -> std::io::Result<()> {
    let Some(path) = style_path() else {
        return Ok(());
    };
    save_to(&path, style)
}

fn load_from(path: &Path) -> TrayIconStyle {
    let Ok(bytes) = fs::read(path) else {
        return TrayIconStyle::default();
    };
    serde_json::from_slice::<TrayState>(&bytes)
        .map(|s| s.style)
        .unwrap_or_default()
}

fn save_to(path: &Path, style: TrayIconStyle) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_vec_pretty(&TrayState { style })
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(path, body)
}

pub(crate) fn image(style: TrayIconStyle) -> tauri::image::Image<'static> {
    // PNG bytes embedded at build time. Falls back to a 1x1 transparent
    // image only if decode fails, so tray construction can never panic.
    const WHITE_PNG: &[u8] = include_bytes!("../../icons/tray.png");
    const BLUE_PNG: &[u8] = include_bytes!("../../icons/32x32.png");
    let bytes = match style {
        TrayIconStyle::White => WHITE_PNG,
        TrayIconStyle::Blue => BLUE_PNG,
    };
    tauri::image::Image::from_bytes(bytes)
        .unwrap_or_else(|_| tauri::image::Image::new_owned(vec![0; 4], 1, 1))
}

/// Only the white glyph may be recoloured as a monochrome template mask;
/// the blue icon must keep its colour.
pub(crate) fn is_template(style: TrayIconStyle) -> bool {
    style == TrayIconStyle::White
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parse_accepts_known_styles_and_rejects_others() {
        assert_eq!(TrayIconStyle::parse("white"), Some(TrayIconStyle::White));
        assert_eq!(TrayIconStyle::parse("blue"), Some(TrayIconStyle::Blue));
        assert_eq!(TrayIconStyle::parse("White"), None);
        assert_eq!(TrayIconStyle::parse(""), None);
    }

    #[test]
    fn as_str_round_trips_through_parse() {
        for style in [TrayIconStyle::White, TrayIconStyle::Blue] {
            assert_eq!(TrayIconStyle::parse(style.as_str()), Some(style));
        }
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        save_to(&path, TrayIconStyle::Blue).unwrap();
        assert_eq!(load_from(&path), TrayIconStyle::Blue);
    }

    #[test]
    fn load_from_missing_returns_white_default() {
        let dir = tempdir().unwrap();
        assert_eq!(
            load_from(&dir.path().join("nope.json")),
            TrayIconStyle::White
        );
    }

    #[test]
    fn load_from_garbage_or_unknown_style_returns_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, b"not json").unwrap();
        assert_eq!(load_from(&path), TrayIconStyle::White);
        std::fs::write(&path, br#"{ "style": "plaid" }"#).unwrap();
        assert_eq!(load_from(&path), TrayIconStyle::White);
    }

    #[test]
    fn both_styles_decode_to_real_images() {
        // Guards the embedded PNG assets: a corrupt one would silently fall
        // back to the 1x1 transparent image and the tray would look "missing".
        for style in [TrayIconStyle::White, TrayIconStyle::Blue] {
            let img = image(style);
            assert!(img.width() > 1, "{} icon failed to decode", style.as_str());
        }
    }
}
