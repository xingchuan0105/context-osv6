//! Judge response cache (design §4.4): identical judge calls across runs reuse
//! the stored raw response instead of re-billing the API.
//!
//! Key scheme: FNV-1a 64-bit over length-prefixed key material (everything that
//! changes the prompt: judge model, `SCHEMA_VERSION`, and every `JudgeInput`
//! field). FNV-1a is std-only and deterministic across builds — `DefaultHasher`
//! is explicitly not (its output may change between toolchain versions). The
//! full key material is stored inside each cache file and verified on read, so
//! a hash collision costs one recomputation, never a wrong hit.
//!
//! Only successful (parseable) raw responses are stored — errors are never
//! cached. Files live in a directory shared across run ids
//! (`e2e_output/rag_eval_v2/cache/{key}.json`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::artifact::JudgeInput;
use super::judge_prompt::SCHEMA_VERSION;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Everything that must match for a cached judge response to be valid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct KeyMaterial {
    model: String,
    schema_version: String,
    question: String,
    reference_answer: String,
    expected_should_answer: bool,
    model_answer: String,
    context_source: String,
    cited_context: Vec<String>,
    rubric_notes: Option<String>,
}

fn key_material(model: &str, input: &JudgeInput) -> KeyMaterial {
    KeyMaterial {
        model: model.to_string(),
        schema_version: SCHEMA_VERSION.to_string(),
        question: input.question.clone(),
        reference_answer: input.reference_answer.clone(),
        expected_should_answer: input.expected_should_answer,
        model_answer: input.model_answer.clone(),
        context_source: input.context_source.as_str().to_string(),
        cited_context: input.cited_context.clone(),
        rubric_notes: input.rubric_notes.clone(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheFile {
    key_material: KeyMaterial,
    raw_response: String,
}

/// FNV-1a 64-bit over length-prefixed parts (length prefix prevents
/// `"ab","c"` vs `"a","bc"` ambiguity).
fn fnv1a64(parts: &[&str]) -> u64 {
    let mut h = FNV_OFFSET_BASIS;
    for part in parts {
        for b in (part.len() as u64).to_le_bytes() {
            h = (h ^ b as u64).wrapping_mul(FNV_PRIME);
        }
        for b in part.as_bytes() {
            h = (h ^ *b as u64).wrapping_mul(FNV_PRIME);
        }
    }
    h
}

/// On-disk judge response cache. All I/O failures degrade to miss / no-store —
/// the cache is an optimization, never a correctness dependency.
#[derive(Debug, Clone)]
pub struct JudgeCache {
    dir: PathBuf,
}

impl JudgeCache {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Hex cache key for this exact judge call.
    pub fn key(model: &str, input: &JudgeInput) -> String {
        let m = key_material(model, input);
        let mut parts: Vec<&str> = vec![
            &m.model,
            &m.schema_version,
            &m.question,
            &m.reference_answer,
            if m.expected_should_answer { "1" } else { "0" },
            &m.model_answer,
            &m.context_source,
        ];
        parts.extend(m.cited_context.iter().map(String::as_str));
        parts.push(m.rubric_notes.as_deref().unwrap_or(""));
        format!("{:016x}", fnv1a64(&parts))
    }

    /// Verified load: returns the cached raw judge response iff the file's
    /// stored key material matches this exact call. Missing file, corrupt
    /// JSON, or key mismatch are all a plain miss.
    pub fn load(&self, key: &str, model: &str, input: &JudgeInput) -> Option<String> {
        let raw = std::fs::read_to_string(self.dir.join(format!("{key}.json"))).ok()?;
        let file: CacheFile = serde_json::from_str(&raw).ok()?;
        if file.key_material == key_material(model, input) {
            Some(file.raw_response)
        } else {
            None
        }
    }

    /// Store a successful raw response. Never call with error output.
    pub fn store(&self, key: &str, model: &str, input: &JudgeInput, raw_response: &str) {
        let file = CacheFile {
            key_material: key_material(model, input),
            raw_response: raw_response.to_string(),
        };
        let Ok(json) = serde_json::to_string_pretty(&file) else {
            return;
        };
        if std::fs::create_dir_all(&self.dir).is_err() {
            return;
        }
        let _ = std::fs::write(self.dir.join(format!("{key}.json")), json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval_v2::artifact::ContextSource;

    fn input(answer: &str) -> JudgeInput {
        JudgeInput {
            question: "Y公司哪一年在大连建厂？".to_string(),
            reference_answer: "Y公司2019年在大连建厂。".to_string(),
            expected_should_answer: true,
            model_answer: answer.to_string(),
            cited_context: vec!["Y冷冻设备公司2019年于大连市投资建厂".to_string()],
            context_source: ContextSource::Cited,
            rubric_notes: None,
        }
    }

    fn temp_cache_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rag_eval_v2_cache_test_{}_{}",
            std::process::id(),
            tag
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn store_load_roundtrip() {
        let dir = temp_cache_dir("roundtrip");
        let cache = JudgeCache::new(&dir);
        let input = input("2019 年在大连建厂");
        let key = JudgeCache::key("deepseek-v4-flash", &input);
        assert!(cache.load(&key, "deepseek-v4-flash", &input).is_none());

        cache.store(&key, "deepseek-v4-flash", &input, "{\"schema_version\":...}");
        let got = cache
            .load(&key, "deepseek-v4-flash", &input)
            .expect("verified hit");
        assert_eq!(got, "{\"schema_version\":...}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_misses_when_key_material_differs() {
        let dir = temp_cache_dir("verify");
        let cache = JudgeCache::new(&dir);
        let stored = input("answer A");
        let key = JudgeCache::key("deepseek-v4-flash", &stored);
        cache.store(&key, "deepseek-v4-flash", &stored, "raw");

        // Same key (filename) but a different call context → verification miss.
        let other = input("answer B");
        assert!(cache.load(&key, "deepseek-v4-flash", &other).is_none());
        // Different judge model → miss.
        assert!(cache.load(&key, "deepseek-v4-pro", &stored).is_none());
        // Corrupt file → miss.
        std::fs::write(dir.join(format!("{key}.json")), "not json").unwrap();
        assert!(cache.load(&key, "deepseek-v4-flash", &stored).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn key_is_stable_and_sensitive_to_inputs() {
        let a = JudgeCache::key("m", &input("x"));
        assert_eq!(a, JudgeCache::key("m", &input("x")));
        assert_ne!(a, JudgeCache::key("m", &input("y")));
        assert_ne!(a, JudgeCache::key("m2", &input("x")));
        // Length prefixing: no concatenation ambiguity.
        let mut i1 = input("x");
        i1.cited_context = vec!["ab".to_string(), "c".to_string()];
        let mut i2 = input("x");
        i2.cited_context = vec!["a".to_string(), "bc".to_string()];
        assert_ne!(JudgeCache::key("m", &i1), JudgeCache::key("m", &i2));
    }
}
