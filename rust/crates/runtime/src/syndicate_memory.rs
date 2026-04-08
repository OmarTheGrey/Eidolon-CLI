//! Session-scoped persistent memory for Syndicate mode.
//!
//! Provides a thread-safe key-value store plus an append-only observation log
//! that syndicate agents share during a run. Backed by a JSONL file for
//! persistence across agent restarts within the same syndicate session.

use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[allow(clippy::cast_possible_truncation)] // epoch millis fits in u64 until ~year 584_942_417
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// A single key-value entry in syndicate shared memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub key: String,
    pub value: String,
    pub written_by: String,
    pub created_at: u64,
    pub updated_at: u64,
}

/// An append-only log entry for agent observations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryLogEntry {
    pub agent_id: String,
    pub operation: String,
    pub key: Option<String>,
    pub detail: String,
    pub timestamp: u64,
}

/// On-disk record format — each JSONL line is one of these.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum MemoryRecord {
    Entry(MemoryEntry),
    Log(MemoryLogEntry),
}

/// Thread-safe shared memory for a syndicate session.
#[derive(Debug, Clone)]
pub struct SyndicateMemory {
    inner: Arc<Mutex<MemoryInner>>,
}

#[derive(Debug)]
struct MemoryInner {
    entries: BTreeMap<String, MemoryEntry>,
    log: Vec<MemoryLogEntry>,
    path: Option<PathBuf>,
}

impl SyndicateMemory {
    /// Create a new in-memory store (no persistence).
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(MemoryInner {
                entries: BTreeMap::new(),
                log: Vec::new(),
                path: None,
            })),
        }
    }

    /// Create a new store backed by a JSONL file. Loads existing records if the
    /// file already exists.
    pub fn with_path(path: &Path) -> Result<Self, String> {
        let mut entries = BTreeMap::new();
        let mut log = Vec::new();

        if path.exists() {
            let file =
                std::fs::File::open(path).map_err(|e| format!("open memory file: {e}"))?;
            let reader = std::io::BufReader::new(file);
            for (idx, line) in reader.lines().enumerate() {
                let line = line.map_err(|e| format!("read memory line: {e}"))?;
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                match serde_json::from_str::<MemoryRecord>(trimmed) {
                    Ok(MemoryRecord::Entry(entry)) => {
                        entries.insert(entry.key.clone(), entry);
                    }
                    Ok(MemoryRecord::Log(entry)) => {
                        log.push(entry);
                    }
                    Err(e) => {
                        // Skip malformed lines rather than failing the whole load.
                        eprintln!(
                            "syndicate memory: skipping malformed record at line {}: {}; raw={}",
                            idx + 1,
                            e,
                            trimmed
                        );
                    }
                }
            }
        }

        Ok(Self {
            inner: Arc::new(Mutex::new(MemoryInner {
                entries,
                log,
                path: Some(path.to_path_buf()),
            })),
        })
    }

    /// Write or update a key-value pair. Appends to the backing file if
    /// persistence is enabled.
    pub fn write(&self, key: &str, value: &str, agent_id: &str) -> Result<MemoryEntry, String> {
        let mut inner = self.inner.lock().expect("syndicate memory lock poisoned");
        let ts = now_ms();
        let new_entry = MemoryEntry {
            key: key.to_owned(),
            value: value.to_owned(),
            written_by: agent_id.to_owned(),
            created_at: inner
                .entries
                .get(key)
                .map_or(ts, |existing| existing.created_at),
            updated_at: ts,
        };
        // Persist first — only mutate in-memory state on success
        Self::append_record(inner.path.as_ref(), &MemoryRecord::Entry(new_entry.clone()))?;
        inner.entries.insert(key.to_owned(), new_entry.clone());
        Ok(new_entry)
    }

    /// Read a single key. Returns `None` if the key doesn't exist.
    #[must_use]
    pub fn read(&self, key: &str) -> Option<MemoryEntry> {
        let inner = self.inner.lock().expect("syndicate memory lock poisoned");
        inner.entries.get(key).cloned()
    }

    /// Return all entries sorted by key.
    #[must_use]
    pub fn read_all(&self) -> Vec<MemoryEntry> {
        let inner = self.inner.lock().expect("syndicate memory lock poisoned");
        inner.entries.values().cloned().collect()
    }

    /// Append a structured observation to the shared log.
    pub fn append_log(
        &self,
        agent_id: &str,
        operation: &str,
        key: Option<&str>,
        detail: &str,
    ) -> Result<MemoryLogEntry, String> {
        let mut inner = self.inner.lock().expect("syndicate memory lock poisoned");
        let entry = MemoryLogEntry {
            agent_id: agent_id.to_owned(),
            operation: operation.to_owned(),
            key: key.map(str::to_owned),
            detail: detail.to_owned(),
            timestamp: now_ms(),
        };
        // Persist first — only mutate in-memory state on success
        Self::append_record(inner.path.as_ref(), &MemoryRecord::Log(entry.clone()))?;
        inner.log.push(entry.clone());
        Ok(entry)
    }

    /// Search entries whose key or value contains the query (case-insensitive).
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<MemoryEntry> {
        let inner = self.inner.lock().expect("syndicate memory lock poisoned");
        let lower = query.to_lowercase();
        inner
            .entries
            .values()
            .filter(|e| {
                e.key.to_lowercase().contains(&lower) || e.value.to_lowercase().contains(&lower)
            })
            .cloned()
            .collect()
    }

    /// Return the append-only observation log.
    #[must_use]
    pub fn log_entries(&self) -> Vec<MemoryLogEntry> {
        let inner = self.inner.lock().expect("syndicate memory lock poisoned");
        inner.log.clone()
    }

    /// Number of key-value entries currently stored.
    ///
    /// This does not include append-only log records.
    #[must_use]
    pub fn len(&self) -> usize {
        let inner = self.inner.lock().expect("syndicate memory lock poisoned");
        inner.entries.len()
    }

    /// Whether there are zero key-value entries.
    ///
    /// This follows Rust collection conventions and only checks the entry map.
    /// A non-empty log does not affect this result.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        let inner = self.inner.lock().expect("syndicate memory lock poisoned");
        inner.entries.is_empty()
    }

    /// Number of append-only log records.
    ///
    /// This is independent of [`Self::len`], which only counts key-value entries.
    #[must_use]
    pub fn log_len(&self) -> usize {
        let inner = self.inner.lock().expect("syndicate memory lock poisoned");
        inner.log.len()
    }

    // ── internal ──

    fn append_record(path: Option<&PathBuf>, record: &MemoryRecord) -> Result<(), String> {
        if let Some(path) = path {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("create memory directory: {e}"))?;
            }
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| format!("open memory file for append: {e}"))?;
            let line =
                serde_json::to_string(record).map_err(|e| format!("serialize record: {e}"))?;
            writeln!(file, "{line}").map_err(|e| format!("write memory record: {e}"))?;
        }
        Ok(())
    }
}

impl Default for SyndicateMemory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn write_read_round_trip() {
        let mem = SyndicateMemory::new();
        mem.write("goal", "implement feature X", "agent-1").unwrap();
        let entry = mem.read("goal").unwrap();
        assert_eq!(entry.value, "implement feature X");
        assert_eq!(entry.written_by, "agent-1");
    }

    #[test]
    fn write_overwrites_existing() {
        let mem = SyndicateMemory::new();
        mem.write("status", "running", "a1").unwrap();
        mem.write("status", "done", "a2").unwrap();
        let entry = mem.read("status").unwrap();
        assert_eq!(entry.value, "done");
        assert_eq!(entry.written_by, "a2");
    }

    #[test]
    fn read_nonexistent_returns_none() {
        let mem = SyndicateMemory::new();
        assert!(mem.read("nope").is_none());
    }

    #[test]
    fn read_all_returns_sorted() {
        let mem = SyndicateMemory::new();
        mem.write("z_key", "z", "a").unwrap();
        mem.write("a_key", "a", "a").unwrap();
        let all = mem.read_all();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].key, "a_key");
        assert_eq!(all[1].key, "z_key");
    }

    #[test]
    fn search_case_insensitive() {
        let mem = SyndicateMemory::new();
        mem.write("Plan", "Build the FEATURE", "a").unwrap();
        mem.write("other", "unrelated", "a").unwrap();
        let results = mem.search("feature");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "Plan");
    }

    #[test]
    fn append_log_and_retrieve() {
        let mem = SyndicateMemory::new();
        mem.append_log("agent-1", "observe", Some("file.rs"), "found issue")
            .unwrap();
        mem.append_log("agent-2", "decide", None, "will fix it")
            .unwrap();
        let logs = mem.log_entries();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].agent_id, "agent-1");
        assert_eq!(logs[1].operation, "decide");
    }

    #[test]
    fn persistence_round_trip() {
        let dir = std::env::temp_dir()
            .join(format!("eidolon_syndicate_mem_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("memory.jsonl");

        // Write data via first instance
        {
            let mem = SyndicateMemory::with_path(&path).unwrap();
            mem.write("key1", "val1", "agent-a").unwrap();
            mem.append_log("agent-a", "init", None, "started").unwrap();
            mem.write("key2", "val2", "agent-b").unwrap();
        }

        // Load from second instance
        {
            let mem = SyndicateMemory::with_path(&path).unwrap();
            assert_eq!(mem.len(), 2);
            assert_eq!(mem.read("key1").unwrap().value, "val1");
            assert_eq!(mem.read("key2").unwrap().value, "val2");
            assert_eq!(mem.log_entries().len(), 1);
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn len_and_is_empty() {
        let mem = SyndicateMemory::new();
        assert!(mem.is_empty());
        assert_eq!(mem.len(), 0);
        assert_eq!(mem.log_len(), 0);
        mem.write("k", "v", "a").unwrap();
        assert!(!mem.is_empty());
        assert_eq!(mem.len(), 1);
        assert_eq!(mem.log_len(), 0);
    }

    #[test]
    fn is_empty_ignores_log_entries() {
        let mem = SyndicateMemory::new();
        mem.append_log("agent-1", "observe", None, "only log").unwrap();
        assert!(mem.is_empty());
        assert_eq!(mem.len(), 0);
        assert_eq!(mem.log_len(), 1);
    }
}
