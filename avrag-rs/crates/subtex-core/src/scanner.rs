use crate::error::SubtexError;
use ignore::WalkBuilder;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

/// Directories skipped on top of gitignore semantics (they also apply outside
/// git repositories, where the `ignore` crate's gitignore rules need
/// `require_git(false)`).
const SKIPPED_DIR_NAMES: [&str; 3] = ["node_modules", "target", "__pycache__"];
const HASH_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedFile {
    /// Root-relative path with `/` separators.
    pub rel_path: String,
    /// SHA-256 hex of the file bytes (per-file incremental identity).
    pub content_hash: String,
    pub size: u64,
    pub mtime_ms: u64,
}

/// Previously indexed state of one file, keyed by rel path. When size and
/// mtime are unchanged the hash is reused instead of re-reading the file —
/// the reconcile sweep runs on a timer, so it must not re-hash every byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedEntry {
    pub content_hash: String,
    pub size: u64,
    pub mtime_ms: u64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DirDiff {
    pub added: Vec<ScannedFile>,
    pub modified: Vec<ScannedFile>,
    pub removed: Vec<String>,
}

/// Walk the root once and return every indexable file with its content hash.
///
/// Hidden entries and gitignore-style ignores are respected (also outside git
/// repositories); unreadable entries are skipped so one bad file cannot fail a
/// whole scan.
pub fn scan_dir(root: &Path) -> Result<Vec<ScannedFile>, SubtexError> {
    scan_dir_cached(root, &HashMap::new())
}

/// Cached variant: files whose size and mtime match `previous` reuse the old
/// hash instead of re-reading content.
pub fn scan_dir_cached(
    root: &Path,
    previous: &HashMap<String, CachedEntry>,
) -> Result<Vec<ScannedFile>, SubtexError> {
    let canonical = fs::canonicalize(root).map_err(|source| SubtexError::Canonicalize {
        path: root.to_path_buf(),
        source,
    })?;
    let walker = WalkBuilder::new(&canonical)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .require_git(false)
        .parents(false)
        .filter_entry(|entry| {
            if entry.file_type().is_some_and(|t| t.is_dir()) {
                let name = entry.file_name().to_string_lossy();
                !SKIPPED_DIR_NAMES.contains(&name.as_ref())
            } else {
                true
            }
        })
        .build();

    let mut files = Vec::new();
    for entry in walker {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(&canonical) else {
            continue;
        };
        let rel_path = rel.to_string_lossy().replace('\\', "/");
        if let Some((path_meta, cached)) = file_metadata(entry.path())
            .ok()
            .zip(previous.get(&rel_path))
        {
            if path_meta.0 == cached.size && path_meta.1 == cached.mtime_ms {
                files.push(ScannedFile {
                    rel_path,
                    content_hash: cached.content_hash.clone(),
                    size: cached.size,
                    mtime_ms: cached.mtime_ms,
                });
                continue;
            }
        }
        let Ok((content_hash, size, mtime_ms)) = hash_file(entry.path()) else {
            continue;
        };
        files.push(ScannedFile {
            rel_path,
            content_hash,
            size,
            mtime_ms,
        });
    }
    files.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(files)
}

/// Scan one file (single-file reindex jobs). `None` when missing/unreadable.
/// Paths that escape the root are an error, not a miss.
pub fn scan_file(root: &Path, rel: &str) -> Result<Option<ScannedFile>, SubtexError> {
    let path = crate::root::resolve_under_root(root, rel)?;
    if !path.is_file() {
        return Ok(None);
    }
    let Ok((content_hash, size, mtime_ms)) = hash_file(&path) else {
        return Ok(None);
    };
    let rel_path = crate::root::rel_under_root(root, rel)?;
    Ok(Some(ScannedFile {
        rel_path,
        content_hash,
        size,
        mtime_ms,
    }))
}

/// `(size, mtime_ms)` for cache comparison.
fn file_metadata(path: &Path) -> std::io::Result<(u64, u64)> {
    let meta = fs::metadata(path)?;
    let mtime_ms = meta
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    Ok((meta.len(), mtime_ms))
}

/// Diff a scan against the previous `rel_path -> content_hash` map (e.g. from
/// the store's `files` table). All outputs sorted by `rel_path`.
pub fn scan_diff(previous: &HashMap<String, String>, current: &[ScannedFile]) -> DirDiff {
    let mut diff = DirDiff::default();
    let current_set: HashMap<&str, &ScannedFile> = current
        .iter()
        .map(|f| (f.rel_path.as_str(), f))
        .collect();

    for file in current {
        match previous.get(&file.rel_path) {
            None => diff.added.push(file.clone()),
            Some(old_hash) if old_hash != &file.content_hash => diff.modified.push(file.clone()),
            Some(_) => {}
        }
    }
    for prev_path in previous.keys() {
        if !current_set.contains_key(prev_path.as_str()) {
            diff.removed.push(prev_path.clone());
        }
    }
    diff.added.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    diff.modified.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    diff.removed.sort();
    diff
}

fn hash_file(path: &Path) -> std::io::Result<(String, u64, u64)> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; HASH_BUFFER_BYTES];
    let mut size = 0u64;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        size += n as u64;
    }
    let mtime_ms = fs::metadata(path)?
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    Ok((hex::encode(hasher.finalize()), size, mtime_ms))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Markdown,
    Text,
    Code,
    Csv,
    Pdf,
    Docx,
    Audio,
    Other,
}

impl FileKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileKind::Markdown => "markdown",
            FileKind::Text => "text",
            FileKind::Code => "code",
            FileKind::Csv => "csv",
            FileKind::Pdf => "pdf",
            FileKind::Docx => "docx",
            FileKind::Audio => "audio",
            FileKind::Other => "other",
        }
    }
}

const MARKDOWN_EXTS: [&str; 2] = ["md", "markdown"];
const TEXT_EXTS: [&str; 5] = ["txt", "rst", "ini", "cfg", "conf"];
const CODE_EXTS: [&str; 36] = [
    "rs", "py", "ts", "tsx", "js", "jsx", "mjs", "cjs", "go", "java", "c", "h", "cc", "cpp", "hpp",
    "cs", "rb", "php", "swift", "kt", "scala", "lua", "sh", "bash", "zsh", "ps1", "sql", "html",
    "htm", "css", "scss", "vue", "svelte", "json", "yaml", "yml",
];
const AUDIO_EXTS: [&str; 9] = ["wav", "mp3", "m4a", "aac", "ogg", "flac", "opus", "wma", "amr"];

/// Coarse file classification for directory facts and parse routing.
pub fn file_kind(path: &Path) -> FileKind {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        e if MARKDOWN_EXTS.contains(&e) => FileKind::Markdown,
        e if TEXT_EXTS.contains(&e) => FileKind::Text,
        e if CODE_EXTS.contains(&e) => FileKind::Code,
        "csv" | "tsv" => FileKind::Csv,
        "pdf" => FileKind::Pdf,
        "docx" | "doc" => FileKind::Docx,
        e if AUDIO_EXTS.contains(&e) => FileKind::Audio,
        _ => FileKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn write(root: &Path, rel: &str, content: &[u8]) -> PathBuf {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn scan_respects_gitignore_hidden_and_denylist() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        write(&root, "notes.md", b"# hello");
        write(&root, "ignored.log", b"noise");
        write(&root, "node_modules/pkg/x.js", b"dep");
        write(&root, "target/debug/y.rs", b"build");
        write(&root, ".hidden", b"secret");
        write(&root, "src/main.rs", b"fn main() {}");
        fs::write(root.join(".gitignore"), "ignored.log\n").unwrap();

        let files = scan_dir(&root).unwrap();
        let rels: Vec<&str> = files.iter().map(|f| f.rel_path.as_str()).collect();
        assert_eq!(rels, vec!["notes.md", "src/main.rs"]);
    }

    #[test]
    fn scan_hash_is_content_sha256_and_stable() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        write(&root, "a.md", b"same bytes");

        let first = scan_dir(&root).unwrap();
        let second = scan_dir(&root).unwrap();
        assert_eq!(first, second);
        let expected = hex::encode(Sha256::digest(b"same bytes"));
        assert_eq!(first[0].content_hash, expected);
        assert!(first[0].mtime_ms > 0);
        assert_eq!(first[0].size, 10);
    }

    #[test]
    fn scan_diff_classifies_add_modify_remove() {
        let previous = HashMap::from([
            ("kept.md".to_string(), "h1".to_string()),
            ("changed.md".to_string(), "h2".to_string()),
            ("gone.md".to_string(), "h3".to_string()),
        ]);
        let current = vec![
            ScannedFile {
                rel_path: "changed.md".into(),
                content_hash: "h2b".into(),
                size: 1,
                mtime_ms: 1,
            },
            ScannedFile {
                rel_path: "kept.md".into(),
                content_hash: "h1".into(),
                size: 1,
                mtime_ms: 1,
            },
            ScannedFile {
                rel_path: "new.md".into(),
                content_hash: "h4".into(),
                size: 1,
                mtime_ms: 1,
            },
        ];

        let diff = scan_diff(&previous, &current);
        assert_eq!(diff.added.iter().map(|f| f.rel_path.as_str()).collect::<Vec<_>>(), ["new.md"]);
        assert_eq!(
            diff.modified.iter().map(|f| f.rel_path.as_str()).collect::<Vec<_>>(),
            ["changed.md"]
        );
        assert_eq!(diff.removed, vec!["gone.md"]);
    }

    #[test]
    fn file_kind_maps_extensions() {
        assert_eq!(file_kind(Path::new("a.MD")), FileKind::Markdown);
        assert_eq!(file_kind(Path::new("b.txt")), FileKind::Text);
        assert_eq!(file_kind(Path::new("c.rs")), FileKind::Code);
        assert_eq!(file_kind(Path::new("d.csv")), FileKind::Csv);
        assert_eq!(file_kind(Path::new("e.pdf")), FileKind::Pdf);
        assert_eq!(file_kind(Path::new("f.docx")), FileKind::Docx);
        assert_eq!(file_kind(Path::new("g.m4a")), FileKind::Audio);
        assert_eq!(file_kind(Path::new("h.amr")), FileKind::Audio);
        assert_eq!(file_kind(Path::new("i.weird")), FileKind::Other);
        assert_eq!(file_kind(Path::new("j")), FileKind::Other);
    }

    #[test]
    fn scan_file_rejects_paths_outside_root() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        write(&root, "inside.md", b"ok");
        let outside = dir.path().parent().unwrap().join("outside.md");
        fs::write(&outside, b"nope").ok();

        let hit = scan_file(&root, "inside.md").unwrap().unwrap();
        assert_eq!(hit.rel_path, "inside.md");
        assert!(scan_file(&root, "missing.md").unwrap().is_none());
        assert!(scan_file(&root, "../outside.md").is_err());
    }
}
