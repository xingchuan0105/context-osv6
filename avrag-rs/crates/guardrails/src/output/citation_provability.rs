//! Citation provability guard.
//!
//! Verifies that every `[citation:N]` marker in the synthesized response
//! corresponds to a real chunk that was actually retrieved.

use common::{GuardResult, RiskLevel};
use lazy_static::lazy_static;
use regex::Regex;
use uuid::Uuid;

lazy_static! {
    /// Matches `[citation:N]` markers where N is a number
    static ref CITATION_RE: Regex = Regex::new(r"\[citation:(\d+)\]").unwrap();
}

/// Guard that validates citations are backed by real chunks.
#[derive(Debug, Clone)]
pub struct CitationProvabilityGuard;

impl CitationProvabilityGuard {
    pub fn new() -> Self {
        Self
    }

    /// Check that all citations in the response correspond to real chunks.
    ///
    /// - `citations`: the citation objects from the runtime
    /// - `chunk_ids`: IDs of chunks actually retrieved and used
    ///
    /// Returns a passing result if all citations are valid,
    /// or a blocking result if any citation index is out of bounds.
    pub fn check(
        &self,
        response: &str,
        _citations: &[common::Citation],
        chunk_ids: &[Uuid],
        trace_id: Option<String>,
    ) -> GuardResult {
        let citation_indices: Vec<u32> = CITATION_RE
            .captures_iter(response)
            .filter_map(|cap| cap.get(1)?.as_str().parse().ok())
            .collect();

        if citation_indices.is_empty() {
            return GuardResult::pass("output:citation_provability");
        }

        // Build a set of valid chunk indices
        let valid_range = 0..chunk_ids.len() as u32;
        let mut invalid = Vec::new();

        for idx in citation_indices {
            if !valid_range.contains(&idx) {
                invalid.push(idx);
            }
        }

        if invalid.is_empty() {
            GuardResult::pass("output:citation_provability")
        } else {
            GuardResult::block(
                "output:citation_provability",
                RiskLevel::High,
                format!(
                    "Response contains citations with invalid indices: {:?}",
                    invalid
                ),
                trace_id,
                None,
            )
        }
    }
}

impl Default for CitationProvabilityGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_citations_passed() {
        let guard = CitationProvabilityGuard::new();
        let chunk_ids = vec![Uuid::new_v4(), Uuid::new_v4()];
        let result = guard.check("Hello world", &[], &chunk_ids, None);
        assert!(result.passed);
    }

    #[test]
    fn test_valid_citation_passed() {
        let guard = CitationProvabilityGuard::new();
        let chunk_ids = vec![Uuid::new_v4(), Uuid::new_v4()];
        let citations = vec![
            common::Citation {
                citation_id: 1,
                doc_id: "doc1".into(),
                chunk_id: Some(chunk_ids[0].to_string()),
                doc_name: "Doc 1".into(),
                preview: None,
                content: None,
                score: 0.9,
                layer: None,
            },
            common::Citation {
                citation_id: 2,
                doc_id: "doc2".into(),
                chunk_id: Some(chunk_ids[1].to_string()),
                doc_name: "Doc 2".into(),
                preview: None,
                content: None,
                score: 0.8,
                layer: None,
            },
        ];
        // Citation indices in the response are 0-based, corresponding to chunk array positions
        let response = "Answer [citation:0] and more [citation:1] text";
        let result = guard.check(response, &citations, &chunk_ids, None);
        assert!(result.passed);
    }

    #[test]
    fn test_out_of_bounds_citation_blocked() {
        let guard = CitationProvabilityGuard::new();
        let chunk_ids = vec![Uuid::new_v4(), Uuid::new_v4()];
        let response = "Answer [citation:0] and more [citation:5] text";
        let result = guard.check(response, &[], &chunk_ids, None);
        assert!(!result.passed);
        assert_eq!(result.guard_type, "output:citation_provability");
    }

    #[test]
    fn test_duplicate_citations_passed() {
        let guard = CitationProvabilityGuard::new();
        let chunk_ids = vec![Uuid::new_v4(), Uuid::new_v4()];
        let response = "Answer [citation:0] and again [citation:0] text";
        let result = guard.check(response, &[], &chunk_ids, None);
        assert!(result.passed);
    }

    #[test]
    fn test_all_valid_citations_at_boundaries() {
        let guard = CitationProvabilityGuard::new();
        let chunk_ids = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        // Valid: 0, 1, 2 (all within 0..3)
        let response = "See [citation:0] and [citation:1] and [citation:2]";
        let result = guard.check(response, &[], &chunk_ids, None);
        assert!(result.passed);
    }

    #[test]
    fn test_boundary_out_of_bounds() {
        let guard = CitationProvabilityGuard::new();
        let chunk_ids = vec![Uuid::new_v4()]; // only 1 chunk
        // Citation index 1 is out of bounds (valid range is 0..1)
        let response = "See [citation:0] and [citation:1]";
        let result = guard.check(response, &[], &chunk_ids, None);
        assert!(!result.passed);
    }
}
