//! Cross-session full-text search via `SQLite` `FTS5`.
//!
//! Maintains a sidecar `SQLite` database alongside the `JSONL` session files.
//! Each message is indexed by session ID, role, and text content. The `FTS5`
//! virtual table enables fast keyword search across all past conversations.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};

/// A search result returned by the session index.
#[derive(Debug, Clone)]
pub struct SessionSearchResult {
    pub session_id: String,
    pub role: String,
    pub content: String,
    /// FTS5 rank score (lower is more relevant).
    pub rank: f64,
    pub timestamp: i64,
}

/// SQLite-backed full-text search index for session messages.
#[derive(Debug)]
pub struct SessionIndex {
    conn: Connection,
    db_path: PathBuf,
}

impl SessionIndex {
    /// Open or create the session index database at the given path.
    /// Creates the schema (including `FTS5` virtual table) if it doesn't exist.
    pub fn open(db_path: &Path) -> Result<Self, String> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create index directory: {e}"))?;
        }
        let conn = Connection::open(db_path)
            .map_err(|e| format!("failed to open session index: {e}"))?;

        // Enable WAL mode for concurrent readers + single writer.
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| format!("failed to set WAL mode: {e}"))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp INTEGER NOT NULL DEFAULT 0
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
                content, content='messages', content_rowid='id'
            );
            CREATE TRIGGER IF NOT EXISTS messages_ai AFTER INSERT ON messages BEGIN
                INSERT INTO messages_fts(rowid, content) VALUES (new.id, new.content);
            END;
            CREATE TRIGGER IF NOT EXISTS messages_ad AFTER DELETE ON messages BEGIN
                INSERT INTO messages_fts(messages_fts, rowid, content) VALUES('delete', old.id, old.content);
            END;",
        )
        .map_err(|e| format!("failed to create session index schema: {e}"))?;

        Ok(Self {
            conn,
            db_path: db_path.to_path_buf(),
        })
    }

    /// Index a single message. Duplicate insertions (same `session_id` + content
    /// + timestamp) are allowed — `FTS5` handles this gracefully.
    pub fn index_message(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
        timestamp: i64,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO messages (session_id, role, content, timestamp) VALUES (?1, ?2, ?3, ?4)",
                params![session_id, role, content, timestamp],
            )
            .map_err(|e| format!("failed to index message: {e}"))?;
        Ok(())
    }

    /// Search across all indexed sessions using `FTS5` match syntax.
    /// Returns results ordered by relevance (best match first).
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SessionSearchResult>, String> {
        let limit_i64 = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut stmt = self
            .conn
            .prepare(
                "SELECT m.session_id, m.role, m.content, m.timestamp, rank
                 FROM messages_fts
                 JOIN messages m ON messages_fts.rowid = m.id
                 WHERE messages_fts MATCH ?1
                 ORDER BY rank
                 LIMIT ?2",
            )
            .map_err(|e| format!("failed to prepare search query: {e}"))?;

        let results = stmt
            .query_map(params![query, limit_i64], |row| {
                Ok(SessionSearchResult {
                    session_id: row.get(0)?,
                    role: row.get(1)?,
                    content: row.get(2)?,
                    timestamp: row.get(3)?,
                    rank: row.get(4)?,
                })
            })
            .map_err(|e| format!("search query failed: {e}"))?
            .filter_map(Result::ok)
            .collect();

        Ok(results)
    }

    /// Number of indexed messages.
    #[must_use]
    pub fn message_count(&self) -> usize {
        self.conn
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get::<_, i64>(0))
            .map(|count| usize::try_from(count.max(0)).unwrap_or(0))
            .unwrap_or(0)
    }

    /// Path to the database file.
    #[must_use]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("eidolon-session-index-test-{unique}.db"))
    }

    #[test]
    fn indexes_and_searches_messages() {
        let db_path = temp_db();
        let index = SessionIndex::open(&db_path).expect("should open");

        index
            .index_message("session-1", "user", "how does permission enforcement work", 1000)
            .expect("should index");
        index
            .index_message("session-1", "assistant", "permission enforcement uses PermissionPolicy", 1001)
            .expect("should index");
        index
            .index_message("session-2", "user", "explain the indexing crate", 2000)
            .expect("should index");

        assert_eq!(index.message_count(), 3);

        let results = index.search("permission", 10).expect("should search");
        assert_eq!(results.len(), 2);
        assert!(results[0].content.contains("permission"));

        let results = index.search("indexing", 10).expect("should search");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].session_id, "session-2");

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn empty_search_returns_no_results() {
        let db_path = temp_db();
        let index = SessionIndex::open(&db_path).expect("should open");

        let results = index.search("nonexistent", 10).expect("should search");
        assert!(results.is_empty());

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn reopening_database_preserves_data() {
        let db_path = temp_db();

        {
            let index = SessionIndex::open(&db_path).expect("should open");
            index
                .index_message("s1", "user", "persistent data test", 100)
                .expect("should index");
        }

        let index = SessionIndex::open(&db_path).expect("should reopen");
        assert_eq!(index.message_count(), 1);

        let results = index.search("persistent", 10).expect("should search");
        assert_eq!(results.len(), 1);

        let _ = std::fs::remove_file(&db_path);
    }
}
