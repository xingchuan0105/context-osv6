//! Per-agent-run workspace for SaC `save`/`load` (A7).
//!
//! Host-side store (not a real sandbox mount): survives across code blocks
//! because each block is a fresh Python process. Keys are relative path-like
//! names; `..` / absolute / null are rejected.

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::Value;

/// In-memory session filesystem shared across codegen blocks of one agent run.
#[derive(Debug, Default)]
pub struct SessionFs {
    files: Mutex<HashMap<String, Value>>,
}

impl SessionFs {
    pub fn new() -> Self {
        Self::default()
    }

    /// Normalize and validate a relative path key.
    pub fn normalize_key(path: &str) -> Result<String, String> {
        let p = path.trim();
        if p.is_empty() {
            return Err("path is required".into());
        }
        if p.contains('\0') {
            return Err("path must not contain NUL".into());
        }
        if p.starts_with('/') || p.starts_with('\\') {
            return Err("path must be relative (no leading slash)".into());
        }
        if p.contains("..") {
            return Err("path must not contain '..'".into());
        }
        // Reject Windows drive letters.
        if p.len() >= 2 && p.as_bytes()[1] == b':' {
            return Err("path must not be an absolute Windows path".into());
        }
        Ok(p.to_string())
    }

    pub fn save(&self, path: &str, data: Value) -> Result<(), String> {
        let key = Self::normalize_key(path)?;
        self.files
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, data);
        Ok(())
    }

    pub fn load(&self, path: &str) -> Result<Value, String> {
        let key = Self::normalize_key(path)?;
        self.files
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("file not found: {key}"))
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.files
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn save_load_roundtrip() {
        let fs = SessionFs::new();
        fs.save("cands.json", json!([{"id": 1}])).unwrap();
        let v = fs.load("cands.json").unwrap();
        assert_eq!(v, json!([{"id": 1}]));
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(SessionFs::normalize_key("../etc/passwd").is_err());
        assert!(SessionFs::normalize_key("/abs").is_err());
        assert!(SessionFs::normalize_key("").is_err());
        assert!(SessionFs::normalize_key("a/../b").is_err());
    }

    #[test]
    fn missing_load_errors() {
        let fs = SessionFs::new();
        assert!(fs.load("missing.json").is_err());
    }
}
