use crate::error::SubtexError;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const SUBTEX_DIR_NAME: &str = "subtex";
pub const STORE_DB_FILE: &str = "index.db";
pub const ROOT_POINTER_FILE: &str = "root.txt";
pub const SCRATCH_DIR_NAME: &str = "scratch";
pub const ROOTS_DIR_NAME: &str = "roots";

/// Identity of one attached directory: its canonical path plus the derived,
/// stable store directory under the Subtex data dir.
///
/// Store layout (`<data>/roots/<sha256(canonical_root)>/`):
/// - `index.db`  — SQLite: chunks + FTS5 + vec0 + meta + jobs + usage + readiness
/// - `root.txt`  — the canonical root path (known-root enumeration reads this,
///   never scans the disk)
/// - `scratch/`  — transcription intermediates; never in the project directory
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootHandle {
    root: PathBuf,
    hash: String,
    store_dir: PathBuf,
}

impl RootHandle {
    pub fn open(root: &Path) -> Result<Self, SubtexError> {
        Self::with_data_dir(root, default_data_dir()?)
    }

    pub fn with_data_dir(root: &Path, data_dir: PathBuf) -> Result<Self, SubtexError> {
        let canonical = fs::canonicalize(root).map_err(|source| SubtexError::Canonicalize {
            path: root.to_path_buf(),
            source,
        })?;
        if !canonical.is_dir() {
            return Err(SubtexError::RootNotFound(canonical));
        }
        let hash = root_hash(&canonical);
        let store_dir = data_dir.join(ROOTS_DIR_NAME).join(&hash);
        Ok(Self {
            root: canonical,
            hash,
            store_dir,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn store_dir(&self) -> &Path {
        &self.store_dir
    }

    pub fn db_path(&self) -> PathBuf {
        self.store_dir.join(STORE_DB_FILE)
    }

    pub fn root_pointer_path(&self) -> PathBuf {
        self.store_dir.join(ROOT_POINTER_FILE)
    }

    pub fn scratch_dir(&self) -> PathBuf {
        self.store_dir.join(SCRATCH_DIR_NAME)
    }
}

/// Resolve `rel` so the result is always a path inside `root`.
///
/// Absolute paths, `..` escapes, and empty/whitespace-only inputs are rejected.
/// `.` / redundant inner `..` that stay inside the root are normalized.
pub fn resolve_under_root(root: &Path, rel: &str) -> Result<PathBuf, SubtexError> {
    let rel = rel.trim();
    let rel_path = Path::new(rel);
    if rel.is_empty() || rel_path.is_absolute() {
        return Err(SubtexError::PathOutsideRoot {
            root: root.to_path_buf(),
            path: PathBuf::from(rel),
        });
    }
    let canonical = fs::canonicalize(root).map_err(|source| SubtexError::Canonicalize {
        path: root.to_path_buf(),
        source,
    })?;
    let mut out = canonical.clone();
    for component in rel_path.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
                if !out.starts_with(&canonical) {
                    return Err(SubtexError::PathOutsideRoot {
                        root: canonical,
                        path: rel_path.to_path_buf(),
                    });
                }
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(SubtexError::PathOutsideRoot {
                    root: canonical,
                    path: rel_path.to_path_buf(),
                });
            }
        }
    }
    if !out.starts_with(&canonical) {
        return Err(SubtexError::PathOutsideRoot {
            root: canonical,
            path: out,
        });
    }
    Ok(out)
}

/// `resolve_under_root` then the `/`-separated path relative to the canonical root.
pub fn rel_under_root(root: &Path, rel: &str) -> Result<String, SubtexError> {
    let abs = resolve_under_root(root, rel)?;
    let canonical = fs::canonicalize(root).map_err(|source| SubtexError::Canonicalize {
        path: root.to_path_buf(),
        source,
    })?;
    let stripped = abs
        .strip_prefix(&canonical)
        .map_err(|_| SubtexError::PathOutsideRoot {
            root: canonical.clone(),
            path: abs.clone(),
        })?;
    Ok(stripped.to_string_lossy().replace('\\', "/"))
}

/// SHA-256 of the canonical root path, hex-encoded — the store directory name.
pub fn root_hash(canonical_root: &Path) -> String {
    let digest = Sha256::digest(canonical_root.to_string_lossy().as_bytes());
    hex::encode(digest)
}

/// `<data>/subtex` data directory: `SUBTEX_DATA_DIR` wins, else
/// `$XDG_DATA_HOME/subtex`, else `~/.local/share/subtex`.
pub fn default_data_dir() -> Result<PathBuf, SubtexError> {
    if let Some(dir) = std::env::var_os("SUBTEX_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let base = match std::env::var_os("XDG_DATA_HOME") {
        Some(xdg) => PathBuf::from(xdg),
        None => {
            let home = std::env::var_os("HOME").ok_or(SubtexError::HomeNotSet)?;
            PathBuf::from(home).join(".local").join("share")
        }
    };
    Ok(base.join(SUBTEX_DIR_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tmp_root() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let canonical = fs::canonicalize(dir.path()).unwrap();
        (dir, canonical)
    }

    #[test]
    fn hash_is_stable_across_path_spellings() {
        let (dir, canonical) = tmp_root();
        fs::create_dir(dir.path().join("sub")).unwrap();

        let a = root_hash(&canonical);
        let b = root_hash(&fs::canonicalize(dir.path().join(".")).unwrap());
        let c = root_hash(&fs::canonicalize(dir.path().join("sub").join("..")).unwrap());
        let d = root_hash(&fs::canonicalize(
            PathBuf::from(format!("{}/", canonical.display())),
        )
        .unwrap());

        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_eq!(a, d);
    }

    #[test]
    fn hash_differs_for_distinct_roots() {
        let (a, a_canonical) = tmp_root();
        let (b, b_canonical) = tmp_root();
        assert_ne!(a_canonical, b_canonical);
        assert_ne!(root_hash(&a_canonical), root_hash(&b_canonical));
        drop(a);
        drop(b);
    }

    #[test]
    fn hash_is_sha256_hex() {
        let (_, canonical) = tmp_root();
        let hash = root_hash(&canonical);
        assert_eq!(hash.len(), 64);
        assert!(hash.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
    }

    #[test]
    fn handle_resolves_store_layout() {
        let (dir, canonical) = tmp_root();
        let data = dir.path().join("data");
        let handle = RootHandle::with_data_dir(&canonical, data.clone()).unwrap();

        assert_eq!(handle.root(), canonical.as_path());
        assert_eq!(handle.store_dir(), data.join(ROOTS_DIR_NAME).join(handle.hash()));
        assert_eq!(handle.db_path(), handle.store_dir().join(STORE_DB_FILE));
        assert_eq!(handle.root_pointer_path(), handle.store_dir().join(ROOT_POINTER_FILE));
        assert_eq!(handle.scratch_dir(), handle.store_dir().join(SCRATCH_DIR_NAME));
    }

    #[test]
    fn open_rejects_missing_root_and_plain_file() {
        let (dir, _) = tmp_root();
        let missing = dir.path().join("nope");
        let err = RootHandle::with_data_dir(&missing, dir.path().join("data")).unwrap_err();
        assert!(matches!(err, SubtexError::Canonicalize { .. }), "{err}");

        let file = dir.path().join("plain.txt");
        std::fs::write(&file, b"x").unwrap();
        let err = RootHandle::with_data_dir(&file, dir.path().join("data")).unwrap_err();
        assert!(err.to_string().contains("not a directory"), "{err}");
    }

    #[test]
    fn default_data_dir_honors_env_override() {
        // Only test in this process; no other test in this crate reads the env.
        // Edition 2024 makes env mutation unsafe (process-global effect).
        unsafe { std::env::set_var("SUBTEX_DATA_DIR", "/tmp/subtex-test-data") };
        assert_eq!(default_data_dir().unwrap(), PathBuf::from("/tmp/subtex-test-data"));
        unsafe { std::env::remove_var("SUBTEX_DATA_DIR") };
    }

    #[test]
    fn resolve_under_root_normalizes_and_rejects_escapes() {
        let (dir, canonical) = tmp_root();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("inside.md"), b"ok").unwrap();
        fs::write(dir.path().join("sub").join("a.md"), b"ok").unwrap();

        assert_eq!(
            resolve_under_root(&canonical, "inside.md").unwrap(),
            canonical.join("inside.md")
        );
        assert_eq!(
            resolve_under_root(&canonical, "./sub/../inside.md").unwrap(),
            canonical.join("inside.md")
        );
        assert_eq!(
            rel_under_root(&canonical, "sub/../sub/a.md").unwrap(),
            "sub/a.md"
        );

        assert!(matches!(
            resolve_under_root(&canonical, "../outside.md"),
            Err(SubtexError::PathOutsideRoot { .. })
        ));
        assert!(matches!(
            resolve_under_root(&canonical, "/etc/passwd"),
            Err(SubtexError::PathOutsideRoot { .. })
        ));
        assert!(matches!(
            resolve_under_root(&canonical, ""),
            Err(SubtexError::PathOutsideRoot { .. })
        ));
        assert!(matches!(
            resolve_under_root(&canonical, "sub/../../outside.md"),
            Err(SubtexError::PathOutsideRoot { .. })
        ));
    }
}
