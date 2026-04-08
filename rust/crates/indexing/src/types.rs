use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::SystemTime;

/// Metadata for a single chunk of source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMeta {
    /// Path relative to workspace root.
    pub file_path: PathBuf,
    /// 1-indexed start line in the original file.
    pub start_line: usize,
    /// 1-indexed end line (inclusive).
    pub end_line: usize,
    /// Raw text content of the chunk.
    pub content: String,
}

/// A chunk together with its embedding vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedChunk {
    pub meta: ChunkMeta,
    /// 384-dim L2-normalized embedding.
    pub vector: Vec<f32>,
}

/// All chunks originating from a single file, keyed by content hash for
/// incremental re-indexing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    /// SHA-256 of the file content at index time.
    pub file_hash: [u8; 32],
    pub chunks: Vec<EmbeddedChunk>,
}

/// The full workspace index — maps relative file paths to their entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceIndex {
    pub entries: BTreeMap<PathBuf, IndexEntry>,
    pub model_id: String,
    pub created_at: SystemTime,
}

impl WorkspaceIndex {
    #[must_use]
    pub fn new(model_id: String) -> Self {
        Self {
            entries: BTreeMap::new(),
            model_id,
            created_at: SystemTime::now(),
        }
    }

    /// Total number of embedded chunks across all files.
    #[must_use]
    pub fn chunk_count(&self) -> usize {
        self.entries.values().map(|e| e.chunks.len()).sum()
    }

    /// Number of indexed files.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.entries.len()
    }
}

/// Runtime configuration for the indexing subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    pub enabled: bool,
    pub model_id: String,
    /// Number of lines per chunk.
    pub chunk_lines: usize,
    /// Overlap between consecutive chunks.
    pub overlap_lines: usize,
    /// Skip files larger than this.
    pub max_file_size_bytes: usize,
    /// Number of chunks injected as auto-context before each turn.
    pub auto_context_top_k: usize,
    pub auto_context_enabled: bool,
    /// Where to persist the serialized index.
    pub cache_dir: PathBuf,
    /// File extensions to skip (without leading dot).
    pub excluded_extensions: Vec<String>,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            model_id: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            chunk_lines: 50,
            overlap_lines: 10,
            max_file_size_bytes: 524_288, // 512 KB
            auto_context_top_k: 5,
            auto_context_enabled: true,
            cache_dir: PathBuf::from(".eidolon/.index-cache"),
            excluded_extensions: vec![
                "png", "jpg", "jpeg", "gif", "bmp", "ico", "svg", "webp", "wasm", "lock", "min.js",
                "min.css", "map", "bin", "exe", "dll", "so", "dylib", "o", "a", "pyc", "class",
                "jar", "zip", "tar", "gz", "bz2", "7z", "rar", "pdf", "doc", "docx",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        }
    }
}

/// A single search result returned by the index.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub meta: ChunkMeta,
    /// Cosine similarity score (0.0–1.0).
    pub score: f32,
}

/// Progress report emitted during index construction.
#[derive(Debug, Clone)]
pub struct IndexProgress {
    pub files_total: usize,
    pub files_done: usize,
    pub chunks_total: usize,
    pub chunks_embedded: usize,
}
