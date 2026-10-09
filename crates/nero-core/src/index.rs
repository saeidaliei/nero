use std::{collections::{HashMap, HashSet}, fs, path::Path, time::Duration};

use rusqlite::{params, Connection};

use crate::{document, NoteSummary, Result, SearchResult, Workspace};

const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS notes (
    path TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    modified_ns INTEGER NOT NULL,
    size INTEGER NOT NULL
);
CREATE VIRTUAL TABLE IF NOT EXISTS note_search USING fts5(
    path UNINDEXED,
    title,
    body
);
CREATE TABLE IF NOT EXISTS links (
    source_path TEXT NOT NULL,
    target TEXT NOT NULL,
    PRIMARY KEY (source_path, target)
);
"#;

pub struct Index {
    connection: Connection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexStats {
    pub indexed: usize,
    pub removed: usize,
}

impl Index {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version != SCHEMA_VERSION {
            connection.execute_batch("DROP TABLE IF EXISTS note_search; DROP TABLE IF EXISTS links; DROP TABLE IF EXISTS notes;")?;
        }
        connection.execute_batch(SCHEMA)?;
        connection.execute_batch(&format!("PRAGMA user_version={SCHEMA_VERSION};"))?;
        Ok(Self { connection })
    }

    pub fn refresh(&mut self, workspace: &Workspace) -> Result<IndexStats> {
        let existing = self.existing_metadata()?;
        let notes = workspace.list_notes()?;
        let current_paths: HashSet<String> = notes
            .iter()
            .map(|note| note.path.to_string_lossy().replace('\\', "/"))
            .collect();

        let transaction = self.connection.transaction()?;
        let mut indexed = 0usize;

        for note in &notes {
            let path_key = normalize_path(&note.path);
            let absolute = workspace.root().join(&note.path);
            let metadata = fs::metadata(&absolute)?;
            let modified_ns = modified_ns(&metadata);
            let size = metadata.len() as i64;
            let unchanged = existing
                .get(&path_key)
                .is_some_and(|(old_modified, old_size, old_title)| {
                    *old_modified == modified_ns && *old_size == size && note.title.as_str() == old_title.as_str()
                });

            if unchanged {
                continue;
            }

            let body = fs::read_to_string(&absolute)?;
            let document = document::parse(&body);

            transaction.execute("DELETE FROM note_search WHERE path = ?1", params![path_key])?;
            transaction.execute(
                "INSERT INTO note_search(path, title, body) VALUES (?1, ?2, ?3)",
                params![path_key, &document.title, &body],
            )?;
            transaction.execute("DELETE FROM links WHERE source_path = ?1", params![normalize_path(&note.path)])?;
            for target in &document.wiki_links {
                transaction.execute(
                    "INSERT OR IGNORE INTO links(source_path, target) VALUES (?1, ?2)",
                    params![normalize_path(&note.path), &target.target],
                )?;
            }
            transaction.execute(
                "INSERT INTO notes(path, title, modified_ns, size) VALUES (?1, ?2, ?3, ?4)\
                 ON CONFLICT(path) DO UPDATE SET title=excluded.title, modified_ns=excluded.modified_ns, size=excluded.size",
                params![normalize_path(&note.path), &document.title, modified_ns, size],
            )?;
            indexed += 1;
        }

        let mut removed = 0usize;
        let mut stale = Vec::new();
        {
            let mut statement = transaction.prepare("SELECT path FROM notes")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            for row in rows {
                let path = row?;
                if !current_paths.contains(&path) {
                    stale.push(path);
                }
            }
        }
        for path in stale {
            transaction.execute("DELETE FROM note_search WHERE path = ?1", params![&path])?;
            transaction.execute("DELETE FROM links WHERE source_path = ?1", params![&path])?;
            transaction.execute("DELETE FROM notes WHERE path = ?1", params![&path])?;
            removed += 1;
        }

        transaction.commit()?;
        Ok(IndexStats { indexed, removed })
    }

    pub fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let normalized = query.trim();
        if normalized.is_empty() {
            return Ok(Vec::new());
        }
        let fts_query = build_fts_query(normalized);
        let mut statement = self.connection.prepare(
            "SELECT path, title, body, bm25(note_search) AS rank\n             FROM note_search\n             WHERE note_search MATCH ?1\n             ORDER BY rank, path\n             LIMIT 100",
        )?;
        let rows = statement.query_map(params![fts_query], |row| {
            let path: String = row.get(0)?;
            let title: String = row.get(1)?;
            let body: String = row.get(2)?;
            let score: f64 = row.get(3)?;
            Ok((path, title, body, score))
        })?;

        let mut results = Vec::new();
        for row in rows {
            let (path, title, body, score) = row?;
            results.push(SearchResult {
                note: NoteSummary { path: path.into(), title },
                score,
                preview: crate::preview_for_query(&body, normalized),
            });
        }
        Ok(results)
    }

    pub fn link_sources_for_target(
        &self,
        target: &std::path::Path,
        notes: &[NoteSummary],
    ) -> Result<Vec<std::path::PathBuf>> {
        let mut statement = self.connection.prepare("SELECT source_path, target FROM links ORDER BY source_path")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (source, raw_target) = row?;
            let source_path = std::path::PathBuf::from(&source);
            if crate::resolve_link_for_index(&source_path, &raw_target, notes)
                .is_some_and(|resolved| normalize_path(&resolved.path) == normalize_path(target))
            {
                result.push(source_path);
            }
        }
        result.sort();
        result.dedup();
        Ok(result)
    }

    fn existing_metadata(&self) -> Result<HashMap<String, (i64, i64, String)>> {
        let mut statement = self.connection.prepare("SELECT path, modified_ns, size, title FROM notes")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        let mut result = HashMap::new();
        for row in rows {
            let (path, modified_ns, size, title) = row?;
            result.insert(path, (modified_ns, size, title));
        }
        Ok(result)
    }
}

fn modified_ns(metadata: &fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn build_fts_query(input: &str) -> String {
    input
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}
