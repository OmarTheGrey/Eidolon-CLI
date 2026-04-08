use crate::types::IndexConfig;
use ignore::WalkBuilder;
use std::fs;
use std::path::{Path, PathBuf};

/// Walk the workspace tree and return paths eligible for indexing.
///
/// Respects `.gitignore` (and `.ignore` / `.eidolonignore` if present).
/// Skips binary files, files exceeding the configured size limit, and files
/// whose extension appears in the exclusion list.
#[must_use]
pub fn discover_files(workspace_root: &Path, config: &IndexConfig) -> Vec<PathBuf> {
    let mut files = Vec::new();

    let walker = WalkBuilder::new(workspace_root)
        .hidden(true) // skip hidden dirs like .git
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .add_custom_ignore_filename(".eidolonignore")
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        // Extension filter — check both simple extension and compound extension
        // (e.g. ".min.js" and ".js" for "app.min.js").
        if is_excluded_extension(path, &config.excluded_extensions) {
            continue;
        }

        // Size filter
        if let Ok(meta) = fs::metadata(path) {
            if usize::try_from(meta.len()).unwrap_or(usize::MAX) > config.max_file_size_bytes {
                continue;
            }
        }

        // Binary detection: read first 8 KB and look for null bytes
        if is_likely_binary(path) {
            continue;
        }

        files.push(path.to_path_buf());
    }

    files.sort();
    files
}

/// Check if a file's extension (simple or compound) is in the exclusion list.
///
/// For `app.min.js` this checks both `"js"` and `"min.js"`.
fn is_excluded_extension(path: &Path, excluded: &[String]) -> bool {
    let name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n.to_ascii_lowercase(),
        None => return false,
    };

    // Simple extension (last component after final dot).
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_ascii_lowercase();
        if excluded.iter().any(|x| x == &ext_lower) {
            return true;
        }
    }

    // Compound extensions: check all suffixes like ".min.js", ".tar.gz".
    for exc in excluded {
        let suffix = format!(".{exc}");
        if name.ends_with(&suffix) {
            return true;
        }
    }

    false
}

/// Heuristic: a file is likely binary if its first 8 KB contain a null byte.
fn is_likely_binary(path: &Path) -> bool {
    let Ok(data) = fs::read(path) else {
        return true; // unreadable → skip
    };
    let check_len = data.len().min(8192);
    data[..check_len].contains(&0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn discovers_text_files_skips_binary() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("hello.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("image.png"), [0u8; 64]).unwrap();

        let config = IndexConfig::default();
        let files = discover_files(dir.path(), &config);

        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("hello.rs"));
    }

    #[test]
    fn skips_excluded_extensions() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("app.min.js"), "var x=1;").unwrap();
        fs::write(dir.path().join("lib.rs"), "pub fn f(){}").unwrap();

        let config = IndexConfig::default();
        let files = discover_files(dir.path(), &config);

        assert!(files.iter().any(|p| p.ends_with("lib.rs")));
        // min.js is in the excluded list
        assert!(!files.iter().any(|p| p.ends_with("app.min.js")));
    }

    #[test]
    fn skips_oversized_files() {
        let dir = tempfile::tempdir().unwrap();
        let big = vec![b'x'; 600_000];
        fs::write(dir.path().join("big.txt"), &big).unwrap();
        fs::write(dir.path().join("small.txt"), "hi").unwrap();

        let config = IndexConfig::default();
        let files = discover_files(dir.path(), &config);

        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("small.txt"));
    }
}
