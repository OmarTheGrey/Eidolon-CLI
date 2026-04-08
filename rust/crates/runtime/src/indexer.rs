//! Background workspace indexer.
//!
//! Spawns a dedicated OS thread (CPU-heavy embedding work should not starve the
//! async runtime) that builds an [`indexing::WorkspaceIndex`], optionally
//! loading a cached index first for incremental updates. Consumers interact
//! through the lightweight [`IndexHandle`].

use indexing::{
    build_index, ensure_model, load_cache, load_model, save_cache, search, search_with_threshold,
    Embedder, IndexConfig, IndexProgress, SearchResult, WorkspaceIndex,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

/// A cheaply cloneable handle to a workspace index that may still be building.
#[derive(Clone)]
pub struct IndexHandle {
    inner: Arc<IndexHandleInner>,
}

struct IndexHandleInner {
    /// Populated once the background build finishes.
    index: OnceLock<WorkspaceIndex>,
    /// Updated periodically by the builder thread.
    progress: Mutex<Option<IndexProgress>>,
    /// Embedder is needed at query time to embed the user's query.
    embedder: OnceLock<Embedder>,
    config: IndexConfig,
}

impl IndexHandle {
    /// Whether the index is ready for queries.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.inner.index.get().is_some()
    }

    /// Current build progress, if the indexer is still running.
    #[must_use]
    pub fn progress(&self) -> Option<IndexProgress> {
        self.inner.progress.lock().ok()?.clone()
    }

    /// Query the index for the top-K chunks most similar to `query_text`.
    ///
    /// Returns `None` if the index or embedder aren't ready yet.
    #[must_use]
    pub fn query(&self, query_text: &str, top_k: usize) -> Option<Vec<SearchResult>> {
        let index = self.inner.index.get()?;
        let embedder = self.inner.embedder.get()?;
        let query_vec = embedder.embed_one(query_text).ok()?;
        Some(search_with_threshold(index, &query_vec, top_k, 0.3))
    }

    /// Query the index without a minimum score threshold.
    #[must_use]
    pub fn query_unfiltered(&self, query_text: &str, top_k: usize) -> Option<Vec<SearchResult>> {
        let index = self.inner.index.get()?;
        let embedder = self.inner.embedder.get()?;
        let query_vec = embedder.embed_one(query_text).ok()?;
        Some(search(index, &query_vec, top_k))
    }

    /// Snapshot of index stats (file count, chunk count). `None` if not ready.
    #[must_use]
    pub fn stats(&self) -> Option<(usize, usize)> {
        let idx = self.inner.index.get()?;
        Some((idx.file_count(), idx.chunk_count()))
    }

    #[must_use]
    pub fn config(&self) -> &IndexConfig {
        &self.inner.config
    }
}

/// Start the background indexer and return a handle immediately.
///
/// The actual index build happens on a dedicated OS thread. The returned
/// handle can be polled for readiness or queried once populated.
#[must_use]
pub fn start_background_indexer(workspace_root: PathBuf, config: IndexConfig) -> IndexHandle {
    let inner = Arc::new(IndexHandleInner {
        index: OnceLock::new(),
        progress: Mutex::new(None),
        embedder: OnceLock::new(),
        config: config.clone(),
    });

    let handle = IndexHandle {
        inner: Arc::clone(&inner),
    };

    std::thread::Builder::new()
        .name("eidolon-indexer".into())
        .spawn(move || {
            if let Err(e) = run_indexer(&inner, &workspace_root, &config) {
                eprintln!("[indexer] build failed: {e}");
            }
        })
        .expect("failed to spawn indexer thread");

    handle
}

fn run_indexer(
    inner: &IndexHandleInner,
    workspace_root: &std::path::Path,
    config: &IndexConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Download / locate model.
    let model_dir = ensure_model(&config.model_id)?;
    let (model, tokenizer) = load_model(&model_dir)?;
    let embedder = Embedder::new(model, tokenizer);

    // 2. Load cache if available.
    let cache_dir = workspace_root.join(&config.cache_dir);
    let existing = load_cache(&cache_dir, &config.model_id)?;

    // 3. Progress reporter.
    let progress_ref = &inner.progress;
    let on_progress = |p: IndexProgress| {
        if let Ok(mut guard) = progress_ref.lock() {
            *guard = Some(p);
        }
    };

    // 4. Build index.
    let index = build_index(
        workspace_root,
        config,
        &embedder,
        existing.as_ref(),
        on_progress,
    )?;

    // 5. Persist cache (best-effort).
    let _ = save_cache(&index, &cache_dir);

    // 6. Publish.
    let _ = inner.index.set(index);
    let _ = inner.embedder.set(embedder);

    Ok(())
}
