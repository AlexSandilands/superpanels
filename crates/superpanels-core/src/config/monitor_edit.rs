//! Add or update a `[[monitor]]` block on disk while preserving comments.
//!
//! Driven by `superpanels monitor configure <NAME-OR-ID> [...flags]`
//! (PLAN §1.6). Uses `toml_edit` so existing comments survive — the user
//! has likely added their own notes between blocks.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;
use toml_edit::{Array, ArrayOfTables, DocumentMut, Item, Table, value};

/// How the caller identified the target monitor.
///
/// Either a stable id (KDE per-output UUID, EDID hash) or a name
/// (`"DP-1"`); we update — or create — the matching `[[monitor]]` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorIdentifier {
    /// Match against the block's `stable_id` field.
    StableId(String),
    /// Match against the block's `name` field.
    Name(String),
}

/// Errors returned from [`write_monitor_block`].
#[derive(Debug, Error)]
pub enum MonitorEditError {
    /// Could not read the on-disk config.
    #[error("could not read config at {path}: {source}")]
    Read {
        /// File the I/O attempt was against.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// Could not write the on-disk config.
    #[error("could not write config at {path}: {source}")]
    Write {
        /// File the I/O attempt was against.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// `toml_edit` could not parse the existing file.
    #[error("could not parse config at {path}: {message}")]
    Parse {
        /// File the parse attempt was against.
        path: PathBuf,
        /// Parser-supplied message.
        message: String,
    },
}

/// Add or update one `[[monitor]]` block in `path` matching `identifier`,
/// setting `physical_mm = [w, h]`. Other fields on a matched block are
/// preserved; comments and surrounding formatting are preserved.
///
/// If no block matches, a new one is appended. The file is created (with
/// just the new block) when it doesn't exist yet.
///
/// # Errors
///
/// Returns [`MonitorEditError::Read`] / [`MonitorEditError::Write`] /
/// [`MonitorEditError::Parse`] depending on which step fails.
pub fn write_monitor_block(
    path: &Path,
    identifier: &MonitorIdentifier,
    physical_mm: [u32; 2],
) -> Result<(), MonitorEditError> {
    let existing = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            return Err(MonitorEditError::Read {
                path: path.to_owned(),
                source: e,
            });
        }
    };

    let mut doc: DocumentMut = if existing.is_empty() {
        DocumentMut::new()
    } else {
        existing
            .parse()
            .map_err(|e: toml_edit::TomlError| MonitorEditError::Parse {
                path: path.to_owned(),
                message: e.to_string(),
            })?
    };

    let monitors = ensure_monitor_array(&mut doc);
    let target = monitors
        .iter_mut()
        .find(|t| block_matches(t, identifier))
        .map(|t| {
            apply_block_fields(t, identifier, physical_mm);
        });
    if target.is_none() {
        let mut t = Table::new();
        apply_block_fields(&mut t, identifier, physical_mm);
        monitors.push(t);
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| MonitorEditError::Write {
            path: parent.to_owned(),
            source: e,
        })?;
    }
    fs::write(path, doc.to_string()).map_err(|e| MonitorEditError::Write {
        path: path.to_owned(),
        source: e,
    })
}

fn ensure_monitor_array(doc: &mut DocumentMut) -> &mut ArrayOfTables {
    let item = doc
        .as_table_mut()
        .entry("monitor")
        .or_insert_with(|| Item::ArrayOfTables(ArrayOfTables::new()));
    if item.as_array_of_tables_mut().is_none() {
        // Existing `monitor = ...` was scalar, not [[monitor]]; replace it
        // with an empty array of tables. The user's previous value is lost,
        // but only because the on-disk shape was already wrong for this
        // schema.
        *item = Item::ArrayOfTables(ArrayOfTables::new());
    }
    // reason: `is_none` checked + replaced above, so this must succeed.
    #[allow(clippy::expect_used)]
    item.as_array_of_tables_mut()
        .expect("normalised to ArrayOfTables above")
}

fn block_matches(block: &Table, identifier: &MonitorIdentifier) -> bool {
    match identifier {
        MonitorIdentifier::StableId(id) => block
            .get("stable_id")
            .and_then(|i| i.as_str())
            .is_some_and(|s| s == id),
        MonitorIdentifier::Name(name) => block
            .get("name")
            .and_then(|i| i.as_str())
            .is_some_and(|s| s == name),
    }
}

fn apply_block_fields(block: &mut Table, identifier: &MonitorIdentifier, physical_mm: [u32; 2]) {
    match identifier {
        MonitorIdentifier::StableId(id) => {
            if !block.contains_key("stable_id") {
                block.insert("stable_id", value(id.as_str()));
            }
        }
        MonitorIdentifier::Name(name) => {
            if !block.contains_key("name") {
                block.insert("name", value(name.as_str()));
            }
        }
    }
    let mut arr = Array::new();
    arr.push(i64::from(physical_mm[0]));
    arr.push(i64::from(physical_mm[1]));
    block.insert("physical_mm", value(arr));
}

/// Compute physical mm from a diagonal length and an aspect ratio.
///
/// `diagonal_inches` is the typical advertised diagonal (e.g. 27.0 for a
/// 27" panel); `aspect_w` / `aspect_h` come from the user's `--aspect W:H`
/// flag.
///
/// # Example
///
/// ```
/// # use superpanels_core::config::diagonal_to_mm;
/// let [w, h] = diagonal_to_mm(27.0, 16, 9);
/// assert!((w as i32 - 597).abs() <= 2);
/// assert!((h as i32 - 336).abs() <= 2);
/// ```
#[must_use]
pub fn diagonal_to_mm(diagonal_inches: f64, aspect_w: u32, aspect_h: u32) -> [u32; 2] {
    let aw = f64::from(aspect_w);
    let ah = f64::from(aspect_h);
    let diag_mm = diagonal_inches * 25.4;
    let scale = diag_mm / (aw * aw + ah * ah).sqrt();
    let w = (aw * scale).round();
    let h = (ah * scale).round();
    let to_u32 = |v: f64| -> u32 {
        if v.is_finite() && v >= 0.0 && v <= f64::from(u32::MAX) {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            // reason: range checked above
            let n = v as u32;
            n
        } else {
            0
        }
    };
    [to_u32(w), to_u32(h)]
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on io errors
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn writes_new_block_when_file_is_empty() {
        // Arrange
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        // Act
        write_monitor_block(
            &path,
            &MonitorIdentifier::Name("DP-1".to_owned()),
            [597, 336],
        )
        .unwrap();

        // Assert
        let written = fs::read_to_string(&path).unwrap();
        assert!(written.contains("[[monitor]]"));
        assert!(written.contains("name = \"DP-1\""));
        assert!(written.contains("physical_mm = [597, 336]"));
    }

    #[test]
    fn updates_existing_block_when_name_matches() {
        // Arrange
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let initial = "# user comment\n\
                       [[monitor]]\n\
                       name = \"DP-1\"\n\
                       physical_mm = [100, 100]\n";
        fs::write(&path, initial).unwrap();

        // Act
        write_monitor_block(
            &path,
            &MonitorIdentifier::Name("DP-1".to_owned()),
            [597, 336],
        )
        .unwrap();

        // Assert
        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("# user comment"),
            "comment was dropped: {written}"
        );
        assert!(written.contains("physical_mm = [597, 336]"));
        // Only one block.
        assert_eq!(written.matches("[[monitor]]").count(), 1);
    }

    #[test]
    fn appends_new_block_when_no_match_found() {
        // Arrange
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let initial = "[[monitor]]\nname = \"DP-1\"\nphysical_mm = [100, 100]\n";
        fs::write(&path, initial).unwrap();

        // Act
        write_monitor_block(
            &path,
            &MonitorIdentifier::Name("HDMI-A-1".to_owned()),
            [527, 296],
        )
        .unwrap();

        // Assert
        let written = fs::read_to_string(&path).unwrap();
        assert_eq!(written.matches("[[monitor]]").count(), 2);
        assert!(written.contains("HDMI-A-1"));
    }

    #[test]
    fn diagonal_to_mm_27_inch_16_9_matches_known_panel_size() {
        // Arrange + Act
        let [w, h] = diagonal_to_mm(27.0, 16, 9);

        // Assert — within rounding of the canonical 597x336.
        assert!((595..=599).contains(&w), "width was {w}");
        assert!((334..=338).contains(&h), "height was {h}");
    }
}
