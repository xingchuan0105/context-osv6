//! Test fingerprinting — compute SHA-256 source hashes for E2E tests.

use sha2::{Digest, Sha256};
use std::path::Path;

use crate::models::{TestFingerprint, TestStatus};

/// Compute SHA-256 hex hash of a file's contents.
pub fn compute_source_hash(source_path: &Path) -> String {
    let content = std::fs::read_to_string(source_path).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Check if two fingerprint hashes match (source code unchanged).
/// Returns false if either is empty.
pub fn fingerprint_match(a: &str, b: &str) -> bool {
    a == b && !a.is_empty()
}

/// Build a fingerprint for a known E2E test by name.
/// Maps test names to their source files and computes the source hash.
pub fn fingerprint_for_test(test_name: &str) -> Option<TestFingerprint> {
    let (source_file, strategy, format_skill) = match test_name {
        // e2e_chat.rs
        "chat_simple_conversation_state_machine" => {
            ("crates/app/tests/e2e_chat.rs", "chat", None)
        }
        "chat_with_tool_call_state_machine" => {
            ("crates/app/tests/e2e_chat.rs", "chat", None)
        }
        "chat_ppt_format_skill_injected" => {
            ("crates/app/tests/e2e_chat.rs", "chat", Some("ppt"))
        }
        // e2e_rag.rs
        "rag_single_pass_sufficient_state_machine" => {
            ("crates/app/tests/e2e_rag.rs", "rag", None)
        }
        "rag_replan_insufficient_state_machine" => {
            ("crates/app/tests/e2e_rag.rs", "rag", None)
        }
        "rag_html_format_skill_injected" => {
            ("crates/app/tests/e2e_rag.rs", "rag", Some("html"))
        }
        // e2e_search.rs
        "search_single_pass_state_machine" => {
            ("crates/app/tests/e2e_search.rs", "search", None)
        }
        "search_vertical_escalation_state_machine" => {
            ("crates/app/tests/e2e_search.rs", "search", None)
        }
        // e2e_format_output.rs
        "format_output_golden_scenarios" => {
            ("crates/app/tests/e2e_format_output.rs", "format", None)
        }
        // e2e_ingestion_answer.rs
        "ingestion_answer_pipeline" => {
            ("crates/app/tests/e2e_ingestion_answer.rs", "ingestion", None)
        }
        _ => return None,
    };

    let path = Path::new(source_file);
    let sha256 = compute_source_hash(path);

    Some(TestFingerprint {
        test_name: test_name.to_string(),
        strategy: strategy.to_string(),
        format_skill: format_skill.map(String::from),
        status: TestStatus::Passed,
        duration_ms: 0,
        token_usage: None,
        retrieval_hits: None,
        llm_call_count: 0,
        tool_call_count: 0,
        error_message: None,
        failure_kind: None,
        sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_compute_source_hash_is_deterministic() {
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        let content = b"hello world, this is a test file for fingerprinting.";
        tmpfile.write_all(content).unwrap();
        tmpfile.flush().unwrap();

        let path = tmpfile.path();
        let hash1 = compute_source_hash(path);
        let hash2 = compute_source_hash(path);

        assert_eq!(hash1, hash2, "hash should be deterministic");
        assert_eq!(hash1.len(), 64, "SHA-256 hex string should be 64 chars");
        assert!(!hash1.is_empty(), "hash should not be empty");
    }

    #[test]
    fn test_fingerprint_match() {
        assert!(fingerprint_match("abc123", "abc123"), "equal non-empty strings should match");
        assert!(!fingerprint_match("abc123", "def456"), "different strings should not match");
        assert!(!fingerprint_match("", ""), "empty strings should not match");
        assert!(!fingerprint_match("abc123", ""), "empty vs non-empty should not match");
        assert!(!fingerprint_match("", "abc123"), "non-empty vs empty should not match");
    }

    #[test]
    fn test_fingerprint_for_test_finds_known_tests() {
        let fp = fingerprint_for_test("chat_simple_conversation_state_machine");
        assert!(fp.is_some(), "known test should be found");

        let fp = fp.unwrap();
        assert_eq!(fp.test_name, "chat_simple_conversation_state_machine");
        assert_eq!(fp.strategy, "chat");
        assert!(fp.format_skill.is_none());
        assert_eq!(fp.sha256.len(), 64, "sha256 should be 64 hex chars");
        assert!(!fp.sha256.is_empty(), "sha256 should not be empty");
    }

    #[test]
    fn test_fingerprint_for_test_returns_none_for_unknown() {
        let fp = fingerprint_for_test("totally_unknown_test_name");
        assert!(fp.is_none(), "unknown test should return None");
    }

    #[test]
    fn test_fingerprint_for_test_maps_format_skills() {
        let fp_chat = fingerprint_for_test("chat_ppt_format_skill_injected").unwrap();
        assert_eq!(fp_chat.format_skill.as_deref(), Some("ppt"));

        let fp_rag = fingerprint_for_test("rag_html_format_skill_injected").unwrap();
        assert_eq!(fp_rag.format_skill.as_deref(), Some("html"));
    }

    #[test]
    fn test_compute_source_hash_empty_file() {
        let tmpfile = tempfile::NamedTempFile::new().unwrap();
        let hash = compute_source_hash(tmpfile.path());
        assert_eq!(hash.len(), 64, "empty file should still produce 64-char hash");
        // SHA-256 of empty string
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
