use crate::chunker::chunk_file;
use crate::discovery::discover_files;
use crate::embedder::Embedder;
use crate::types::{EmbeddedChunk, IndexConfig, IndexEntry, IndexProgress, WorkspaceIndex};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

/// Build (or incrementally update) a workspace index.
///
/// If an `existing` index is provided, files whose SHA-256 hash hasn't changed
/// are carried over without re-embedding. The `on_progress` callback is
/// invoked after each file is processed.
pub fn build_index(
    workspace_root: &Path,
    config: &IndexConfig,
    embedder: &Embedder,
    existing: Option<&WorkspaceIndex>,
    mut on_progress: impl FnMut(IndexProgress),
) -> Result<WorkspaceIndex, IndexBuildError> {
    let files = discover_files(workspace_root, config);
    let files_total = files.len();

    let mut entries: BTreeMap<std::path::PathBuf, IndexEntry> = BTreeMap::new();
    let mut chunks_total = 0usize;
    let mut chunks_embedded = 0usize;

    for (i, file_path) in files.iter().enumerate() {
        let content = std::fs::read(file_path)
            .map_err(|e| IndexBuildError(format!("read {}: {e}", file_path.display())))?;

        let hash: [u8; 32] = Sha256::digest(&content).into();

        let relative = file_path
            .strip_prefix(workspace_root)
            .unwrap_or(file_path)
            .to_path_buf();

        // Check if we can reuse the existing entry (same hash).
        if let Some(prev) = existing {
            if let Some(entry) = prev.entries.get(&relative) {
                if entry.file_hash == hash {
                    chunks_total += entry.chunks.len();
                    entries.insert(relative, entry.clone());

                    on_progress(IndexProgress {
                        files_total,
                        files_done: i + 1,
                        chunks_total,
                        chunks_embedded,
                    });
                    continue;
                }
            }
        }

        // Chunk the file.
        let chunk_metas = chunk_file(file_path, workspace_root, config);
        if chunk_metas.is_empty() {
            on_progress(IndexProgress {
                files_total,
                files_done: i + 1,
                chunks_total,
                chunks_embedded,
            });
            continue;
        }

        // Embed in batches.
        let texts: Vec<&str> = chunk_metas.iter().map(|c| c.content.as_str()).collect();
        let mut all_vectors = Vec::with_capacity(texts.len());

        for batch_start in (0..texts.len()).step_by(32) {
            let batch_end = (batch_start + 32).min(texts.len());
            let batch = &texts[batch_start..batch_end];
            let vecs = embedder
                .embed_batch(batch)
                .map_err(|e| IndexBuildError(format!("embed: {e}")))?;
            all_vectors.extend(vecs);
        }

        let embedded_chunks: Vec<EmbeddedChunk> = chunk_metas
            .into_iter()
            .zip(all_vectors)
            .map(|(meta, vector)| EmbeddedChunk { meta, vector })
            .collect();

        chunks_total += embedded_chunks.len();
        chunks_embedded += embedded_chunks.len();

        entries.insert(
            relative,
            IndexEntry {
                file_hash: hash,
                chunks: embedded_chunks,
            },
        );

        on_progress(IndexProgress {
            files_total,
            files_done: i + 1,
            chunks_total,
            chunks_embedded,
        });
    }

    let mut index = WorkspaceIndex::new(config.model_id.clone());
    index.entries = entries;
    Ok(index)
}

#[derive(Debug)]
pub struct IndexBuildError(pub String);

impl std::fmt::Display for IndexBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "index build error: {}", self.0)
    }
}

impl std::error::Error for IndexBuildError {}
