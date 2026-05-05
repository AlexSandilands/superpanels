//! One-shot migration of the legacy `library-index.json` (Phase 2) into the
//! `SQLite` library DB (Phase 4b, `SPEC §14.5`). On a successful migration the
//! original JSON is renamed to `library-index.json.v1.bak` so the user has a
//! recoverable copy.

use std::path::Path;

use tracing::{info, warn};

use super::db::{DbError, LibraryDb};
use super::{LibraryError, load_index};

/// Run once at daemon startup. If `json_path` doesn't exist, the function is a
/// no-op. If it does, every entry in the JSON is upserted into `db` and the
/// JSON file is renamed to `<json_path>.v1.bak`. Idempotent: re-running with
/// the backup already in place leaves the DB unchanged.
pub fn migrate_json_to_sqlite(
    json_path: &Path,
    db: &mut LibraryDb,
) -> Result<bool, MigrationError> {
    if !json_path.exists() {
        return Ok(false);
    }
    let entries = load_index(json_path)?;
    info!(
        path = %json_path.display(),
        count = entries.len(),
        "migrating library-index.json into SQLite"
    );

    for entry in &entries {
        db.insert_entry_full(entry)?;
    }

    let backup = json_path.with_extension("json.v1.bak");
    if let Err(e) = std::fs::rename(json_path, &backup) {
        warn!(
            error = %e,
            path = %json_path.display(),
            "migrated entries into SQLite but could not rename JSON to backup"
        );
    } else {
        info!(backup = %backup.display(), "JSON index migrated; backup left in place");
    }
    Ok(true)
}

#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("library JSON: {0}")]
    Library(#[from] LibraryError),
    #[error("library DB: {0}")]
    Db(#[from] DbError),
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
mod tests {
    use std::path::PathBuf;
    use std::time::SystemTime;

    use tempfile::tempdir;

    use super::super::LibraryEntry;
    use super::*;
    use crate::library::persist_index;

    fn entry(path: &str) -> LibraryEntry {
        LibraryEntry {
            path: PathBuf::from(path),
            resolution: (16, 9),
            aspect_ratio: 16.0 / 9.0,
            file_size: 1024,
            modified: SystemTime::UNIX_EPOCH,
            tags: vec!["nature".to_owned()],
            favourite: true,
            last_shown: None,
            show_count: 5,
        }
    }

    #[test]
    fn migration_no_op_when_json_missing() {
        let dir = tempdir().unwrap();
        let mut db = LibraryDb::open_in_memory().unwrap();
        let did = migrate_json_to_sqlite(&dir.path().join("nope.json"), &mut db).unwrap();
        assert!(!did);
        assert_eq!(db.list_entries().unwrap().len(), 0);
    }

    #[test]
    fn migration_imports_entries_and_renames_json() {
        let dir = tempdir().unwrap();
        let json_path = dir.path().join("library-index.json");
        let entries = vec![entry("/walls/a.png"), entry("/walls/b.png")];
        persist_index(&entries, &json_path).unwrap();

        let mut db = LibraryDb::open_in_memory().unwrap();
        let did = migrate_json_to_sqlite(&json_path, &mut db).unwrap();
        assert!(did);

        let loaded = db.list_entries().unwrap();
        assert_eq!(loaded.len(), 2);
        assert!(loaded.iter().all(|e| e.favourite));
        assert!(loaded.iter().all(|e| e.tags == vec!["nature".to_owned()]));

        assert!(!json_path.exists());
        assert!(dir.path().join("library-index.json.v1.bak").exists());
    }

    #[test]
    fn migration_is_idempotent_when_re_run_against_same_db() {
        let dir = tempdir().unwrap();
        let json_path = dir.path().join("library-index.json");
        persist_index(&[entry("/walls/a.png")], &json_path).unwrap();

        let mut db = LibraryDb::open_in_memory().unwrap();
        migrate_json_to_sqlite(&json_path, &mut db).unwrap();
        // JSON is now backed up; second call is a no-op since the source is gone.
        let did = migrate_json_to_sqlite(&json_path, &mut db).unwrap();
        assert!(!did);
        assert_eq!(db.list_entries().unwrap().len(), 1);
    }
}
