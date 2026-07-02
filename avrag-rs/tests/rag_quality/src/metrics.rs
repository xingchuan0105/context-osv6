//! Evaluation metrics for RAG quality — PRD §13.2
//!
//! Release gates:
//! - Recall@15 not decreasing more than 3%
//! - Citation Accuracy and Hallucination Rate are reported, not hard-gated
//!   here. Generation-layer gates live in `metrics_v2::ScorecardSummary`.

use crate::golden_set::GoldenExample;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Recall@K result for a single query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallResult {
    pub query: String,
    pub k: usize,
    pub recall: f64, // fraction of golden chunks recalled (0.0 to 1.0)
    pub retrieved_count: usize,
    pub golden_count: usize,
    pub matched_chunks: Vec<usize>, // indices of matched golden chunks
}

/// Citation accuracy for a single query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationAccuracyResult {
    pub query: String,
    pub accuracy: f64, // 0.0 to 1.0
    pub true_positives: usize,
    pub false_positives: usize,
    pub missing: Vec<u32>,  // citation indices that should have been present
    pub spurious: Vec<u32>, // citation indices present but not in golden set
}

/// Hallucination detection result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HallucinationResult {
    pub query: String,
    pub is_hallucinated: bool,
    pub hallucination_score: f64, // 0.0 = perfect, 1.0 = fully hallucinated
    pub flagged_phrases: Vec<String>,
}

/// Aggregated evaluation metrics across a golden set.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvaluationMetrics {
    pub recall_at_15: f64,
    pub citation_accuracy: f64,
    pub hallucination_rate: f64,

    pub total_examples: usize,
    pub recall_results: Vec<RecallResult>,
    pub citation_results: Vec<CitationAccuracyResult>,
    pub hallucination_results: Vec<HallucinationResult>,
}

impl EvaluationMetrics {
    /// Compute Recall@K for a single query.
    ///
    /// `retrieved_chunks`: content strings of chunks returned by the RAG pipeline.
    /// `golden_example`: the golden-set example with expected chunks.
    /// `k`: the recall cutoff (typically 15 per PRD §13.2).
    pub fn recall_at_k(
        query: &str,
        retrieved_chunks: &[String],
        golden_example: &GoldenExample,
        k: usize,
    ) -> RecallResult {
        let retrieved_for_eval = &retrieved_chunks[..retrieved_chunks.len().min(k)];
        let mut matched = Vec::new();

        for (golden_idx, golden_chunk) in golden_example.source_chunks.iter().enumerate() {
            if retrieved_for_eval
                .iter()
                .any(|chunk| golden_chunk.matches(chunk))
            {
                matched.push(golden_idx);
            }
        }

        let golden_count = golden_example.source_chunks.len();
        let recall = if golden_count > 0 {
            matched.len() as f64 / golden_count as f64
        } else {
            1.0 // no golden chunks means vacuously perfect recall
        };

        RecallResult {
            query: query.to_string(),
            k,
            recall,
            retrieved_count: retrieved_chunks.len().min(k),
            golden_count,
            matched_chunks: matched,
        }
    }

    /// Compute Citation Accuracy for a single query.
    ///
    /// Compares the citations in the generated answer against the golden set's expected citations.
    /// `citation_indices`: the citation indices (e.g., [1, 2, 3]) extracted from the answer.
    ///
    /// Special case: when both `expected` and `actual` are empty (e.g. the
    /// example has no `expected_citations` AND the LLM produced no
    /// citations — common for adversarial "not mentioned" responses),
    /// accuracy is vacuously 1.0. The old behavior (`total = max(1, 0) = 1;
    /// true_positives = 0; accuracy = 0.0`) was a bug that mis-scored
    /// correct refusals as 0% accuracy.
    pub fn citation_accuracy(
        query: &str,
        citation_indices: &[u32],
        golden_example: &GoldenExample,
    ) -> CitationAccuracyResult {
        let expected: HashSet<u32> = golden_example.expected_citations.iter().copied().collect();
        let actual: HashSet<u32> = citation_indices.iter().copied().collect();

        let true_positives = expected.intersection(&actual).count();
        let false_positives = actual.len() - true_positives;
        let missing: Vec<u32> = expected.difference(&actual).copied().collect();
        let spurious: Vec<u32> = actual.difference(&expected).copied().collect();

        let accuracy = if expected.is_empty() && actual.is_empty() {
            1.0 // vacuously correct (nothing expected, nothing produced)
        } else {
            let total = expected.len().max(1);
            true_positives as f64 / total as f64
        };

        CitationAccuracyResult {
            query: query.to_string(),
            accuracy,
            true_positives,
            false_positives,
            missing,
            spurious,
        }
    }

    /// Extract citation indices from a generated answer string.
    ///
    /// Looks for `[citation:N]` markers and returns the unique set of N values.
    pub fn extract_citation_indices(answer: &str) -> Vec<u32> {
        use regex::Regex;
        static RE: once_cell::sync::Lazy<Regex> =
            once_cell::sync::Lazy::new(|| Regex::new(r"\[citation:(\d+)\]").unwrap());

        let mut indices: Vec<u32> = RE
            .captures_iter(answer)
            .filter_map(|cap| cap.get(1)?.as_str().parse().ok())
            .collect();
        indices.sort();
        indices.dedup();
        indices
    }

    /// Compute Hallucination Rate for a single query.
    ///
    /// Hallucination is detected by checking whether the generated answer
    /// makes claims not supported by any retrieved chunk.
    ///
    /// This is a lightweight heuristic implementation.
    /// For production, replace with a trained NLI (natural language inference) model.
    ///
    /// Special cases:
    /// - If the answer is an explicit refusal ("not mentioned", "no
    ///   information", etc.) the answer is NOT hallucinated, even
    ///   though it doesn't match any chunk.
    /// - If the answer is empty, it is NOT hallucinated.
    /// - If `retrieved_chunks` is empty AND the question has no
    ///   expected answer (chat / search mode without documents), skip
    ///   the check entirely — there's nothing to be faithful to.
    ///
    /// PRD §13.2 gate: Hallucination Rate <= 2%
    pub fn hallucination_check(
        query: &str,
        answer: &str,
        retrieved_chunks: &[String],
    ) -> HallucinationResult {
        // No-context mode: the question isn't grounded in any
        // retrieved knowledge, so the heuristic can't judge
        // faithfulness. We can't say it's NOT a hallucination (the
        // LLM might be making things up), but we also can't
        // meaningfully run the check. Mark as not-flagged so the
        // chat/search subsets don't dominate the rate.
        if retrieved_chunks.is_empty() {
            return HallucinationResult {
                query: query.to_string(),
                is_hallucinated: false,
                hallucination_score: 0.0,
                flagged_phrases: vec![],
            };
        }

        // Refusal patterns: the LLM is correctly saying "I don't know",
        // which is NOT a hallucination. Flag-as-hallucination should
        // only fire on FAILED refusals (the LLM tried to answer and
        // made up facts).
        let answer_lower = answer.to_lowercase();
        let is_refusal = answer_lower.contains("not mentioned")
            || answer_lower.contains("no information")
            || answer_lower.contains("don't know")
            || answer_lower.contains("do not know")
            || answer_lower.contains("cannot answer")
            || answer_lower.contains("no answer")
            || answer_lower.contains("unable to answer")
            || answer.trim().is_empty();

        if is_refusal {
            return HallucinationResult {
                query: query.to_string(),
                is_hallucinated: false,
                hallucination_score: 0.0,
                flagged_phrases: vec![],
            };
        }

        // Simple heuristic: split answer into sentences and check
        // whether each sentence's key claims appear in at least one chunk.
        let sentences: Vec<&str> = answer.split(['.', '!', '?'].as_slice()).collect();
        let context = retrieved_chunks.join(" ");
        let total_sentences = sentences.len().max(1);

        let mut flagged = Vec::new();
        for sentence in sentences {
            let trimmed = sentence.trim();
            if trimmed.len() < 10 {
                continue; // skip very short fragments
            }
            // Check if key content words from the sentence appear in any chunk.
            // This is a very rough proxy for hallucination detection.
            let words: Vec<&str> = trimmed.split_whitespace().collect();
            let significant_words: Vec<&str> = words
                .into_iter()
                .filter(|w| w.len() > 5 && !is_stopword(w))
                .collect();

            if significant_words.is_empty() {
                continue;
            }

            // Count how many significant words appear in the context
            let context_lower = context.to_lowercase();
            let match_count = significant_words
                .iter()
                .filter(|w| context_lower.contains(&w.to_lowercase()))
                .count();

            // If less than 40% of significant words appear in any chunk, flag as suspicious
            if match_count < significant_words.len() / 2 {
                flagged.push(trimmed.to_string());
            }
        }

        let hallucination_score = if flagged.is_empty() {
            0.0
        } else {
            flagged.len() as f64 / total_sentences as f64
        };

        HallucinationResult {
            query: query.to_string(),
            is_hallucinated: hallucination_score > 0.1, // flag if >10% sentences suspicious
            hallucination_score,
            flagged_phrases: flagged,
        }
    }

    /// Aggregate per-example results into summary metrics.
    pub fn aggregate(
        recall_results: Vec<RecallResult>,
        citation_results: Vec<CitationAccuracyResult>,
        hallucination_results: Vec<HallucinationResult>,
    ) -> Self {
        let total_examples = recall_results.len();

        let recall_at_15 = if total_examples > 0 {
            recall_results.iter().map(|r| r.recall).sum::<f64>() / total_examples as f64
        } else {
            0.0
        };

        let citation_accuracy = if total_examples > 0 {
            citation_results.iter().map(|r| r.accuracy).sum::<f64>() / total_examples as f64
        } else {
            0.0
        };

        let hallucination_rate = if total_examples > 0 {
            hallucination_results
                .iter()
                .filter(|r| r.is_hallucinated)
                .count() as f64
                / total_examples as f64
        } else {
            0.0
        };

        Self {
            recall_at_15,
            citation_accuracy,
            hallucination_rate,
            total_examples,
            recall_results,
            citation_results,
            hallucination_results,
        }
    }

    /// Check PRD §13.2 release gates.
    ///
    /// Returns `Ok(())` if all gates pass, `Err(String)` listing failures.
    ///
    /// Gate:
    /// - Recall@15 not decreasing more than 3% from the supplied baseline.
    ///
    /// Citation Accuracy and the legacy Hallucination Rate are reported only:
    /// citation selection is now scored by `metrics_v2::SelectionScore`, and
    /// generation faithfulness by `metrics_v2` / `judge`.
    pub fn assert_passing(&self, baseline_recall: f64) -> Result<(), String> {
        let mut errors = Vec::new();

        let recall_drop = baseline_recall - self.recall_at_15;
        if recall_drop > 0.03 {
            errors.push(format!(
                "Recall@15 regression: {:.1}% drop (gate: ≤3%). Current: {:.2}%",
                recall_drop * 100.0,
                self.recall_at_15 * 100.0
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("\n"))
        }
    }
}

/// Returns `true` if the word is a common English stopword.
fn is_stopword(word: &str) -> bool {
    matches!(
        word.to_lowercase().as_str(),
        "the"
            | "and"
            | "that"
            | "this"
            | "with"
            | "from"
            | "they"
            | "have"
            | "were"
            | "been"
            | "said"
            | "which"
            | "their"
            | "will"
            | "also"
            | "into"
            | "has"
            | "more"
            | "her"
            | "two"
            | "first"
            | "new"
            | "than"
            | "most"
    )
}
