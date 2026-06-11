//! Resume state: the last-active profile and apply metadata, persisted so a
//! daemon restart (or a daemon-less GUI launch) lands back where the user
//! left off instead of a blank slate.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// File name under [`state_dir`].
pub const RESUME_FILE: &str = "resume-state.json";

/// Last-apply snapshot persisted across daemon restarts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumeState {
    pub active_profile: String,
    #[serde(default)]
    pub last_apply_backend: Option<String>,
    #[serde(default)]
    pub last_apply_unix_secs: Option<u64>,
}

#[derive(Debug, Error)]
pub enum ResumeError {
    #[error("resume-state I/O at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("resume-state at {path} is malformed: {source}")]
    Malformed {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

/// State-dir path: `$XDG_STATE_HOME/superpanels/` (or
/// `~/.local/state/superpanels/`). `None` when neither resolves.
pub fn state_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("XDG_STATE_HOME") {
        let p = PathBuf::from(dir);
        if !p.as_os_str().is_empty() {
            return Some(p.join("superpanels"));
        }
    }
    let home = std::env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("superpanels"),
    )
}

/// Resolved resume-file path; `None` when no state dir can be determined.
pub fn resume_path() -> Option<PathBuf> {
    state_dir().map(|d| d.join(RESUME_FILE))
}

/// Persist atomically (write-temp-then-rename, same scheme as the slideshow
/// state) so a crash mid-write can't leave a truncated file.
pub fn save(state: &ResumeState, path: &Path) -> Result<(), ResumeError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ResumeError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let json = serde_json::to_vec_pretty(state).map_err(|source| ResumeError::Malformed {
        path: path.to_path_buf(),
        source,
    })?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &json).map_err(|source| ResumeError::Io {
        path: tmp.clone(),
        source,
    })?;
    std::fs::rename(&tmp, path).map_err(|source| ResumeError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

/// `Ok(None)` when the file doesn't exist yet; `Err` on unreadable or
/// malformed content so callers can warn instead of silently dropping it.
pub fn load(path: &Path) -> Result<Option<ResumeState>, ResumeError> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ResumeError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|source| ResumeError::Malformed {
            path: path.to_path_buf(),
            source,
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample() -> ResumeState {
        ResumeState {
            active_profile: "Lofi".to_owned(),
            last_apply_backend: Some("kde".to_owned()),
            last_apply_unix_secs: Some(1_781_213_342),
        }
    }

    #[test]
    fn save_and_load_round_trips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("resume-state.json");
        save(&sample(), &path).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded, Some(sample()));
    }

    #[test]
    fn save_creates_missing_parent_dirs() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("resume-state.json");
        save(&sample(), &path).unwrap();
        assert!(load(&path).unwrap().is_some());
    }

    #[test]
    fn load_missing_file_returns_none() {
        let dir = tempdir().unwrap();
        let loaded = load(&dir.path().join("absent.json")).unwrap();
        assert_eq!(loaded, None);
    }

    #[test]
    fn load_malformed_file_returns_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("resume-state.json");
        std::fs::write(&path, b"{not json").unwrap();
        let err = load(&path).unwrap_err();
        assert!(matches!(err, ResumeError::Malformed { .. }));
    }

    #[test]
    fn load_tolerates_missing_optional_fields() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("resume-state.json");
        std::fs::write(&path, br#"{"active_profile": "Lofi"}"#).unwrap();
        let loaded = load(&path).unwrap().unwrap();
        assert_eq!(loaded.active_profile, "Lofi");
        assert_eq!(loaded.last_apply_backend, None);
    }
}
