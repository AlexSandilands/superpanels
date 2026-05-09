//! Add or update a `[[monitor]]` block on disk while preserving comments.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;
use toml_edit::{Array, ArrayOfTables, DocumentMut, Item, Table, value};

/// How the caller identified the target monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorIdentifier {
    StableId(String),
    Name(String),
}

#[derive(Debug, Error)]
pub enum MonitorEditError {
    #[error("could not read config at {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not write config at {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not parse config at {path}: {message}")]
    Parse { path: PathBuf, message: String },
}

/// Set `physical_mm` on the matching `[[monitor]]` block (or append a new one).
/// Comments and other fields on matched blocks are preserved.
pub fn write_monitor_block(
    path: &Path,
    identifier: &MonitorIdentifier,
    physical_mm: [f64; 2],
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
        // Existing `monitor = ...` was scalar; replace with an array of tables.
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

fn apply_block_fields(block: &mut Table, identifier: &MonitorIdentifier, physical_mm: [f64; 2]) {
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
    arr.push(physical_mm[0]);
    arr.push(physical_mm[1]);
    block.insert("physical_mm", value(arr));
}

/// Physical mm from a diagonal length and an aspect ratio (e.g. 27.0, 16:9).
/// Result is rounded to 1dp so it matches what the GUI's mm editor displays.
#[must_use]
pub fn diagonal_to_mm(diagonal_inches: f64, aspect_w: u32, aspect_h: u32) -> [f64; 2] {
    let aw = f64::from(aspect_w);
    let ah = f64::from(aspect_h);
    let diag_mm = diagonal_inches * 25.4;
    let scale = diag_mm / (aw * aw + ah * ah).sqrt();
    let round_1dp = |v: f64| (v * 10.0).round() / 10.0;
    [round_1dp(aw * scale), round_1dp(ah * scale)]
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
            [597.0, 336.0],
        )
        .unwrap();

        // Assert
        let written = fs::read_to_string(&path).unwrap();
        assert!(written.contains("[[monitor]]"));
        assert!(written.contains("name = \"DP-1\""));
        assert!(written.contains("physical_mm = [597.0, 336.0]"));
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
            [597.0, 336.0],
        )
        .unwrap();

        // Assert
        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("# user comment"),
            "comment was dropped: {written}"
        );
        assert!(written.contains("physical_mm = [597.0, 336.0]"));
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
            [527.0, 296.0],
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

        // Assert
        assert!((595.0..=599.0).contains(&w), "width was {w}");
        assert!((334.0..=338.0).contains(&h), "height was {h}");
    }
}
