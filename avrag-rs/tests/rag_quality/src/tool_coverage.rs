//! Tool invocation coverage scoring for `golden_set_tools.json` probes.
//!
//! Unlike answer-quality metrics in `metrics_v2`, this module only checks whether
//! the agent's trace invoked the expected tool(s). Tool names follow runtime
//! `ToolResult.tool` values (e.g. `chunk_fetch` shim → `index_lookup`).

use crate::golden_set::GoldenExample;
use contracts::{ToolResult, ToolStatus};
use serde::{Deserialize, Serialize};

/// Ordered tool names from a chat response's `tool_results`.
pub fn extract_tool_trace(tool_results: &[ToolResult]) -> Vec<String> {
    tool_results
        .iter()
        .filter(|r| r.status == ToolStatus::Ok)
        .map(|r| r.tool.clone())
        .collect()
}

/// Returns `true` when `expected` appears in `actual` in order (not necessarily adjacent).
pub fn tool_sequence_matches(expected: &[String], actual: &[String]) -> bool {
    if expected.is_empty() {
        return true;
    }
    let mut i = 0;
    for tool in actual {
        if tool == &expected[i] {
            i += 1;
            if i == expected.len() {
                return true;
            }
        }
    }
    false
}

/// Returns `true` when `expected` appears at least once in `actual`.
pub fn tool_present(expected: &str, actual: &[String]) -> bool {
    actual.iter().any(|t| t == expected)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCoverageScore {
    pub query: String,
    pub description: String,
    pub actual_tools: Vec<String>,
    pub expected_tool: Option<String>,
    pub expected_tool_sequence: Option<Vec<String>>,
    pub requires_triplet_reingest: bool,
    /// Single-tool expectation met (vacuous `true` when no single-tool expectation).
    pub tool_hit: bool,
    /// Sequence expectation met (vacuous `true` when no sequence expectation).
    pub sequence_hit: bool,
    /// Overall pass for this probe.
    pub covered: bool,
    /// Probe declares tool expectations (at least one of expected_tool / expected_tool_sequence).
    pub has_expectation: bool,
}

impl ToolCoverageScore {
    pub fn score(example: &GoldenExample, actual_tools: &[String]) -> Self {
        let has_single = example.expected_tool.is_some();
        let has_sequence = example
            .expected_tool_sequence
            .as_ref()
            .is_some_and(|s| !s.is_empty());
        let has_expectation = has_single || has_sequence;

        let tool_hit = example
            .expected_tool
            .as_deref()
            .map(|t| tool_present(t, actual_tools))
            .unwrap_or(true);

        let sequence_hit = example
            .expected_tool_sequence
            .as_deref()
            .map(|seq| tool_sequence_matches(seq, actual_tools))
            .unwrap_or(true);

        let covered = if !has_expectation {
            false
        } else if has_single && has_sequence {
            tool_hit && sequence_hit
        } else if has_single {
            tool_hit
        } else {
            sequence_hit
        };

        Self {
            query: example.query.clone(),
            description: example.description.clone(),
            actual_tools: actual_tools.to_vec(),
            expected_tool: example.expected_tool.clone(),
            expected_tool_sequence: example.expected_tool_sequence.clone(),
            requires_triplet_reingest: example.requires_triplet_reingest,
            tool_hit,
            sequence_hit,
            covered,
            has_expectation,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolCoverageSummary {
    pub total: usize,
    pub with_expectations: usize,
    pub covered: usize,
    pub coverage_rate: f64,
    pub single_tool_total: usize,
    pub single_tool_hit: usize,
    pub single_tool_hit_rate: f64,
    pub sequence_total: usize,
    pub sequence_hit: usize,
    pub sequence_hit_rate: f64,
    pub triplet_reingest_pending: usize,
    pub triplet_reingest_covered: usize,
}

impl ToolCoverageSummary {
    pub fn from_scores(scores: &[ToolCoverageScore]) -> Self {
        let total = scores.len();
        let with_expectations: Vec<&ToolCoverageScore> =
            scores.iter().filter(|s| s.has_expectation).collect();
        let covered = with_expectations.iter().filter(|s| s.covered).count();

        let single: Vec<&ToolCoverageScore> = with_expectations
            .iter()
            .copied()
            .filter(|s| s.expected_tool.is_some())
            .collect();
        let single_hit = single.iter().filter(|s| s.tool_hit).count();

        let sequence: Vec<&ToolCoverageScore> = with_expectations
            .iter()
            .copied()
            .filter(|s| {
                s.expected_tool_sequence
                    .as_ref()
                    .is_some_and(|seq| !seq.is_empty())
            })
            .collect();
        let sequence_hit_count = sequence.iter().filter(|s| s.sequence_hit).count();

        let triplet: Vec<&ToolCoverageScore> = with_expectations
            .iter()
            .copied()
            .filter(|s| s.requires_triplet_reingest)
            .collect();
        let triplet_covered = triplet.iter().filter(|s| s.covered).count();

        let n_exp = with_expectations.len().max(1) as f64;
        let n_single = single.len().max(1) as f64;
        let n_seq = sequence.len().max(1) as f64;

        Self {
            total,
            with_expectations: with_expectations.len(),
            covered,
            coverage_rate: covered as f64 / n_exp,
            single_tool_total: single.len(),
            single_tool_hit: single_hit,
            single_tool_hit_rate: single_hit as f64 / n_single,
            sequence_total: sequence.len(),
            sequence_hit: sequence_hit_count,
            sequence_hit_rate: sequence_hit_count as f64 / n_seq,
            triplet_reingest_pending: triplet.len(),
            triplet_reingest_covered: triplet_covered,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::golden_set::{GoldenDifficulty, GoldenExample};

    fn example_with_tool(expected_tool: Option<&str>) -> GoldenExample {
        GoldenExample {
            query: "q".into(),
            expected_answer: "a".into(),
            source_chunks: vec![],
            expected_citations: vec![],
            mode: "rag".into(),
            description: "test".into(),
            is_adversarial: false,
            expected_should_answer: true,
            refusal_keywords: vec![],
            must_include: vec![],
            must_not_include: vec![],
            retrieval_hints: vec![],
            difficulty: GoldenDifficulty::Medium,
            relevance_grades: Default::default(),
            expected_tool: expected_tool.map(str::to_string),
            expected_tool_sequence: None,
            requires_triplet_reingest: false,
        }
    }

    fn example_with_sequence(seq: Vec<&str>) -> GoldenExample {
        let mut ex = example_with_tool(None);
        ex.expected_tool_sequence = Some(seq.into_iter().map(str::to_string).collect());
        ex
    }

    #[test]
    fn subsequence_allows_interleaved_tools() {
        let actual = vec![
            "dense_retrieval".into(),
            "doc_profile".into(),
            "index_lookup".into(),
        ];
        assert!(tool_sequence_matches(
            &["doc_profile".into(), "index_lookup".into()],
            &actual
        ));
    }

    #[test]
    fn subsequence_requires_order() {
        let actual = vec!["index_lookup".into(), "doc_profile".into()];
        assert!(!tool_sequence_matches(
            &["doc_profile".into(), "index_lookup".into()],
            &actual
        ));
    }

    #[test]
    fn single_tool_hit_and_miss() {
        let ex = example_with_tool(Some("doc_summary"));
        let hit = ToolCoverageScore::score(&ex, &["doc_summary".into()]);
        assert!(hit.covered);
        let miss = ToolCoverageScore::score(&ex, &["doc_profile".into()]);
        assert!(!miss.covered);
    }

    #[test]
    fn sequence_coverage_pass_and_fail() {
        let ex = example_with_sequence(vec!["doc_profile", "index_lookup"]);
        let pass = ToolCoverageScore::score(
            &ex,
            &[
                "doc_profile".into(),
                "index_lookup".into(),
            ],
        );
        assert!(pass.covered);
        let fail = ToolCoverageScore::score(&ex, &["doc_profile".into()]);
        assert!(!fail.covered);
    }

    #[test]
    fn summary_aggregates_rates() {
        let scores = vec![
            ToolCoverageScore::score(
                &example_with_tool(Some("doc_summary")),
                &["doc_summary".into()],
            ),
            ToolCoverageScore::score(
                &example_with_tool(Some("doc_profile")),
                &["dense_retrieval".into()],
            ),
            ToolCoverageScore::score(
                &example_with_sequence(vec!["doc_profile", "index_lookup"]),
                &["doc_profile".into(), "index_lookup".into()],
            ),
        ];
        let summary = ToolCoverageSummary::from_scores(&scores);
        assert_eq!(summary.with_expectations, 3);
        assert_eq!(summary.covered, 2);
        assert!((summary.coverage_rate - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(summary.single_tool_total, 2);
        assert_eq!(summary.single_tool_hit, 1);
        assert_eq!(summary.sequence_total, 1);
        assert_eq!(summary.sequence_hit, 1);
    }

    #[test]
    fn extract_tool_trace_skips_errors() {
        let results = vec![
            ToolResult {
                tool: "doc_profile".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: None,
                trace: None,
            },
            ToolResult {
                tool: "doc_summary".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Error,
                data: None,
                trace: None,
            },
            ToolResult {
                tool: "index_lookup".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: None,
                trace: None,
            },
        ];
        assert_eq!(
            extract_tool_trace(&results),
            vec!["doc_profile", "index_lookup"]
        );
    }
}
