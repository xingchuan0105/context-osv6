//! Inbox suggest/confirm (F6). Drop-point config and matching live here;
//! the global sqlite store persists the queue. Moves always stay inside an
//! attached root and never create a destination directory.

use crate::error::SubtexError;
use crate::resolve_under_root;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const INBOX_CONFIG_FILE: &str = "inbox.json";
pub const PROPOSE_AFTER_ACCEPTS: i64 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxRule {
    pub id: String,
    /// Lowercase extensions without a dot (`m4a`, `pdf`).
    #[serde(default)]
    pub exts: Vec<String>,
    #[serde(default)]
    pub name_contains: Option<String>,
    pub target_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InboxConfig {
    #[serde(default)]
    pub autofill: bool,
    #[serde(default)]
    pub drop_points: Vec<String>,
    #[serde(default)]
    pub rules: Vec<InboxRule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub path: PathBuf,
    pub drop_point: PathBuf,
    pub target_root: Option<PathBuf>,
    pub dest_name: String,
    pub confidence: Confidence,
    pub reason: String,
    pub rule_id: Option<String>,
}

impl InboxConfig {
    pub fn path(data_dir: &Path) -> PathBuf {
        data_dir.join(INBOX_CONFIG_FILE)
    }

    pub fn load(data_dir: &Path) -> Result<Self, SubtexError> {
        let path = Self::path(data_dir);
        if !path.is_file() {
            return Ok(Self::default());
        }
        let bytes = fs::read(&path)?;
        serde_json::from_slice(&bytes).map_err(|e| {
            SubtexError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        })
    }

    pub fn save(&self, data_dir: &Path) -> Result<(), SubtexError> {
        fs::create_dir_all(data_dir)?;
        let path = Self::path(data_dir);
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(self).map_err(|e| {
            SubtexError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        })?)?;
        fs::rename(tmp, path)?;
        Ok(())
    }

    pub fn add_drop_point(&mut self, dir: &Path) -> Result<(), SubtexError> {
        let canonical = fs::canonicalize(dir).map_err(|source| SubtexError::Canonicalize {
            path: dir.to_path_buf(),
            source,
        })?;
        if !canonical.is_dir() {
            return Err(SubtexError::RootNotFound(canonical));
        }
        let s = canonical.to_string_lossy().to_string();
        if !self.drop_points.iter().any(|p| p == &s) {
            self.drop_points.push(s);
        }
        Ok(())
    }

    pub fn remove_drop_point(&mut self, dir: &Path) {
        let s = dir.to_string_lossy();
        self.drop_points.retain(|p| p != s.as_ref());
        if let Ok(canonical) = fs::canonicalize(dir) {
            let c = canonical.to_string_lossy().to_string();
            self.drop_points.retain(|p| p != &c);
        }
    }
}

/// Immediate (non-recursive) files in a drop point. Hidden names and anything
/// already inside an attached root are skipped.
pub fn list_drop_point_files(
    drop_point: &Path,
    attached_roots: &[PathBuf],
) -> Result<Vec<PathBuf>, SubtexError> {
    let canonical = fs::canonicalize(drop_point).map_err(|source| SubtexError::Canonicalize {
        path: drop_point.to_path_buf(),
        source,
    })?;
    let mut files = Vec::new();
    let Ok(entries) = fs::read_dir(&canonical) else {
        return Ok(files);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        if !path.is_file() {
            continue;
        }
        if attached_roots.iter().any(|root| path.starts_with(root)) {
            continue;
        }
        files.push(path);
    }
    files.sort();
    Ok(files)
}

pub fn suggest(
    path: &Path,
    drop_point: &Path,
    rules: &[InboxRule],
    attached_roots: &[PathBuf],
) -> Suggestion {
    let dest_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let file_lower = dest_name.to_ascii_lowercase();

    for rule in rules {
        if !rule_matches(rule, &ext, &file_lower) {
            continue;
        }
        let Ok(target) = fs::canonicalize(&rule.target_root) else {
            continue;
        };
        if !attached_roots.iter().any(|root| root == &target) {
            continue;
        }
        return Suggestion {
            path: path.to_path_buf(),
            drop_point: drop_point.to_path_buf(),
            target_root: Some(target),
            dest_name,
            confidence: Confidence::High,
            reason: format!("rule {} matched extension/name", rule.id),
            rule_id: Some(rule.id.clone()),
        };
    }

    if let Some(target) = unique_name_match(&file_lower, attached_roots) {
        let root_name = target
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        return Suggestion {
            path: path.to_path_buf(),
            drop_point: drop_point.to_path_buf(),
            target_root: Some(target),
            dest_name,
            confidence: Confidence::Medium,
            reason: format!("filename contains attached root name {root_name}"),
            rule_id: None,
        };
    }

    Suggestion {
        path: path.to_path_buf(),
        drop_point: drop_point.to_path_buf(),
        target_root: None,
        dest_name,
        confidence: Confidence::Low,
        reason: "no attached root mapped".to_string(),
        rule_id: None,
    }
}

fn rule_matches(rule: &InboxRule, ext: &str, file_lower: &str) -> bool {
    let ext_ok = rule.exts.is_empty()
        || rule
            .exts
            .iter()
            .any(|e| e.trim_start_matches('.').eq_ignore_ascii_case(ext));
    if !ext_ok {
        return false;
    }
    match rule.name_contains.as_deref() {
        None | Some("") => true,
        Some(needle) => file_lower.contains(&needle.to_ascii_lowercase()),
    }
}

fn unique_name_match(file_lower: &str, attached_roots: &[PathBuf]) -> Option<PathBuf> {
    let mut hits: Vec<PathBuf> = Vec::new();
    for root in attached_roots {
        let Some(name) = root.file_name() else {
            continue;
        };
        let name = name.to_string_lossy().to_ascii_lowercase();
        if name.chars().count() < 3 {
            continue;
        }
        if file_lower.contains(&name) {
            hits.push(root.clone());
        }
    }
    if hits.len() == 1 {
        hits.pop()
    } else {
        None
    }
}

pub fn accept_pattern(ext: &str, target_root: &Path) -> String {
    format!(
        "{}|{}",
        ext.to_ascii_lowercase(),
        target_root.to_string_lossy()
    )
}

/// Rename `src` onto `target_root / dest_name`. The destination directory
/// must already exist (the attached root); a colliding name gets a suffix.
pub fn move_into_root(
    src: &Path,
    target_root: &Path,
    dest_name: &str,
) -> Result<PathBuf, SubtexError> {
    let dest_name = Path::new(dest_name)
        .file_name()
        .ok_or_else(|| SubtexError::PathOutsideRoot {
            root: target_root.to_path_buf(),
            path: PathBuf::from(dest_name),
        })?;
    let dest_rel = dest_name.to_string_lossy();
    let mut dest = resolve_under_root(target_root, dest_rel.as_ref())?;
    if dest.exists() {
        let stem = dest
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());
        let ext = dest
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        let tagged = format!("{stem}-{}{ext}", now_stamp());
        dest = resolve_under_root(target_root, &tagged)?;
    }
    fs::rename(src, &dest).or_else(|e| {
        if e.kind() == std::io::ErrorKind::CrossesDevices {
            fs::copy(src, &dest)?;
            fs::remove_file(src)?;
            Ok(())
        } else {
            Err(e)
        }
    })?;
    Ok(dest)
}

pub fn undo_move(src: &Path, dest: &Path) -> Result<(), SubtexError> {
    if dest.is_file() && !src.exists() {
        if let Some(parent) = src.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(dest, src)?;
    }
    Ok(())
}

fn now_stamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "dup".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::TempDir::new().unwrap();
        let canonical = fs::canonicalize(dir.path()).unwrap();
        (dir, canonical)
    }

    #[test]
    fn rule_match_is_high_when_root_is_attached() {
        let (_drop, drop_c) = tmp();
        let (_root, root_c) = tmp();
        let file = drop_c.join("clip.m4a");
        fs::write(&file, b"x").unwrap();
        let rule = InboxRule {
            id: "audio".into(),
            exts: vec!["m4a".into()],
            name_contains: None,
            target_root: root_c.display().to_string(),
        };
        let s = suggest(&file, &drop_c, &[rule], &[root_c.clone()]);
        assert_eq!(s.confidence, Confidence::High);
        assert_eq!(s.target_root.as_deref(), Some(root_c.as_path()));
        assert_eq!(s.rule_id.as_deref(), Some("audio"));
    }

    #[test]
    fn filename_unique_root_name_is_medium() {
        let (_drop, drop_c) = tmp();
        let root_dir = tempfile::TempDir::new().unwrap();
        let named = root_dir.path().join("asr-filetrans");
        fs::create_dir(&named).unwrap();
        let root_c = fs::canonicalize(&named).unwrap();
        let file = drop_c.join("asr-filetrans-notes.md");
        fs::write(&file, b"x").unwrap();
        let s = suggest(&file, &drop_c, &[], &[root_c.clone()]);
        assert_eq!(s.confidence, Confidence::Medium);
        assert_eq!(s.target_root.as_deref(), Some(root_c.as_path()));
    }

    #[test]
    fn unmapped_file_is_low() {
        let (_drop, drop_c) = tmp();
        let (_root, root_c) = tmp();
        let file = drop_c.join("random.bin");
        fs::write(&file, b"x").unwrap();
        let s = suggest(&file, &drop_c, &[], &[root_c]);
        assert_eq!(s.confidence, Confidence::Low);
        assert!(s.target_root.is_none());
    }

    #[test]
    fn move_and_undo_roundtrip() {
        let (_drop, drop_c) = tmp();
        let (_root, root_c) = tmp();
        let src = drop_c.join("note.md");
        fs::write(&src, b"hello").unwrap();
        let dest = move_into_root(&src, &root_c, "note.md").unwrap();
        assert!(!src.exists());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "hello");
        undo_move(&src, &dest).unwrap();
        assert_eq!(fs::read_to_string(&src).unwrap(), "hello");
        assert!(!dest.exists());
    }

    #[test]
    fn list_skips_hidden_and_files_already_in_a_root() {
        let (_drop, drop_c) = tmp();
        let (_root, root_c) = tmp();
        fs::write(drop_c.join("keep.md"), b"x").unwrap();
        fs::write(drop_c.join(".hidden"), b"x").unwrap();
        fs::write(root_c.join("inside.md"), b"x").unwrap();
        let files = list_drop_point_files(&drop_c, &[root_c]).unwrap();
        assert_eq!(files, vec![drop_c.join("keep.md")]);
    }
}
