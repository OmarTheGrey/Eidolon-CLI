use crate::types::WorkspaceIndex;
use std::fs;
use std::io;
use std::path::Path;

const CACHE_FILE_NAME: &str = "workspace.idx";

/// Persist the index to disk using bincode.
///
/// Writes atomically: data goes to a `.tmp` file first, then is renamed into
/// place, preventing corruption from interrupted writes.
pub fn save_cache(index: &WorkspaceIndex, cache_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(cache_dir)?;

    let final_path = cache_dir.join(CACHE_FILE_NAME);
    let tmp_path = cache_dir.join(format!("{CACHE_FILE_NAME}.tmp"));

    let bytes = bincode::serialize(index).map_err(io::Error::other)?;

    fs::write(&tmp_path, &bytes)?;
    fs::rename(&tmp_path, &final_path)?;
    Ok(())
}

/// Load a previously cached index.
///
/// Returns `Ok(None)` if no cache exists or it cannot be decoded (corrupt /
/// schema change). Returns `Ok(None)` if the cached `model_id` doesn't match
/// `expected_model_id` (model change → must rebuild).
pub fn load_cache(cache_dir: &Path, expected_model_id: &str) -> io::Result<Option<WorkspaceIndex>> {
    let path = cache_dir.join(CACHE_FILE_NAME);
    if !path.exists() {
        return Ok(None);
    }

    let bytes = fs::read(&path)?;
    let index: WorkspaceIndex = match bincode::deserialize(&bytes) {
        Ok(idx) => idx,
        Err(_) => return Ok(None), // corrupt or schema change
    };

    if index.model_id != expected_model_id {
        return Ok(None); // model changed → rebuild
    }

    Ok(Some(index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::WorkspaceIndex;

    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let index = WorkspaceIndex::new("test-model".into());

        save_cache(&index, dir.path()).unwrap();
        let loaded = load_cache(dir.path(), "test-model").unwrap();

        let loaded = loaded.expect("cache should load");
        assert_eq!(loaded.model_id, "test-model");
        assert_eq!(loaded.file_count(), 0);
    }

    #[test]
    fn model_mismatch_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let index = WorkspaceIndex::new("model-a".into());
        save_cache(&index, dir.path()).unwrap();

        let loaded = load_cache(dir.path(), "model-b").unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn missing_cache_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = load_cache(dir.path(), "any").unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn corrupt_cache_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CACHE_FILE_NAME);
        fs::write(&path, b"not valid bincode").unwrap();

        let loaded = load_cache(dir.path(), "any").unwrap();
        assert!(loaded.is_none());
    }
}
