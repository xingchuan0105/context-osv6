//! Deterministic retrieval/selection metrics for eval v2 (Layer A).
//!
//! Trimmed copy of `metrics_v2::score_retrieval` / `score_selection`: pure
//! numbers, no label logic (v2 labels are score-driven and land with
//! aggregation in a later slice). The graded variants (`relevance_grades`,
//! ADR-0011) are kept verbatim: `source_chunks` count as grade 3 (critical)
//! and `relevance_grades` adds partial-credit evidence; with empty grades they
//! reduce to the binary metrics.

use crate::golden_set::{ChunkMatch, GoldenExample};
use crate::harness_extract::{CitedChunks, RetrievedChunks};
use serde::{Deserialize, Serialize};

/// Match each golden `source_chunks[i]` against a list of chunk contents.
/// Returns the indices of golden chunks that found a match.
fn matched_golden_indices(contents: &[String], example: &GoldenExample) -> Vec<usize> {
    example
        .source_chunks
        .iter()
        .enumerate()
        .filter(|(_, g)| contents.iter().any(|c| g.matches(c)))
        .map(|(i, _)| i)
        .collect()
}

// ---------------------------------------------------------------------------
// Retrieval layer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalScoreV2 {
    pub query: String,
    pub k: usize,
    /// Full-stream recall: fraction of golden evidence found ANYWHERE in the
    /// merged cross-round deduped stream. The multi-round agent surfaces gold
    /// in later ReAct rounds that a top-k truncation would hide, so this is
    /// the primary number — RETRIEVAL_MISS and the suite means use it.
    pub recall: f64,
    pub hit: bool,
    /// Top-k view of the same stream (single-shot ranking diagnostic; k=15 at
    /// the runner call site). Additive; pre-field artifacts deserialize with 0.
    #[serde(default)]
    pub recall_at_k: f64,
    #[serde(default)]
    pub hit_at_k: bool,
    /// MRR over the merged stream (first-hit rank across all rounds).
    pub mrr: f64,
    /// nDCG over the merged stream (binary relevance; IDCG spans
    /// `min(golden_count, stream_len)` ideal positions).
    pub ndcg: f64,
    /// Graded recall: weighted fraction of evidence-grade mass found in the
    /// merged stream (`source_chunks` = grade 3, `relevance_grades` = 1..3
    /// partial credit). Reduces to binary `recall` when grades are empty.
    pub graded_recall: f64,
    /// Graded nDCG over the merged stream: linear gain = max matched evidence
    /// grade at each rank. Reduces to binary `ndcg` when grades are empty.
    pub graded_ndcg: f64,
    /// Total chunks in the merged deduped stream (across all rounds).
    pub retrieved_count: usize,
    pub golden_count: usize,
    pub matched_golden: Vec<usize>,
    /// Rank (0-indexed, first-seen order in the merged stream) of each matched
    /// golden chunk's first hit, parallel to `matched_golden`.
    pub first_hit_ranks: Vec<usize>,
}

/// Score the retrieval layer. `retrieved` is the deduped first-seen-ordered
/// chunk list from `extract_retrieved_chunks` (merged across all loop rounds).
/// Primary recall/hit are computed over the FULL stream; the top-k view is
/// reported separately as `recall_at_k`/`hit_at_k`.
pub fn score_retrieval(
    retrieved: &RetrievedChunks,
    example: &GoldenExample,
    k: usize,
) -> RetrievalScoreV2 {
    let contents: Vec<String> = retrieved.chunks.iter().map(|c| c.content.clone()).collect();
    let golden_count = example.source_chunks.len();

    // Match each golden chunk against the FULL merged stream (multi-round
    // semantics: gold from any ReAct round counts as retrieved).
    let mut matched = Vec::new();
    let mut first_hit_ranks = Vec::new();
    for (gi, g) in example.source_chunks.iter().enumerate() {
        if let Some(rank) = contents.iter().position(|c| g.matches(c)) {
            matched.push(gi);
            first_hit_ranks.push(rank);
        }
    }

    let recall = if golden_count > 0 {
        matched.len() as f64 / golden_count as f64
    } else {
        1.0
    };
    let hit = !matched.is_empty();

    // Top-k view (diagnostic for single-shot ranking quality).
    let topk: Vec<String> = contents.iter().take(k).cloned().collect();
    let mut matched_at_k = 0usize;
    for g in example.source_chunks.iter() {
        if topk.iter().any(|c| g.matches(c)) {
            matched_at_k += 1;
        }
    }
    let recall_at_k = if golden_count > 0 {
        matched_at_k as f64 / golden_count as f64
    } else {
        1.0
    };
    let hit_at_k = matched_at_k > 0;

    let mrr = first_hit_ranks
        .first()
        .map(|&r| 1.0 / (r as f64 + 1.0))
        .unwrap_or(0.0);

    // nDCG over the merged stream with binary relevance (matched golden =
    // relevant). DCG sums 1/log2(rank+2) over first-hit positions; IDCG is the
    // ideal ordering over min(golden_count, stream_len) positions.
    let ndcg = if golden_count == 0 || first_hit_ranks.is_empty() {
        if golden_count == 0 { 1.0 } else { 0.0 }
    } else {
        let dcg: f64 = first_hit_ranks
            .iter()
            .map(|&r| 1.0 / ((r as f64 + 2.0).log2()))
            .sum();
        let ideal_relevant = golden_count.min(contents.len());
        let idcg: f64 = (0..ideal_relevant)
            .map(|i| 1.0 / ((i as f64 + 2.0).log2()))
            .sum();
        if idcg > 0.0 { dcg / idcg } else { 0.0 }
    };

    // Graded relevance: source_chunks = grade 3 (critical); relevance_grades
    // maps a content-signature substring to a finer grade for partial-credit
    // evidence. A retrieved chunk's grade is the max grade among evidence
    // units it matches. With empty relevance_grades, graded metrics reduce to
    // the binary ones.
    const SOURCE_GRADE: u8 = 3;
    let graded_evidence: Vec<(ChunkMatch, u8)> = example
        .relevance_grades
        .iter()
        .map(|(sig, g)| (ChunkMatch::Substring { text: sig.clone() }, *g))
        .collect();
    let total_grade_mass: u32 = (example.source_chunks.len() as u32 * SOURCE_GRADE as u32)
        + graded_evidence.iter().map(|(_, g)| *g as u32).sum::<u32>();
    let mut found_source = vec![false; example.source_chunks.len()];
    let mut found_graded = vec![false; graded_evidence.len()];
    let mut rank_grades: Vec<u8> = Vec::with_capacity(contents.len());
    for c in &contents {
        let mut g: u8 = 0;
        for (i, sc) in example.source_chunks.iter().enumerate() {
            if sc.matches(c) {
                found_source[i] = true;
                g = g.max(SOURCE_GRADE);
            }
        }
        for (i, (m, mg)) in graded_evidence.iter().enumerate() {
            if m.matches(c) {
                found_graded[i] = true;
                g = g.max(*mg);
            }
        }
        rank_grades.push(g);
    }
    let found_grade_mass: u32 =
        found_source.iter().filter(|f| **f).count() as u32 * SOURCE_GRADE as u32
            + graded_evidence
                .iter()
                .zip(found_graded.iter())
                .filter(|(_, f)| **f)
                .map(|((_, g), _)| *g as u32)
                .sum::<u32>();
    let graded_recall = if total_grade_mass > 0 {
        found_grade_mass as f64 / total_grade_mass as f64
    } else {
        1.0
    };
    let graded_ndcg = if total_grade_mass == 0 {
        1.0
    } else {
        let gdcg: f64 = rank_grades
            .iter()
            .enumerate()
            .map(|(r, &g)| g as f64 / ((r as f64 + 2.0).log2()))
            .sum();
        let mut ideal: Vec<u8> = vec![SOURCE_GRADE; example.source_chunks.len()];
        ideal.extend(graded_evidence.iter().map(|(_, g)| *g));
        ideal.sort_by(|a, b| b.cmp(a));
        let gidcg: f64 = ideal
            .iter()
            .take(contents.len())
            .enumerate()
            .map(|(i, &g)| g as f64 / ((i as f64 + 2.0).log2()))
            .sum();
        if gidcg > 0.0 { gdcg / gidcg } else { 0.0 }
    };

    RetrievalScoreV2 {
        query: example.query.clone(),
        k,
        recall,
        hit,
        recall_at_k,
        hit_at_k,
        mrr,
        ndcg,
        graded_recall,
        graded_ndcg,
        retrieved_count: retrieved.len(),
        golden_count,
        matched_golden: matched,
        first_hit_ranks,
    }
}

// ---------------------------------------------------------------------------
// Selection layer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionScoreV2 {
    pub query: String,
    /// Fraction of cited chunks that match a golden chunk.
    pub precision: f64,
    /// Fraction of golden chunks that appear among the cited chunks.
    pub recall: f64,
    pub cited_count: usize,
    pub golden_count: usize,
    pub golden_matched_in_cited: usize,
}

/// Score the selection layer (the synthesizer's citations vs golden).
pub fn score_selection(cited: &CitedChunks, example: &GoldenExample) -> SelectionScoreV2 {
    let contents = cited.contents();
    let golden_count = example.source_chunks.len();
    let matched = matched_golden_indices(&contents, example);
    let golden_matched_in_cited = matched.len();

    let precision = if contents.is_empty() {
        // No citations: vacuously precise if nothing golden was expected either.
        if golden_count == 0 { 1.0 } else { 0.0 }
    } else {
        // A cited chunk is "relevant" if it matches some golden chunk.
        let relevant_cited = contents
            .iter()
            .filter(|c| example.source_chunks.iter().any(|g| g.matches(c)))
            .count();
        relevant_cited as f64 / contents.len() as f64
    };
    let recall = if golden_count > 0 {
        golden_matched_in_cited as f64 / golden_count as f64
    } else {
        1.0
    };

    SelectionScoreV2 {
        query: example.query.clone(),
        precision,
        recall,
        cited_count: contents.len(),
        golden_count,
        golden_matched_in_cited,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness_extract::{CitedChunk, RetrievedChunk};

    fn ex(query: &str, golden: &[&str]) -> GoldenExample {
        GoldenExample {
            query: query.to_string(),
            expected_answer: String::new(),
            source_chunks: golden
                .iter()
                .map(|t| ChunkMatch::Substring {
                    text: t.to_string(),
                })
                .collect(),
            expected_citations: vec![],
            mode: "rag".to_string(),
            description: String::new(),
            is_adversarial: false,
            expected_should_answer: true,
            refusal_keywords: vec![],
            must_include: vec![],
            must_not_include: vec![],
            retrieval_hints: vec![],
            difficulty: Default::default(),
            relevance_grades: Default::default(),
            expected_tool: None,
            expected_tool_sequence: None,
            requires_triplet_reingest: false,
            capabilities: vec![],
            doc_scope_hint: "all".to_string(),
            expect_citations: None,
            requires_network: false,
            prior_turns: vec![],
            client_time: None,
            rubric_notes: None,
            expect_no_retrieval: false,
        }
    }

    fn ret(contents: &[&str]) -> RetrievedChunks {
        RetrievedChunks {
            chunks: contents
                .iter()
                .enumerate()
                .map(|(i, c)| RetrievedChunk {
                    chunk_id: format!("c{i}"),
                    content: c.to_string(),
                    score: Some(1.0 - i as f32 * 0.1),
                    rank: i,
                    tool: "dense_retrieval".to_string(),
                })
                .collect(),
        }
    }

    fn cit(contents: &[(usize, &str)]) -> CitedChunks {
        CitedChunks {
            chunks: contents
                .iter()
                .map(|(id, c)| CitedChunk {
                    chunk_id: Some(format!("c{id}")),
                    citation_id: *id as i64,
                    content: c.to_string(),
                    score: 0.9,
                })
                .collect(),
        }
    }

    #[test]
    fn retrieval_recall_hit_mrr_ndcg() {
        let r = ret(&["noise", "alpha beta", "gamma", "delta"]);
        let e = ex("q", &["alpha beta", "delta"]);
        let s = score_retrieval(&r, &e, 15);
        assert_eq!(s.matched_golden.len(), 2);
        assert!((s.recall - 1.0).abs() < 1e-9);
        assert!(s.hit);
        // first hit at rank 1 → mrr = 1/2
        assert!((s.mrr - 0.5).abs() < 1e-9);
        assert!(s.ndcg > 0.0 && s.ndcg <= 1.0);
        // Everything inside top-k here → both views agree.
        assert!((s.recall_at_k - 1.0).abs() < 1e-9);
        assert!(s.hit_at_k);
    }

    #[test]
    fn retrieval_miss_when_no_golden_in_topk() {
        let r = ret(&["noise", "more noise"]);
        let e = ex("q", &["alpha"]);
        let s = score_retrieval(&r, &e, 15);
        assert_eq!(s.matched_golden.len(), 0);
        assert!((s.recall - 0.0).abs() < 1e-9);
        assert!(!s.hit);
        assert!((s.recall_at_k - 0.0).abs() < 1e-9);
        assert!(!s.hit_at_k);
        assert!((s.mrr - 0.0).abs() < 1e-9);
        assert!((s.ndcg - 0.0).abs() < 1e-9);
    }

    #[test]
    fn full_stream_recall_counts_gold_from_later_rounds() {
        // Multi-round agent scenario: 25 merged chunks, gold first appears at
        // merged rank 20 (a later ReAct round). Old top-15 truncation scored
        // recall=0 (bogus RETRIEVAL_MISS); full-stream recall is 1.0 while the
        // top-k view honestly reports the late rank.
        let mut chunks: Vec<&str> = (0..24).map(|_| "noise").collect();
        chunks[20] = "alpha beta";
        let r = ret(&chunks);
        let e = ex("q", &["alpha beta"]);
        let s = score_retrieval(&r, &e, 15);
        assert!((s.recall - 1.0).abs() < 1e-9);
        assert!(s.hit);
        assert!((s.recall_at_k - 0.0).abs() < 1e-9);
        assert!(!s.hit_at_k);
        assert_eq!(s.first_hit_ranks, vec![20]);
        assert!((s.mrr - 1.0 / 21.0).abs() < 1e-9);
        assert_eq!(s.retrieved_count, 24);
    }

    #[test]
    fn graded_metrics_reduce_to_binary_when_relevance_grades_empty() {
        let r = ret(&["noise", "alpha beta", "gamma"]);
        let e = ex("q", &["alpha beta", "gamma"]);
        let s = score_retrieval(&r, &e, 15);
        assert!((s.graded_recall - s.recall).abs() < 1e-9);
        assert!((s.graded_ndcg - s.ndcg).abs() < 1e-9);
    }

    #[test]
    fn selection_precision_recall() {
        // cited: [golden-match, golden-match, irrelevant]
        let c = cit(&[(0, "alpha beta"), (1, "delta"), (2, "irrelevant")]);
        let e = ex("q", &["alpha beta", "delta"]);
        let s = score_selection(&c, &e);
        assert_eq!(s.cited_count, 3);
        assert_eq!(s.golden_matched_in_cited, 2);
        assert!((s.precision - 2.0 / 3.0).abs() < 1e-9);
        assert!((s.recall - 1.0).abs() < 1e-9);
    }
}
