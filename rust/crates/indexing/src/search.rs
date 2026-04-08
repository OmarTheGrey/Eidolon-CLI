use crate::types::{SearchResult, WorkspaceIndex};

/// Search the index for chunks most similar to the given query embedding.
///
/// Uses brute-force cosine similarity (vectors are pre-normalised so dot
/// product == cosine similarity). Returns up to `top_k` results sorted by
/// descending score.
#[must_use]
pub fn search(index: &WorkspaceIndex, query_embedding: &[f32], top_k: usize) -> Vec<SearchResult> {
    search_with_threshold(index, query_embedding, top_k, 0.0)
}

/// Like [`search`], but discards results below `min_score`.
#[must_use]
pub fn search_with_threshold(
    index: &WorkspaceIndex,
    query_embedding: &[f32],
    top_k: usize,
    min_score: f32,
) -> Vec<SearchResult> {
    let mut scored: Vec<SearchResult> = index
        .entries
        .values()
        .flat_map(|entry| &entry.chunks)
        .filter_map(|chunk| {
            let score = cosine_similarity(&chunk.vector, query_embedding);
            if score >= min_score {
                Some(SearchResult {
                    meta: chunk.meta.clone(),
                    score,
                })
            } else {
                None
            }
        })
        .collect();

    // Sort descending by score.
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(top_k);
    scored
}

/// Cosine similarity between two vectors.
///
/// When both vectors are L2-normalised this reduces to the dot product.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    let denom = norm_a * norm_b;
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ChunkMeta, EmbeddedChunk, IndexEntry, WorkspaceIndex};
    use std::path::PathBuf;

    fn make_chunk(name: &str, vec: Vec<f32>) -> EmbeddedChunk {
        EmbeddedChunk {
            meta: ChunkMeta {
                file_path: PathBuf::from(name),
                start_line: 1,
                end_line: 10,
                content: name.to_string(),
            },
            vector: vec,
        }
    }

    fn normalize(v: &mut [f32]) {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in v.iter_mut() {
                *x /= norm;
            }
        }
    }

    #[test]
    fn search_returns_most_similar_first() {
        let mut v_close = vec![1.0, 0.1, 0.0];
        let mut v_far = vec![0.0, 0.0, 1.0];
        let mut query = vec![1.0, 0.0, 0.0];
        normalize(&mut v_close);
        normalize(&mut v_far);
        normalize(&mut query);

        let mut index = WorkspaceIndex::new("test".into());
        index.entries.insert(
            PathBuf::from("close.rs"),
            IndexEntry {
                file_hash: [0; 32],
                chunks: vec![make_chunk("close.rs", v_close)],
            },
        );
        index.entries.insert(
            PathBuf::from("far.rs"),
            IndexEntry {
                file_hash: [0; 32],
                chunks: vec![make_chunk("far.rs", v_far)],
            },
        );

        let results = search(&index, &query, 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].meta.file_path, PathBuf::from("close.rs"));
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn threshold_filters_low_scores() {
        let mut v = vec![0.0, 0.0, 1.0];
        let mut query = vec![1.0, 0.0, 0.0];
        normalize(&mut v);
        normalize(&mut query);

        let mut index = WorkspaceIndex::new("test".into());
        index.entries.insert(
            PathBuf::from("ortho.rs"),
            IndexEntry {
                file_hash: [0; 32],
                chunks: vec![make_chunk("ortho.rs", v)],
            },
        );

        let results = search_with_threshold(&index, &query, 10, 0.5);
        assert!(results.is_empty());
    }

    #[test]
    fn top_k_limits_results() {
        let mut index = WorkspaceIndex::new("test".into());
        for i in 0..20 {
            let mut v = vec![0.0; 3];
            #[allow(clippy::cast_precision_loss)]
            {
                v[0] = i as f32;
            }
            normalize(&mut v);
            index.entries.insert(
                PathBuf::from(format!("file_{i}.rs")),
                IndexEntry {
                    file_hash: [0; 32],
                    chunks: vec![make_chunk(&format!("file_{i}.rs"), v)],
                },
            );
        }

        let query = vec![1.0, 0.0, 0.0];
        let results = search(&index, &query, 5);
        assert_eq!(results.len(), 5);
    }
}
