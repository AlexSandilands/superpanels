//! SQLite-backed library index. Schema is migrated via
//! `PRAGMA user_version`; tag matching is case-insensitive (`COLLATE NOCASE`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use thiserror::Error;
use tracing::debug;

use super::LibraryEntry;

/// Library DB `user_version`. Bump when schema changes; add a new arm to
/// [`LibraryDb::migrate`].
pub const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("library db at {path}: {source}")]
    Sql {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },
    #[error("library db parent dir {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("library db {path}: schema version {found} is newer than this binary's {expected}")]
    SchemaTooNew {
        path: PathBuf,
        found: i64,
        expected: i64,
    },
}

/// Opens (or creates) the library DB at `path`, runs migrations to the latest
/// schema, and returns a handle.
pub struct LibraryDb {
    conn: Connection,
    path: PathBuf,
}

impl LibraryDb {
    pub fn open(path: &Path) -> Result<Self, DbError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| DbError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let conn = Connection::open(path).map_err(|source| DbError::Sql {
            path: path.to_path_buf(),
            source,
        })?;
        Self::pragmas(&conn).map_err(|source| DbError::Sql {
            path: path.to_path_buf(),
            source,
        })?;
        let mut db = Self {
            conn,
            path: path.to_path_buf(),
        };
        db.migrate()?;
        Ok(db)
    }

    /// In-memory DB for tests.
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory().map_err(|source| DbError::Sql {
            path: PathBuf::from(":memory:"),
            source,
        })?;
        Self::pragmas(&conn).map_err(|source| DbError::Sql {
            path: PathBuf::from(":memory:"),
            source,
        })?;
        let mut db = Self {
            conn,
            path: PathBuf::from(":memory:"),
        };
        db.migrate()?;
        Ok(db)
    }

    fn pragmas(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;\n\
             PRAGMA synchronous = NORMAL;\n\
             PRAGMA foreign_keys = ON;",
        )
    }

    fn sql_err(&self, source: rusqlite::Error) -> DbError {
        DbError::Sql {
            path: self.path.clone(),
            source,
        }
    }

    fn migrate(&mut self) -> Result<(), DbError> {
        let current: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(|e| self.sql_err(e))?;
        if current > SCHEMA_VERSION {
            return Err(DbError::SchemaTooNew {
                path: self.path.clone(),
                found: current,
                expected: SCHEMA_VERSION,
            });
        }
        let tx = self.conn.transaction().map_err(|e| DbError::Sql {
            path: self.path.clone(),
            source: e,
        })?;
        if current < 1 {
            migrate_v0_to_v1(&tx).map_err(|e| DbError::Sql {
                path: self.path.clone(),
                source: e,
            })?;
        }
        tx.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
            .map_err(|e| DbError::Sql {
                path: self.path.clone(),
                source: e,
            })?;
        tx.commit().map_err(|e| DbError::Sql {
            path: self.path.clone(),
            source: e,
        })?;
        debug!(version = SCHEMA_VERSION, "library db ready");
        Ok(())
    }

    /// Replace the on-disk index with the FS-derived `entries` while preserving
    /// per-entry metadata (tags, favourite, `last_shown`, `show_count`) for paths
    /// that already exist. Entries no longer present on disk are removed
    /// (cascading their tag links).
    pub fn replace_entries_preserving_metadata(
        &mut self,
        entries: &[LibraryEntry],
    ) -> Result<(), DbError> {
        let tx = self.conn.transaction().map_err(|e| DbError::Sql {
            path: self.path.clone(),
            source: e,
        })?;

        let known: Vec<String> = {
            let mut stmt = tx
                .prepare("SELECT path FROM entries")
                .map_err(|e| DbError::Sql {
                    path: self.path.clone(),
                    source: e,
                })?;
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(|e| DbError::Sql {
                    path: self.path.clone(),
                    source: e,
                })?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row.map_err(|e| DbError::Sql {
                    path: self.path.clone(),
                    source: e,
                })?);
            }
            out
        };

        let new_paths: std::collections::HashSet<String> = entries
            .iter()
            .map(|e| e.path.to_string_lossy().into_owned())
            .collect();

        for path in known {
            if !new_paths.contains(&path) {
                tx.execute("DELETE FROM entries WHERE path = ?1", params![path])
                    .map_err(|e| DbError::Sql {
                        path: self.path.clone(),
                        source: e,
                    })?;
            }
        }

        for entry in entries {
            upsert_entry_fs_columns(&tx, entry).map_err(|e| DbError::Sql {
                path: self.path.clone(),
                source: e,
            })?;
        }

        tx.commit().map_err(|e| DbError::Sql {
            path: self.path.clone(),
            source: e,
        })?;
        Ok(())
    }

    /// Insert an entry verbatim (FS columns plus metadata + tags). Used by the
    /// JSON→SQLite migration so existing tag data carries over. Idempotent:
    /// re-running with the same input yields the same DB state.
    pub fn insert_entry_full(&mut self, entry: &LibraryEntry) -> Result<(), DbError> {
        let tx = self.conn.transaction().map_err(|e| DbError::Sql {
            path: self.path.clone(),
            source: e,
        })?;
        upsert_entry_fs_columns(&tx, entry).map_err(|e| DbError::Sql {
            path: self.path.clone(),
            source: e,
        })?;
        let path_s = entry.path.to_string_lossy().into_owned();
        tx.execute(
            "UPDATE entries
                SET favourite = ?2,
                    last_shown_unix = ?3,
                    show_count = ?4
              WHERE path = ?1",
            params![
                path_s,
                i64::from(entry.favourite),
                entry.last_shown.and_then(system_time_to_unix),
                i64::from(entry.show_count),
            ],
        )
        .map_err(|e| DbError::Sql {
            path: self.path.clone(),
            source: e,
        })?;
        for tag in &entry.tags {
            set_tag_in_tx(&tx, &entry.path, tag, true).map_err(|e| DbError::Sql {
                path: self.path.clone(),
                source: e,
            })?;
        }
        tx.commit().map_err(|e| DbError::Sql {
            path: self.path.clone(),
            source: e,
        })?;
        Ok(())
    }

    /// Read every entry plus its tags. Used to hydrate the daemon's in-memory
    /// cache on startup.
    pub fn list_entries(&self) -> Result<Vec<LibraryEntry>, DbError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT path, width, height, file_size, modified_unix,
                        favourite, last_shown_unix, show_count
                   FROM entries",
            )
            .map_err(|e| self.sql_err(e))?;
        let rows = stmt
            .query_map([], row_to_entry)
            .map_err(|e| self.sql_err(e))?;
        let mut entries: Vec<LibraryEntry> = Vec::new();
        for row in rows {
            entries.push(row.map_err(|e| self.sql_err(e))?);
        }

        // Bulk-fetch tags so we don't N+1.
        let mut tags_by_path: HashMap<String, Vec<String>> = HashMap::new();
        let mut tag_stmt = self
            .conn
            .prepare(
                "SELECT et.entry_path, t.name
                   FROM entry_tags et
                   JOIN tags t ON t.id = et.tag_id",
            )
            .map_err(|e| self.sql_err(e))?;
        let tag_rows = tag_stmt
            .query_map([], |r| {
                let p: String = r.get(0)?;
                let n: String = r.get(1)?;
                Ok((p, n))
            })
            .map_err(|e| self.sql_err(e))?;
        for row in tag_rows {
            let (p, n) = row.map_err(|e| self.sql_err(e))?;
            tags_by_path.entry(p).or_default().push(n);
        }
        for entry in &mut entries {
            let key = entry.path.to_string_lossy().into_owned();
            if let Some(tags) = tags_by_path.remove(&key) {
                entry.tags = tags;
            }
        }
        Ok(entries)
    }

    /// Idempotent tag toggle. Tag names are normalised to lowercase before
    /// storage; `tag_name` is case-insensitive on lookup. The reserved name
    /// `"favourite"` toggles the boolean column instead.
    pub fn set_tag(&mut self, path: &Path, tag_name: &str, on: bool) -> Result<(), DbError> {
        let path_buf = self.path.clone();
        let to_err = |e: rusqlite::Error| DbError::Sql {
            path: path_buf.clone(),
            source: e,
        };
        let tx = self.conn.transaction().map_err(to_err)?;
        if tag_name.eq_ignore_ascii_case("favourite") {
            tx.execute(
                "UPDATE entries SET favourite = ?2 WHERE path = ?1",
                params![path.to_string_lossy(), i64::from(on)],
            )
            .map_err(to_err)?;
        } else {
            set_tag_in_tx(&tx, path, tag_name, on).map_err(to_err)?;
        }
        tx.commit().map_err(|e| DbError::Sql {
            path: self.path.clone(),
            source: e,
        })?;
        Ok(())
    }

    /// Delete an entry (and its tag links via cascade). Returns `true` when a
    /// row was removed, `false` when `path` wasn't in the index.
    pub fn delete_entry(&mut self, path: &Path) -> Result<bool, DbError> {
        let n = self
            .conn
            .execute(
                "DELETE FROM entries WHERE path = ?1",
                params![path.to_string_lossy()],
            )
            .map_err(|e| self.sql_err(e))?;
        Ok(n > 0)
    }

    /// `true` when the index contains zero entries — useful as the "should we
    /// run the JSON migration?" trigger.
    pub fn is_empty(&self) -> Result<bool, DbError> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))
            .map_err(|e| self.sql_err(e))?;
        Ok(count == 0)
    }

    /// Update `last_shown` + bump `show_count` for a path. No-op when the path
    /// isn't in the index.
    pub fn record_shown(&mut self, path: &Path, when: SystemTime) -> Result<(), DbError> {
        let unix = system_time_to_unix(when);
        self.conn
            .execute(
                "UPDATE entries
                    SET last_shown_unix = ?2,
                        show_count      = show_count + 1
                  WHERE path = ?1",
                params![path.to_string_lossy(), unix],
            )
            .map_err(|e| self.sql_err(e))?;
        Ok(())
    }
}

fn migrate_v0_to_v1(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS entries (
             path             TEXT PRIMARY KEY,
             width            INTEGER NOT NULL,
             height           INTEGER NOT NULL,
             file_size        INTEGER NOT NULL,
             modified_unix    INTEGER,
             favourite        INTEGER NOT NULL DEFAULT 0,
             last_shown_unix  INTEGER,
             show_count       INTEGER NOT NULL DEFAULT 0
         );

         CREATE TABLE IF NOT EXISTS tags (
             id    INTEGER PRIMARY KEY,
             name  TEXT NOT NULL UNIQUE COLLATE NOCASE
         );

         CREATE TABLE IF NOT EXISTS entry_tags (
             entry_path  TEXT NOT NULL,
             tag_id      INTEGER NOT NULL,
             PRIMARY KEY (entry_path, tag_id),
             FOREIGN KEY (entry_path) REFERENCES entries(path) ON DELETE CASCADE,
             FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
         );

         CREATE TABLE IF NOT EXISTS roots (
             path TEXT PRIMARY KEY
         );

         CREATE INDEX IF NOT EXISTS idx_entries_favourite ON entries(favourite);
         CREATE INDEX IF NOT EXISTS idx_entry_tags_path ON entry_tags(entry_path);
         CREATE INDEX IF NOT EXISTS idx_entry_tags_tag ON entry_tags(tag_id);",
    )
}

fn upsert_entry_fs_columns(tx: &Transaction<'_>, entry: &LibraryEntry) -> rusqlite::Result<()> {
    let path_s = entry.path.to_string_lossy().into_owned();
    let modified_unix = system_time_to_unix(entry.modified);
    let (w, h) = entry.resolution;
    tx.execute(
        "INSERT INTO entries (path, width, height, file_size, modified_unix)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(path) DO UPDATE SET
             width         = excluded.width,
             height        = excluded.height,
             file_size     = excluded.file_size,
             modified_unix = excluded.modified_unix",
        params![path_s, w, h, entry.file_size, modified_unix],
    )?;
    Ok(())
}

fn set_tag_in_tx(
    tx: &Transaction<'_>,
    path: &Path,
    tag_name: &str,
    on: bool,
) -> rusqlite::Result<()> {
    let normalised = tag_name.trim().to_ascii_lowercase();
    if normalised.is_empty() {
        return Ok(());
    }
    let path_s = path.to_string_lossy().into_owned();

    // Confirm entry exists. UPDATE/INSERT against a missing FK would be
    // silently ignored under the foreign-keys pragma, so explicit check first.
    let exists: bool = tx
        .query_row(
            "SELECT 1 FROM entries WHERE path = ?1",
            params![path_s],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if !exists {
        return Ok(());
    }

    if on {
        tx.execute(
            "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
            params![normalised],
        )?;
        let tag_id: i64 = tx.query_row(
            "SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE",
            params![normalised],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO entry_tags (entry_path, tag_id) VALUES (?1, ?2)",
            params![path_s, tag_id],
        )?;
    } else {
        tx.execute(
            "DELETE FROM entry_tags
              WHERE entry_path = ?1
                AND tag_id IN (SELECT id FROM tags WHERE name = ?2 COLLATE NOCASE)",
            params![path_s, normalised],
        )?;
    }
    Ok(())
}

fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<LibraryEntry> {
    let path: String = row.get(0)?;
    let width: u32 = row.get(1)?;
    let height: u32 = row.get(2)?;
    let file_size: u64 = row.get(3)?;
    let modified_unix: Option<i64> = row.get(4)?;
    let favourite: i64 = row.get(5)?;
    let last_shown_unix: Option<i64> = row.get(6)?;
    let show_count: i64 = row.get(7)?;

    #[allow(clippy::cast_precision_loss)] // reason: image dims fit f32 exactly
    let aspect_ratio = if height == 0 {
        0.0
    } else {
        (width as f32) / (height as f32)
    };

    Ok(LibraryEntry {
        path: PathBuf::from(path),
        resolution: (width, height),
        aspect_ratio,
        file_size,
        modified: modified_unix
            .and_then(unix_to_system_time)
            .unwrap_or(SystemTime::UNIX_EPOCH),
        tags: Vec::new(),
        favourite: favourite != 0,
        last_shown: last_shown_unix.and_then(unix_to_system_time),
        show_count: u32::try_from(show_count.max(0)).unwrap_or(0),
    })
}

fn system_time_to_unix(t: SystemTime) -> Option<i64> {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_secs()).ok())
}

fn unix_to_system_time(secs: i64) -> Option<SystemTime> {
    if secs < 0 {
        return None;
    }
    u64::try_from(secs)
        .ok()
        .map(|u| SystemTime::UNIX_EPOCH + Duration::from_secs(u))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // reason: tests fail loudly on harness errors
#[allow(clippy::panic)] // reason: assertion-style panics in tests
#[allow(clippy::cloned_ref_to_slice_refs)] // reason: tests reuse `e` after the call
mod tests {
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    use super::*;

    fn entry(path: &str, w: u32, h: u32) -> LibraryEntry {
        LibraryEntry {
            path: PathBuf::from(path),
            resolution: (w, h),
            #[allow(clippy::cast_precision_loss)]
            aspect_ratio: (w as f32) / (h as f32),
            file_size: 1024,
            modified: SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            tags: Vec::new(),
            favourite: false,
            last_shown: None,
            show_count: 0,
        }
    }

    #[test]
    fn migrate_creates_schema_at_latest_version() {
        let db = LibraryDb::open_in_memory().unwrap();
        let v: i64 = db
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
    }

    #[test]
    fn replace_entries_preserves_tags_and_favourite_for_existing_paths() {
        let mut db = LibraryDb::open_in_memory().unwrap();
        let e = entry("/walls/a.png", 16, 9);
        db.replace_entries_preserving_metadata(&[e.clone()])
            .unwrap();
        db.set_tag(&e.path, "blue", true).unwrap();
        db.set_tag(&e.path, "favourite", true).unwrap();

        let e2 = LibraryEntry {
            file_size: 2048,
            ..e.clone()
        };
        db.replace_entries_preserving_metadata(&[e2.clone()])
            .unwrap();

        let loaded = db.list_entries().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].file_size, 2048);
        assert!(loaded[0].favourite);
        assert_eq!(loaded[0].tags, vec!["blue".to_owned()]);
    }

    #[test]
    fn replace_entries_removes_paths_no_longer_present() {
        let mut db = LibraryDb::open_in_memory().unwrap();
        let a = entry("/walls/a.png", 16, 9);
        let b = entry("/walls/b.png", 16, 9);
        db.replace_entries_preserving_metadata(&[a, b.clone()])
            .unwrap();
        assert_eq!(db.list_entries().unwrap().len(), 2);

        db.replace_entries_preserving_metadata(&[b]).unwrap();
        let loaded = db.list_entries().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].path.to_string_lossy(), "/walls/b.png".to_owned());
    }

    #[test]
    fn set_tag_is_case_insensitive_and_idempotent() {
        let mut db = LibraryDb::open_in_memory().unwrap();
        let e = entry("/walls/x.png", 1, 1);
        db.replace_entries_preserving_metadata(&[e.clone()])
            .unwrap();

        db.set_tag(&e.path, "Nature", true).unwrap();
        db.set_tag(&e.path, "nature", true).unwrap(); // idempotent
        db.set_tag(&e.path, "NATURE", false).unwrap();
        db.set_tag(&e.path, "nature", false).unwrap(); // idempotent off

        let loaded = db.list_entries().unwrap();
        assert!(loaded[0].tags.is_empty());

        db.set_tag(&e.path, "Mountain", true).unwrap();
        let loaded = db.list_entries().unwrap();
        assert_eq!(loaded[0].tags, vec!["mountain".to_owned()]);
    }

    #[test]
    fn favourite_toggles_via_reserved_tag_name() {
        let mut db = LibraryDb::open_in_memory().unwrap();
        let e = entry("/walls/x.png", 1, 1);
        db.replace_entries_preserving_metadata(&[e.clone()])
            .unwrap();

        db.set_tag(&e.path, "favourite", true).unwrap();
        assert!(db.list_entries().unwrap()[0].favourite);
        db.set_tag(&e.path, "FAVOURITE", false).unwrap();
        assert!(!db.list_entries().unwrap()[0].favourite);
    }

    #[test]
    fn delete_entry_cascades_tag_links() {
        let mut db = LibraryDb::open_in_memory().unwrap();
        let e = entry("/walls/x.png", 1, 1);
        db.replace_entries_preserving_metadata(&[e.clone()])
            .unwrap();
        db.set_tag(&e.path, "blue", true).unwrap();
        assert!(db.delete_entry(&e.path).unwrap());

        let n: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM entry_tags", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn schema_too_new_is_rejected() {
        let db_path = tempfile::Builder::new()
            .suffix(".db")
            .tempfile()
            .unwrap()
            .into_temp_path();
        let path: &Path = db_path.as_ref();
        {
            let _ = LibraryDb::open(path).unwrap();
        }
        let raw = Connection::open(path).unwrap();
        raw.execute_batch("PRAGMA user_version = 999").unwrap();
        drop(raw);

        match LibraryDb::open(path) {
            Err(DbError::SchemaTooNew { .. }) => {}
            Err(other) => panic!("expected SchemaTooNew, got {other:?}"),
            Ok(_) => panic!("expected SchemaTooNew, got Ok"),
        }
    }

    #[test]
    fn record_shown_updates_last_shown_and_increments_count() {
        let mut db = LibraryDb::open_in_memory().unwrap();
        let e = entry("/walls/x.png", 1, 1);
        db.replace_entries_preserving_metadata(&[e.clone()])
            .unwrap();

        let when = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_500);
        db.record_shown(&e.path, when).unwrap();
        db.record_shown(&e.path, when).unwrap();

        let loaded = db.list_entries().unwrap();
        assert_eq!(loaded[0].show_count, 2);
        assert_eq!(loaded[0].last_shown, Some(when));
    }

    #[test]
    fn insert_entry_full_round_trips_metadata() {
        let mut db = LibraryDb::open_in_memory().unwrap();
        let mut e = entry("/walls/x.png", 16, 9);
        e.favourite = true;
        e.last_shown = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_001_234));
        e.show_count = 3;
        e.tags = vec!["nature".to_owned(), "favourite-pano".to_owned()];

        db.insert_entry_full(&e).unwrap();
        let loaded = db.list_entries().unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(loaded[0].favourite);
        assert_eq!(loaded[0].show_count, 3);
        let mut tags = loaded[0].tags.clone();
        tags.sort();
        assert_eq!(tags, vec!["favourite-pano".to_owned(), "nature".to_owned()]);
    }
}
