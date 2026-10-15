//! Persistent integer identifiers for Markdown notes in one workspace.
//!
//! This registry is intentionally separate from the rebuildable search index. IDs
//! are allocated once per relative note path and are never recycled after deletion.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use rusqlite::{Connection, OptionalExtension, params};

use crate::Result;

pub(crate) struct NoteIdRegistry {
    connection: Connection,
}

impl NoteIdRegistry {
    pub(crate) fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS note_ids (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE
            );",
        )?;
        Ok(Self { connection })
    }

    /// Assign IDs to paths that don't have one and return IDs for all supplied paths.
    /// SQLite's transaction and unique constraint make simultaneous CLI/GUI access safe.
    pub(crate) fn ensure_ids(&mut self, paths: &[PathBuf]) -> Result<HashMap<String, u64>> {
        let transaction = self.connection.transaction()?;
        for path in paths {
            transaction.execute(
                "INSERT OR IGNORE INTO note_ids(path) VALUES (?1)",
                params![normalize_path(path)],
            )?;
        }

        let mut ids = HashMap::with_capacity(paths.len());
        for path in paths {
            let key = normalize_path(path);
            let id: i64 = transaction.query_row(
                "SELECT id FROM note_ids WHERE path = ?1",
                params![key],
                |row| row.get(0),
            )?;
            // IDs are allocated by SQLite's positive AUTOINCREMENT integer primary key.
            let id = u64::try_from(id).map_err(|_| {
                crate::NeroError::Message("note ID must be a positive integer".into())
            })?;
            ids.insert(key, id);
        }
        transaction.commit()?;
        Ok(ids)
    }

    /// Tombstone an ID after `nero delete` so recreating the same filename gets a new ID.
    pub(crate) fn retire_path(&mut self, path: &Path) -> Result<()> {
        let key = normalize_path(path);
        let id: Option<i64> = self
            .connection
            .query_row(
                "SELECT id FROM note_ids WHERE path = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(id) = id {
            self.connection.execute(
                "UPDATE note_ids SET path = ?1 WHERE id = ?2",
                params![format!("__nero_deleted_note_id_{id}__"), id],
            )?;
        }
        Ok(())
    }

    pub(crate) fn path_for_id(&self, id: u64) -> Result<Option<PathBuf>> {
        let Ok(id) = i64::try_from(id) else {
            return Ok(None);
        };
        let path: Option<String> = self
            .connection
            .query_row(
                "SELECT path FROM note_ids WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(path.map(PathBuf::from))
    }
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
