//! Codebase indexing using local embedding models.
//!
//! This crate provides:
//! - **File discovery** (`discovery`) – gitignore-aware workspace walking
//! - **Chunking** (`chunker`) – line-based sliding-window text splitting
//! - **Model management** (`model`) – download and load `all-MiniLM-L6-v2`
//!   (or compatible) via Hugging Face Hub + Candle
//! - **Embedding** (`embedder`) – batch BERT inference with mean-pooling
//! - **Search** (`search`) – brute-force cosine-similarity ranking
//! - **Cache** (`cache`) – bincode persistence for incremental rebuilds
//! - **Index builder** (`index`) – orchestrates discovery → chunk → embed

pub mod cache;
pub mod chunker;
pub mod discovery;
pub mod embedder;
pub mod index;
pub mod model;
pub mod search;
pub mod types;

// Re-export the most commonly used items at crate root.
pub use cache::{load_cache, save_cache};
pub use embedder::Embedder;
pub use index::{build_index, IndexBuildError};
pub use model::{ensure_model, load_model, ModelError};
pub use search::{search, search_with_threshold};
pub use types::{
    ChunkMeta, EmbeddedChunk, IndexConfig, IndexEntry, IndexProgress, SearchResult, WorkspaceIndex,
};
