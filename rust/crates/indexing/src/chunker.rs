use crate::types::{ChunkMeta, IndexConfig};
use std::fs;
use std::path::Path;

/// Split a file into overlapping chunks according to the index configuration.
///
/// Each chunk records the containing file path (relative to `workspace_root`),
/// the 1-indexed start and end line numbers, and the raw text content with a
/// `// File: <path>` header prepended for embedding context.
#[must_use]
pub fn chunk_file(file_path: &Path, workspace_root: &Path, config: &IndexConfig) -> Vec<ChunkMeta> {
    let Ok(content) = fs::read_to_string(file_path) else {
        return Vec::new();
    };

    let relative = file_path.strip_prefix(workspace_root).unwrap_or(file_path);
    let rel_display = relative.display();

    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }

    let window = config.chunk_lines;
    let step = window.saturating_sub(config.overlap_lines).max(1);

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < lines.len() {
        let end = (start + window).min(lines.len());
        let chunk_lines = &lines[start..end];

        let header = format!("// File: {rel_display}\n");
        let body = chunk_lines.join("\n");
        let content = format!("{header}{body}");

        chunks.push(ChunkMeta {
            file_path: relative.to_path_buf(),
            start_line: start + 1, // 1-indexed
            end_line: end,         // 1-indexed inclusive
            content,
        });

        if end >= lines.len() {
            break;
        }
        start += step;
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_config(chunk_lines: usize, overlap: usize) -> IndexConfig {
        IndexConfig {
            chunk_lines,
            overlap_lines: overlap,
            ..IndexConfig::default()
        }
    }

    #[test]
    fn small_file_single_chunk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tiny.rs");
        fs::write(&path, "line1\nline2\nline3").unwrap();

        let chunks = chunk_file(&path, dir.path(), &make_config(50, 10));
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 3);
        assert!(chunks[0].content.contains("// File: tiny.rs"));
    }

    #[test]
    fn overlap_produces_correct_windows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lines.txt");
        // 20 lines, chunk_lines=10, overlap=3 → step=7
        let content: String = (1..=20).fold(String::new(), |mut s, i| {
            use std::fmt::Write;
            writeln!(s, "line {i}").unwrap();
            s
        });
        fs::write(&path, &content).unwrap();

        let chunks = chunk_file(&path, dir.path(), &make_config(10, 3));
        assert!(chunks.len() >= 2);

        // First chunk: lines 1–10
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 10);
        // Second chunk starts at line 8 (7 step + 1-indexed)
        assert_eq!(chunks[1].start_line, 8);
    }

    #[test]
    fn empty_file_no_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.txt");
        fs::write(&path, "").unwrap();

        let chunks = chunk_file(&path, dir.path(), &make_config(50, 10));
        assert!(chunks.is_empty());
    }

    #[test]
    fn chunk_content_includes_file_header() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("src");
        fs::create_dir_all(&sub).unwrap();
        let path = sub.join("main.rs");
        fs::write(&path, "fn main() {\n    println!(\"hello\");\n}").unwrap();

        let chunks = chunk_file(&path, dir.path(), &make_config(50, 10));
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.starts_with("// File: src"));
    }
}
