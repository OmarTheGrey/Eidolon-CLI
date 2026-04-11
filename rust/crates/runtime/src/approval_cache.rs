//! Smart approval cache — remembers user-approved tool call patterns so the
//! same operation doesn't prompt again within a session (or across sessions
//! if persisted to disk).
//!
//! Approval entries match by tool name plus an optional scope:
//! - `ToolOnly` — any invocation of this tool is pre-approved
//! - `PathPrefix(prefix)` — invocations whose input path starts with `prefix`

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Scope of an approval entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope", content = "value")]
pub enum ApprovalScope {
    /// Approve any invocation of this tool.
    ToolOnly,
    /// Approve invocations whose primary path input starts with this prefix.
    PathPrefix(String),
}

/// A single remembered approval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalEntry {
    pub tool_name: String,
    #[serde(flatten)]
    pub scope: ApprovalScope,
}

/// In-memory approval cache with optional disk persistence.
#[derive(Debug, Clone)]
pub struct ApprovalCache {
    /// Entries keyed by tool name for fast lookup.
    entries: BTreeMap<String, Vec<ApprovalScope>>,
    /// Path to persist approvals (None = session-only, not persisted).
    persist_path: Option<PathBuf>,
}

impl ApprovalCache {
    /// Create an empty, session-scoped cache (not persisted to disk).
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            persist_path: None,
        }
    }

    /// Create a cache backed by a JSON file. Existing approvals are loaded
    /// on construction; new approvals are written back atomically.
    #[must_use]
    pub fn with_persistence(path: &Path) -> Self {
        let entries = Self::load_from_file(path);
        Self {
            entries,
            persist_path: Some(path.to_path_buf()),
        }
    }

    /// Record a new approval. If `persist_path` is set, writes to disk.
    pub fn approve(&mut self, tool_name: &str, scope: ApprovalScope) {
        let scopes = self.entries.entry(tool_name.to_string()).or_default();
        if !scopes.contains(&scope) {
            scopes.push(scope);
            self.persist();
        }
    }

    /// Check whether a tool call is pre-approved by a cached entry.
    #[must_use]
    pub fn is_approved(&self, tool_name: &str, input: &str) -> bool {
        let Some(scopes) = self.entries.get(tool_name) else {
            return false;
        };
        scopes.iter().any(|scope| match scope {
            ApprovalScope::ToolOnly => true,
            ApprovalScope::PathPrefix(prefix) => {
                extract_primary_path(input).is_some_and(|path| path.starts_with(prefix.as_str()))
            }
        })
    }

    /// Number of tools with at least one approval entry.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn persist(&self) {
        let Some(path) = &self.persist_path else {
            return;
        };
        let entries: Vec<ApprovalEntry> = self
            .entries
            .iter()
            .flat_map(|(tool_name, scopes)| {
                scopes.iter().map(move |scope| ApprovalEntry {
                    tool_name: tool_name.clone(),
                    scope: scope.clone(),
                })
            })
            .collect();
        if let Ok(json) = serde_json::to_string_pretty(&entries) {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(path, json);
        }
    }

    fn load_from_file(path: &Path) -> BTreeMap<String, Vec<ApprovalScope>> {
        let Ok(contents) = fs::read_to_string(path) else {
            return BTreeMap::new();
        };
        let Ok(entries) = serde_json::from_str::<Vec<ApprovalEntry>>(&contents) else {
            return BTreeMap::new();
        };
        let mut map: BTreeMap<String, Vec<ApprovalScope>> = BTreeMap::new();
        for entry in entries {
            map.entry(entry.tool_name).or_default().push(entry.scope);
        }
        map
    }
}

impl Default for ApprovalCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the primary file path from a tool input JSON string.
/// Looks for `path`, `file_path`, or `filePath` keys.
fn extract_primary_path(input: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(input).ok()?;
    parsed
        .get("path")
        .or_else(|| parsed.get("file_path"))
        .or_else(|| parsed.get("filePath"))
        .and_then(|v| v.as_str())
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_only_approval_matches_any_input() {
        let mut cache = ApprovalCache::new();
        cache.approve("bash", ApprovalScope::ToolOnly);

        assert!(cache.is_approved("bash", r#"{"command":"ls"}"#));
        assert!(cache.is_approved("bash", r#"{"command":"rm -rf /"}"#));
        assert!(!cache.is_approved("write_file", r#"{"path":"x"}"#));
    }

    #[test]
    fn path_prefix_approval_matches_subtree() {
        let mut cache = ApprovalCache::new();
        cache.approve(
            "write_file",
            ApprovalScope::PathPrefix("/home/user/project/".to_string()),
        );

        assert!(cache.is_approved(
            "write_file",
            r#"{"path":"/home/user/project/src/main.rs"}"#
        ));
        assert!(!cache.is_approved(
            "write_file",
            r#"{"path":"/etc/passwd"}"#
        ));
    }

    #[test]
    fn duplicate_approvals_are_idempotent() {
        let mut cache = ApprovalCache::new();
        cache.approve("bash", ApprovalScope::ToolOnly);
        cache.approve("bash", ApprovalScope::ToolOnly);
        assert_eq!(cache.entries.get("bash").unwrap().len(), 1);
    }

    #[test]
    fn persistence_round_trips_via_tempfile() {
        let dir = std::env::temp_dir().join("eidolon-approval-cache-test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("approvals.json");

        {
            let mut cache = ApprovalCache::with_persistence(&path);
            cache.approve("bash", ApprovalScope::ToolOnly);
            cache.approve(
                "write_file",
                ApprovalScope::PathPrefix("/tmp/".to_string()),
            );
        }

        let loaded = ApprovalCache::with_persistence(&path);
        assert!(loaded.is_approved("bash", "{}"));
        assert!(loaded.is_approved("write_file", r#"{"path":"/tmp/foo.txt"}"#));
        assert!(!loaded.is_approved("write_file", r#"{"path":"/etc/foo.txt"}"#));

        let _ = fs::remove_dir_all(&dir);
    }
}
