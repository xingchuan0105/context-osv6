//! Decoupled RAG scorecard (Phase 0.2) — retrieval / selection / generation
//! scored as separate layers, plus a per-query diagnostic label.
//!
//! This replaces the conflation in `metrics.rs` where `Recall@15` was measured
//! against `ChatResponse.citations` (the synthesizer's selection) instead of
//! against the retriever's actual output (`tool_results`). See ADR 0011.
//!
//! ## Layers
//!
//! - **Retrieval** (`RetrievalScore`): scored against `RetrievedChunks` (all
//!   chunks returned by `dense/lexical/graph_retrieval` across all loop
//!   rounds). Metrics: Recall@k, Hit@k, MRR, nDCG@k (binary relevance).
//! - **Selection** (`SelectionScore`): scored against `CitedChunks` (the
//!   synthesizer's citations). Metrics: Citation Precision / Recall, where
//!   membership is resolved via `ChunkMatch::matches` against golden chunks.
//! - **Generation** (`GenerationScore`): Refusal correctness, synthesis
//!   contract compliance, and substring faithfulness (deterministic; the LLM
//!   judge lives in `judge.rs`, Phase 2).
//!
//! ## Diagnostic label
//!
//! `diagnostic_label` assigns one label per query in priority order so the
//! prompt loop can see *which layer* broke, not just that a scalar moved:
//! `RETRIEVAL_MISS` → `SELECTION_MISS` → `GENERATION_UNGROUNDED` →
//! `SYNTHESIS_CONTRACT` → `REFUSAL_WRONG` → `PASS`.

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
pub struct RetrievalScore {
    pub query: String,
    pub k: usize,
    pub recall: f64,
    pub hit: bool,
    pub mrr: f64,
    pub ndcg: f64,
    /// Graded recall (ADR 0011): weighted fraction of evidence-grade mass found
    /// in top-k. `source_chunks` count as grade 3 (critical); `relevance_grades`
    /// entries add partial-credit evidence (1=tangential, 2=relevant, 3=critical)
    /// keyed by a content signature. With empty `relevance_grades` this reduces
    /// to the binary `recall` (all grade 3) — no regression.
    pub graded_recall: f64,
    /// Graded nDCG@k: linear gain = max matched evidence grade at each rank;
    /// IDCG is the ideal descending-grade ordering. Reduces to binary `ndcg`
    /// when `relevance_grades` is empty.
    pub graded_ndcg: f64,
    pub retrieved_count: usize,
    pub golden_count: usize,
    pub matched_golden: Vec<usize>,
    /// Rank (0-indexed, first-seen order) of each matched golden chunk's first
    /// hit, parallel to `matched_golden`. Needed for MRR/nDCG debugging.
    pub first_hit_ranks: Vec<usize>,
}

/// Score the retrieval layer. `retrieved` is the deduped first-seen-ordered
/// chunk list from `extract_retrieved_chunks`.
pub fn score_retrieval(
    retrieved: &RetrievedChunks,
    example: &GoldenExample,
    k: usize,
) -> RetrievalScore {
    let contents: Vec<String> = retrieved.chunks.iter().map(|c| c.content.clone()).collect();
    let golden_count = example.source_chunks.len();

    // Match each golden chunk against the top-k retrieved contents.
    let topk: Vec<String> = contents.into_iter().take(k).collect();
    let mut matched = Vec::new();
    let mut first_hit_ranks = Vec::new();
    for (gi, g) in example.source_chunks.iter().enumerate() {
        if let Some(rank) = topk.iter().position(|c| g.matches(c)) {
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
    let mrr = first_hit_ranks
        .first()
        .map(|&r| 1.0 / (r as f64 + 1.0))
        .unwrap_or(0.0);

    // nDCG@k with binary relevance (matched golden = relevant). DCG sums
    // 1/log2(rank+2) over relevant positions; IDCG is the ideal ordering.
    let ndcg = if golden_count == 0 || first_hit_ranks.is_empty() {
        if golden_count == 0 { 1.0 } else { 0.0 }
    } else {
        let dcg: f64 = first_hit_ranks
            .iter()
            .map(|&r| 1.0 / ((r as f64 + 2.0).log2()))
            .sum();
        let ideal_relevant = golden_count.min(k);
        let idcg: f64 = (0..ideal_relevant)
            .map(|i| 1.0 / ((i as f64 + 2.0).log2()))
            .sum();
        if idcg > 0.0 { dcg / idcg } else { 0.0 }
    };

    // Graded relevance (ADR 0011): source_chunks = grade 3 (critical);
    // relevance_grades maps a content-signature substring to a finer grade for
    // partial-credit evidence. A retrieved chunk's grade is the max grade among
    // evidence units it matches. relevance_grades signatures must be DISTINCT
    // chunks from source_chunks (tangential/related evidence, not duplicates) —
    // otherwise the same chunk would double-count grade mass. With empty
    // relevance_grades, graded metrics reduce to the binary ones.
    const SOURCE_GRADE: u8 = 3;
    let graded_evidence: Vec<(ChunkMatch, u8)> = example
        .relevance_grades
        .iter()
        .map(|(sig, g)| (ChunkMatch::Substring { text: sig.clone() }, *g))
        .collect();
    let total_grade_mass: u32 =
        (example.source_chunks.len() as u32 * SOURCE_GRADE as u32)
            + graded_evidence.iter().map(|(_, g)| *g as u32).sum::<u32>();
    let mut found_source = vec![false; example.source_chunks.len()];
    let mut found_graded = vec![false; graded_evidence.len()];
    let mut rank_grades: Vec<u8> = Vec::with_capacity(topk.len());
    for c in &topk {
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
            .take(k)
            .enumerate()
            .map(|(i, &g)| g as f64 / ((i as f64 + 2.0).log2()))
            .sum();
        if gidcg > 0.0 { gdcg / gidcg } else { 0.0 }
    };

    RetrievalScore {
        query: example.query.clone(),
        k,
        recall,
        hit,
        mrr,
        ndcg,
        graded_recall,
        graded_ndcg,
        retrieved_count: retrieved.len().min(k),
        golden_count,
        matched_golden: matched,
        first_hit_ranks,
    }
}

// ---------------------------------------------------------------------------
// Selection layer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionScore {
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
pub fn score_selection(cited: &CitedChunks, example: &GoldenExample) -> SelectionScore {
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

    SelectionScore {
        query: example.query.clone(),
        precision,
        recall,
        cited_count: contents.len(),
        golden_count,
        golden_matched_in_cited,
    }
}

// ---------------------------------------------------------------------------
// Generation layer
// ---------------------------------------------------------------------------

/// Default Chinese refusal cue words. An answer containing any of these
/// (case-insensitive) is treated as a refusal, not a hallucination.
pub const DEFAULT_REFUSAL_KEYWORDS: &[&str] = &[
    "未提及",
    "未提到",
    "未找到",
    "未在文档中找到",
    "未在资料中找到",
    "未在文中",
    "未在资料中",
    "文中未",
    "文档中未",
    "资料中未",
    "未涉及",
    "未说明",
    "未予说明",
    "未予披露",
    "未提供",
    "未能找到",
    "未能确认",
    "无法确认",
    "无法确定",
    "无法回答",
    "无法提供",
    "不包含",
    "没有提及",
    "没有找到",
    "没有相关",
    "文档没有",
    "资料没有",
    "不在文档",
    "不在资料",
    "暂无相关",
    "无相关内容",
    "找不到",
    "not mentioned",
    "no information",
    "don't know",
    "do not know",
    "cannot answer",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefusalResult {
    pub query: String,
    pub is_refusal: bool,
    pub expected_should_answer: bool,
    /// True iff the refusal behavior matched the golden expectation.
    pub correct: bool,
}

/// Detect whether `answer` is a refusal and check it against the golden
/// expectation (`expected_should_answer`). Extra `refusal_keywords` from the
/// golden example are unioned with the default lexicon.
pub fn refusal_correctness(answer: &str, example: &GoldenExample) -> RefusalResult {
    let lower = answer.to_lowercase();
    let is_refusal = answer.trim().is_empty()
        || DEFAULT_REFUSAL_KEYWORDS
            .iter()
            .any(|kw| lower.contains(&kw.to_lowercase()))
        || example
            .refusal_keywords
            .iter()
            .any(|kw| lower.contains(&kw.to_lowercase()));
    // `correct` iff (answered AND expected_to_answer) OR (refused AND NOT expected_to_answer).
    let correct = (!is_refusal && example.expected_should_answer)
        || (is_refusal && !example.expected_should_answer);
    RefusalResult {
        query: example.query.clone(),
        is_refusal,
        expected_should_answer: example.expected_should_answer,
        correct,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractResult {
    pub query: String,
    pub compliant: bool,
    pub issues: Vec<String>,
}

/// Runtime fallback strings emitted when synthesis JSON parsing fails. These
/// are contract failures (the model produced unparseable JSON), NOT refusals —
/// the user received an English error string, not a grounded answer. Flagging
/// them as contract issues surfaces the real root cause (SYNTHESIS_CONTRACT).
const SYNTHESIS_FALLBACK_MARKERS: &[&str] = &[
    "evidence_insufficient_fallback",
    "could not format a validated cited answer",
    "could not find relevant evidence in your documents",
];

/// Check the rendered answer's synthesis contract: non-empty, citation markup
/// well-formed when evidence exists, no dangling `[[` without closing, and not
/// a runtime synthesis-fallback string.
///
/// `has_evidence` is true when the retrieval layer returned at least one chunk.
/// `is_refusal` is true when the answer is a refusal — refusals legitimately
/// carry no citation markup (the "Honest absence" contract has `citations:[]`),
/// so the no-citation check is skipped for them.
pub fn contract_compliance(answer: &str, has_evidence: bool, is_refusal: bool) -> ContractResult {
    let mut issues = Vec::new();
    if answer.trim().is_empty() {
        issues.push("empty_answer".to_string());
    }
    let lower = answer.to_lowercase();
    if SYNTHESIS_FALLBACK_MARKERS
        .iter()
        .any(|m| lower.contains(m))
    {
        issues.push("synthesis_fallback".to_string());
    }
    if has_evidence && !is_refusal {
        let has_cite =
            answer.contains("[[cite:") || answer.contains("[citation:") || answer.contains("[[");
        if !has_cite && !answer.trim().is_empty() {
            issues.push("no_citation_markup_with_evidence".to_string());
        }
    }
    // Dangling opening bracket without a closing `]]`/`]`.
    let open = answer.matches("[[").count();
    let close = answer.matches("]]").count();
    if open != close {
        issues.push(format!("unbalanced_brackets open={open} close={close}"));
    }
    ContractResult {
        query: String::new(),
        compliant: issues.is_empty(),
        issues,
    }
}

/// Answer-correctness gate (ADR 0011 answer-first labeling).
///
/// A query is answer-correct when the user actually received the right answer:
/// - `expected_should_answer=true`: the answer is NOT a refusal AND contains
///   every `must_include` token (substring). When `must_include` is empty, any
///   non-refusal answer is accepted.
/// - `expected_should_answer=false`: the answer IS a refusal.
///
/// This is independent of which chunk was retrieved — a correct answer found
/// via a different valid chunk still counts (recall@15 is a diagnostic, not
/// the correctness gate).
pub fn answer_correctness(answer: &str, example: &GoldenExample, refusal: &RefusalResult) -> bool {
    // `must_not_include` applies to BOTH branches: a forbidden token (e.g. the
    // 2020 figure "1467亿" leaking into a 2019 answer, or into a refusal of a
    // 2019 question) means the answer is wrong regardless of must_include or
    // refusal. Without this check, a refusal that concedes a wrong fact, or a
    // should-answer that includes a confabulated forbidden figure, would PASS
    // — a false positive. This is a deterministic gate; no judge needed.
    let has_forbidden = example
        .must_not_include
        .iter()
        .any(|m| answer.contains(m.as_str()));
    if has_forbidden {
        return false;
    }
    if example.expected_should_answer {
        if refusal.is_refusal {
            return false;
        }
        if example.must_include.is_empty() {
            // Deterministic layer cannot verify correctness without must_include.
            // Auto-returning true here would PASS a wrong/confabulated answer
            // (false positive). The harness warns at load time for this case so
            // it is caught at annotation time; correctness then falls to the
            // faithfulness/must_not_include gates and in-loop manual review.
            return true;
        }
        example.must_include.iter().all(|m| answer.contains(m.as_str()))
    } else {
        refusal.is_refusal
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaithfulnessReport {
    pub query: String,
    pub faithfulness: f64,
    pub total_claims: usize,
    pub supported_claims: usize,
    pub unsupported_claims: Vec<String>,
}

/// Deterministic substring faithfulness (Phase 0.2).
///
/// Extracts "hard" claim anchors — numbers, dates, and alphanumeric codes —
/// from the answer and checks each appears as a substring in the **cited**
/// chunk content. `faithfulness = supported / total` (1.0 when no claims).
///
/// Limitation (see ADR 0011): this only catches hard factual hallucinations
/// (wrong numbers/dates/codes). Same-rewrite semantic hallucinations
/// ("营收下滑" vs "收入下降") are invisible here — that is the LLM-Judge's
/// job in `judge.rs` (Phase 2).
pub fn substring_faithfulness(answer: &str, cited: &CitedChunks) -> FaithfulnessReport {
    use once_cell::sync::Lazy;
    use regex::Regex;

    static CLAIM_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(
            r"(?x)
              \d{4}[-/]\d{1,2}[-/]\d{1,2}        # 2019-01-02
            | \d{4}\u{5e74}\d{1,2}\u{6708}        # 2019年1月
            | \d{1,2}\u{6708}\d{1,2}\u{65e5}      # 1月2日
            | \d{4}\u{5e74}                       # 2019年
            | \d+\.\d+                            # 3.14
            | \d+                                 # 638
            | [A-Za-z]{2,}[-_]?[A-Za-z0-9]*       # PAC-05 / 4A / IPD
            ",
        )
        .unwrap()
    });

    let normalized = normalize_answer_for_faithfulness(answer);
    let claims: Vec<String> = CLAIM_RE
        .find_iter(&normalized)
        .map(|m| m.as_str().to_string())
        .filter(|c| !is_noise_claim(c))
        .collect();

    let context = cited.contents().join(" ");
    let mut supported = 0;
    let mut unsupported = Vec::new();
    for claim in &claims {
        if context.contains(claim.as_str()) {
            supported += 1;
        } else {
            unsupported.push(claim.clone());
        }
    }

    let total = claims.len();
    let faithfulness = if total == 0 {
        1.0
    } else {
        supported as f64 / total as f64
    };

    FaithfulnessReport {
        query: String::new(),
        faithfulness,
        total_claims: total,
        supported_claims: supported,
        unsupported_claims: unsupported,
    }
}

/// Strip citation markers and lightweight markdown before claim extraction so
/// `[citation:1]` does not produce a false `citation` anchor.
fn normalize_answer_for_faithfulness(answer: &str) -> String {
    use once_cell::sync::Lazy;
    use regex::Regex;

    static CITATION_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)\[citation:\d+\]|\[\[cite:[^\]]+\]\]|\[\[\d+\]\]").unwrap()
    });

    let stripped = CITATION_RE.replace_all(answer, " ");
    stripped
        .replace("**", "")
        .replace('*', "")
        .replace('`', "")
}

/// Drop claim anchors that are too generic to be a factual claim (single
/// letters, bare "true"/"false"-like noise). Also drops single-character
/// claims (e.g. the bare "4" split out of "4A架构") and two-character
/// digit+letter mixes (e.g. "4A"/"4R"/"V1"): these are term fragments that
/// substring-faithfulness cannot reliably verify against a definition chunk
/// (the cited chunk often contains the *definition* "业务、数据、应用、技术"
/// but not the *term* "4A"), producing false hallucination flags. The
/// `must_include` gate already verifies such terms at the answer level.
fn is_noise_claim(s: &str) -> bool {
    let lower = s.to_lowercase();
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let all_alpha = chars.iter().all(|c| c.is_ascii_alphabetic());
    let has_digit = chars.iter().any(|c| c.is_ascii_digit());
    matches!(lower.as_str(), "ok" | "id" | "pdf" | "doc" | "url" | "api")
        || (all_alpha && n < 2) // single letter
        || n < 2 // any single character (bare digit "4")
        || (n == 2 && has_digit && !all_alpha) // "4A" / "4R" / "V1" mixed
}

// ---------------------------------------------------------------------------
// Per-query scorecard + diagnostic label
// ---------------------------------------------------------------------------

/// One-word root-cause label for a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiagnosticLabel {
    RetrievalMiss,
    SelectionMiss,
    GenerationUngrounded,
    SynthesisContract,
    RefusalWrong,
    Pass,
}

impl DiagnosticLabel {
    pub fn as_str(self) -> &'static str {
        match self {
            DiagnosticLabel::RetrievalMiss => "RETRIEVAL_MISS",
            DiagnosticLabel::SelectionMiss => "SELECTION_MISS",
            DiagnosticLabel::GenerationUngrounded => "GENERATION_UNGROUNDED",
            DiagnosticLabel::SynthesisContract => "SYNTHESIS_CONTRACT",
            DiagnosticLabel::RefusalWrong => "REFUSAL_WRONG",
            DiagnosticLabel::Pass => "PASS",
        }
    }
}

/// Faithfulness score below which a query is labeled `GENERATION_UNGROUNDED`.
pub const FAITHFULNESS_UNGROUNDED_THRESHOLD: f64 = 0.5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerQueryScorecard {
    pub query: String,
    pub retrieval: RetrievalScore,
    pub selection: SelectionScore,
    pub refusal: RefusalResult,
    pub contract: ContractResult,
    pub faithfulness: FaithfulnessReport,
    pub label: DiagnosticLabel,
}

/// Build the full per-query scorecard and assign a diagnostic label.
///
/// `retrieved` / `cited` come from `extract_retrieved_chunks` /
/// `extract_cited_chunks`. `answer` is the rendered final answer. `k` is the
/// retrieval cutoff (15).
pub fn score_query(
    retrieved: &RetrievedChunks,
    cited: &CitedChunks,
    answer: &str,
    example: &GoldenExample,
    k: usize,
) -> PerQueryScorecard {
    let retrieval = score_retrieval(retrieved, example, k);
    let selection = score_selection(cited, example);
    let refusal = refusal_correctness(answer, example);
    let contract = contract_compliance(answer, !retrieved.is_empty(), refusal.is_refusal);
    let faithfulness = substring_faithfulness(answer, cited);
    let answer_correct = answer_correctness(answer, example, &refusal);

    // Answer-first labeling (ADR 0011): if the user got a correct, well-formed
    // answer, label PASS regardless of which chunk surfaced it. Only when the
    // answer is wrong do we drill into the layer that broke.
    //
    // Runtime synthesis failures (`synthesis_fallback` = unparseable JSON, or
    // `empty_answer`) are attributed SYNTHESIS_CONTRACT BEFORE retrieval/selection:
    // the answer pipeline broke and the user received an error/empty string, so
    // retrieval and selection metrics are moot for root-cause attribution — even
    // if the golden substring happened not to match the retrieved chunks (which
    // would otherwise mislabel a pipeline failure as RETRIEVAL_MISS).
    //
    // Priority for wrong answers: SYNTHESIS_FALLBACK/EMPTY → RETRIEVAL_MISS →
    // SELECTION_MISS → (other) SYNTHESIS_CONTRACT → REFUSAL_WRONG →
    // GENERATION_UNGROUNDED. Contract is checked before refusal so a runtime
    // synthesis-fallback string (JSON parse failure) is attributed to its real
    // root cause (contract), not misread as a refusal.
    let has_pipeline_failure = contract
        .issues
        .iter()
        .any(|i| i == "synthesis_fallback" || i == "empty_answer");
    let label = if answer_correct && contract.compliant {
        DiagnosticLabel::Pass
    } else if has_pipeline_failure {
        DiagnosticLabel::SynthesisContract
    } else if retrieval.golden_count > 0 && retrieval.matched_golden.is_empty() {
        DiagnosticLabel::RetrievalMiss
    } else if retrieval.golden_count > 0 && selection.golden_matched_in_cited == 0 {
        DiagnosticLabel::SelectionMiss
    } else if !contract.compliant {
        DiagnosticLabel::SynthesisContract
    } else if !refusal.correct {
        DiagnosticLabel::RefusalWrong
    } else if !answer_correct {
        // should-answer but missing required content (must_include), with
        // retrieval otherwise OK → generation failed to extract/synthesize.
        DiagnosticLabel::GenerationUngrounded
    } else if !faithfulness.unsupported_claims.is_empty()
        && faithfulness.faithfulness < FAITHFULNESS_UNGROUNDED_THRESHOLD
    {
        DiagnosticLabel::GenerationUngrounded
    } else {
        DiagnosticLabel::Pass
    };

    PerQueryScorecard {
        query: example.query.clone(),
        retrieval,
        selection,
        refusal,
        contract,
        faithfulness,
        label,
    }
}

// ---------------------------------------------------------------------------
// Aggregation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScorecardSummary {
    pub total: usize,
    pub retrieval_recall_at_k: f64,
    pub retrieval_hit_at_k: f64,
    pub retrieval_mrr: f64,
    pub retrieval_ndcg: f64,
    /// Graded-relevance retrieval metrics (ADR 0011): partial credit for
    /// tangential evidence via `relevance_grades`. Reduce to the binary
    /// `retrieval_*` fields when no query sets `relevance_grades`.
    pub retrieval_graded_recall_at_k: f64,
    pub retrieval_graded_ndcg: f64,
    pub selection_precision: f64,
    pub selection_recall: f64,
    pub refusal_correct_rate: f64,
    pub contract_compliance_rate: f64,
    pub faithfulness_mean: f64,
    /// Mean retrieval recall over **answerable** queries only
    /// (`expected_should_answer == true`). The headline `retrieval_recall_at_k`
    /// averages over ALL queries, which includes adversarial/refusal queries
    /// whose `golden_count == 0` yields a vacuous `recall = 1.0` — inflating
    /// the aggregate by ~`(n_adversarial / n_total)` of "free" recall. This
    /// field is the honest retrieval number on queries that actually need
    /// evidence.
    pub retrieval_recall_at_k_on_answerable: f64,
    /// Graded recall over answerable queries only (excludes vacuous adversarial).
    pub retrieval_graded_recall_at_k_on_answerable: f64,
    /// Mean substring-faithfulness over answerable queries only. Adversarial
    /// refusals carry no hard claims → vacuous `faithfulness = 1.0`, which
    /// inflates `faithfulness_mean`. This field excludes them.
    pub faithfulness_mean_on_answerable: f64,
    pub label_counts: std::collections::BTreeMap<DiagnosticLabel, usize>,
}

impl ScorecardSummary {
    pub fn from_scorecards(cards: &[PerQueryScorecard]) -> Self {
        let total = cards.len();
        let n = total.max(1) as f64;
        let mut label_counts = std::collections::BTreeMap::new();
        for c in cards {
            *label_counts.entry(c.label).or_insert(0) += 1;
        }
        let answerable: Vec<&PerQueryScorecard> = cards
            .iter()
            .filter(|c| c.refusal.expected_should_answer)
            .collect();
        let n_ans = answerable.len().max(1) as f64;
        Self {
            total,
            retrieval_recall_at_k: cards.iter().map(|c| c.retrieval.recall).sum::<f64>() / n,
            retrieval_hit_at_k: cards.iter().filter(|c| c.retrieval.hit).count() as f64 / n,
            retrieval_mrr: cards.iter().map(|c| c.retrieval.mrr).sum::<f64>() / n,
            retrieval_ndcg: cards.iter().map(|c| c.retrieval.ndcg).sum::<f64>() / n,
            retrieval_graded_recall_at_k: cards.iter().map(|c| c.retrieval.graded_recall).sum::<f64>()
                / n,
            retrieval_graded_ndcg: cards.iter().map(|c| c.retrieval.graded_ndcg).sum::<f64>() / n,
            selection_precision: cards.iter().map(|c| c.selection.precision).sum::<f64>() / n,
            selection_recall: cards.iter().map(|c| c.selection.recall).sum::<f64>() / n,
            refusal_correct_rate: cards.iter().filter(|c| c.refusal.correct).count() as f64 / n,
            contract_compliance_rate: cards.iter().filter(|c| c.contract.compliant).count() as f64
                / n,
            faithfulness_mean: cards
                .iter()
                .map(|c| c.faithfulness.faithfulness)
                .sum::<f64>()
                / n,
            retrieval_recall_at_k_on_answerable: answerable
                .iter()
                .map(|c| c.retrieval.recall)
                .sum::<f64>()
                / n_ans,
            retrieval_graded_recall_at_k_on_answerable: answerable
                .iter()
                .map(|c| c.retrieval.graded_recall)
                .sum::<f64>()
                / n_ans,
            faithfulness_mean_on_answerable: answerable
                .iter()
                .map(|c| c.faithfulness.faithfulness)
                .sum::<f64>()
                / n_ans,
            label_counts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness_extract::{CitedChunk, RetrievedChunk};

    fn ex(query: &str, golden: &[&str], expected_should_answer: bool) -> GoldenExample {
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
            expected_should_answer,
            refusal_keywords: vec![],
            must_include: vec![],
            must_not_include: vec![],
            retrieval_hints: vec![],
            difficulty: Default::default(),
            relevance_grades: Default::default(),
            expected_tool: None,
            expected_tool_sequence: None,
            requires_triplet_reingest: false,
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
        let e = ex("q", &["alpha beta", "delta"], true);
        let s = score_retrieval(&r, &e, 15);
        assert_eq!(s.matched_golden.len(), 2);
        assert!((s.recall - 1.0).abs() < 1e-9);
        assert!(s.hit);
        // first hit at rank 1 → mrr = 1/2
        assert!((s.mrr - 0.5).abs() < 1e-9);
        assert!(s.ndcg > 0.0 && s.ndcg <= 1.0);
    }

    #[test]
    fn retrieval_miss_when_no_golden_in_topk() {
        let r = ret(&["noise", "more noise"]);
        let e = ex("q", &["alpha"], true);
        let s = score_retrieval(&r, &e, 15);
        assert_eq!(s.matched_golden.len(), 0);
        assert!((s.recall - 0.0).abs() < 1e-9);
        assert!(!s.hit);
        assert!((s.mrr - 0.0).abs() < 1e-9);
        assert!((s.ndcg - 0.0).abs() < 1e-9);
    }

    #[test]
    fn graded_recall_gives_partial_credit_for_tangential_evidence() {
        // source_chunks = ["alpha"] (grade 3, critical); a tangential chunk
        // matching relevance_grades signature "noise" gets grade 1. Retriever
        // returns only the tangential chunk → binary recall 0, graded recall
        // 1/(3+1) = 0.25, graded ndcg > 0.
        let mut e = ex("q", &["alpha"], true);
        e.relevance_grades.insert("noise".to_string(), 1);
        let r = ret(&["noise chunk"]);
        let s = score_retrieval(&r, &e, 15);
        assert!((s.recall - 0.0).abs() < 1e-9); // binary: critical not found
        assert!((s.graded_recall - 0.25).abs() < 1e-9);
        assert!(s.graded_ndcg > 0.0 && s.graded_ndcg < 1.0);
    }

    #[test]
    fn graded_metrics_reduce_to_binary_when_relevance_grades_empty() {
        let r = ret(&["noise", "alpha beta", "gamma"]);
        let e = ex("q", &["alpha beta", "gamma"], true);
        let s = score_retrieval(&r, &e, 15);
        // No relevance_grades → all evidence grade 3 → graded == binary.
        assert!((s.graded_recall - s.recall).abs() < 1e-9);
        assert!((s.graded_ndcg - s.ndcg).abs() < 1e-9);
    }

    #[test]
    fn graded_recall_full_when_all_evidence_found() {
        // Two evidence units in DISTINCT chunks (the normal corpus case): both
        // found at the top → graded recall 1.0 and graded nDCG 1.0.
        let mut e = ex("q", &["alpha"], true);
        e.relevance_grades.insert("beta".to_string(), 2);
        let r = ret(&["alpha chunk", "beta chunk"]);
        let s = score_retrieval(&r, &e, 15);
        assert!((s.recall - 1.0).abs() < 1e-9);
        assert!((s.graded_recall - 1.0).abs() < 1e-9);
        assert!((s.graded_ndcg - 1.0).abs() < 1e-9);
    }

    #[test]
    fn graded_ndcg_below_one_when_evidence_merged_into_one_chunk() {
        // Both signatures match the SAME chunk → per-rank DCG counts it once
        // (max grade), but IDCG assumes two ideal slots → graded nDCG < 1.0
        // even though all evidence is found. This is the standard nDCG
        // artifact when the corpus merges evidence into one chunk; graded
        // recall still reaches 1.0.
        let mut e = ex("q", &["alpha"], true);
        e.relevance_grades.insert("beta".to_string(), 2);
        let r = ret(&["alpha and beta together"]);
        let s = score_retrieval(&r, &e, 15);
        assert!((s.graded_recall - 1.0).abs() < 1e-9);
        assert!(s.graded_ndcg > 0.0 && s.graded_ndcg < 1.0);
    }

    #[test]
    fn selection_precision_recall() {
        // cited: [golden-match, golden-match, irrelevant]
        let c = cit(&[(0, "alpha beta"), (1, "delta"), (2, "irrelevant")]);
        let e = ex("q", &["alpha beta", "delta"], true);
        let s = score_selection(&c, &e);
        assert_eq!(s.cited_count, 3);
        assert_eq!(s.golden_matched_in_cited, 2);
        assert!((s.precision - 2.0 / 3.0).abs() < 1e-9);
        assert!((s.recall - 1.0).abs() < 1e-9);
    }

    #[test]
    fn refusal_correct_when_should_answer_and_answers() {
        let e = ex("q", &["x"], true);
        let r = refusal_correctness("the answer is 42", &e);
        assert!(!r.is_refusal);
        assert!(r.correct);
    }

    #[test]
    fn refusal_correct_when_should_refuse_and_refuses() {
        let e = ex("q", &["x"], false);
        let r = refusal_correctness("文档中未提及该信息", &e);
        assert!(r.is_refusal);
        assert!(r.correct);
    }

    #[test]
    fn refusal_wrong_when_should_answer_but_refuses() {
        let e = ex("q", &["x"], true);
        let r = refusal_correctness("未找到相关内容", &e);
        assert!(r.is_refusal);
        assert!(!r.correct);
    }

    #[test]
    fn contract_flags_empty_and_unbalanced() {
        let bad = contract_compliance("", true, false);
        assert!(!bad.compliant);
        let unbal = contract_compliance("see [[cite:c1 for info", true, false);
        assert!(!unbal.compliant);
        let good = contract_compliance("answer [[cite:c1]] done", true, false);
        assert!(good.compliant);
        // Refusals legitimately carry no citation markup even with evidence.
        let refusal = contract_compliance("文档中未提及该信息", true, true);
        assert!(refusal.compliant);
    }

    #[test]
    fn faithfulness_ignores_citation_markup_and_markdown() {
        let c = cit(&[(0, "华为IPD流程 概念阶段 概念启动 PAC-05")]);
        let f = substring_faithfulness(
            "华为IPD流程中活动号为PAC-05的活动是**概念启动**，位于**概念阶段**。[citation:1]",
            &c,
        );
        assert!(f.unsupported_claims.is_empty(), "{:?}", f.unsupported_claims);
        assert!((f.faithfulness - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn faithfulness_catches_fabricated_number() {
        // answer claims "2019年" and "638"; cited chunk has 2019年 but not 638.
        let c = cit(&[(0, "公司于2019年成立")]);
        let f = substring_faithfulness("公司于2019年成立，员工638人", &c);
        assert!(f.unsupported_claims.contains(&"638".to_string()));
        assert!(f.faithfulness < 1.0);
    }

    #[test]
    fn label_priority_retrieval_miss_beats_refusal_wrong() {
        // retrieval empty, should-answer but answer refuses.
        let r = ret(&["noise"]);
        let c = cit(&[]);
        let e = ex("q", &["alpha"], true);
        let card = score_query(&r, &c, "未找到", &e, 15);
        assert_eq!(card.label, DiagnosticLabel::RetrievalMiss);
    }

    #[test]
    fn label_selection_miss_when_retrieved_but_not_cited() {
        let r = ret(&["alpha beta"]);
        let c = cit(&[]); // synthesizer cited nothing
        let e = ex("q", &["alpha beta"], true);
        let card = score_query(&r, &c, "answer with no cite", &e, 15);
        assert_eq!(card.label, DiagnosticLabel::SelectionMiss);
    }

    #[test]
    fn label_pass_when_all_good() {
        let r = ret(&["alpha beta"]);
        let c = cit(&[(0, "alpha beta")]);
        let e = ex("q", &["alpha beta"], true);
        let card = score_query(&r, &c, "alpha beta [[cite:c0]]", &e, 15);
        assert_eq!(card.label, DiagnosticLabel::Pass);
    }

    #[test]
    fn label_pass_when_answer_correct_despite_recall_zero() {
        // Retrieval missed the golden substring chunk, but the answer contains
        // the required must_include token (found via a different valid chunk).
        // Answer-correctness overrides retrieval recall (ADR 0011 answer-first).
        let mut e = ex("q", &["2019年于大连市投资建厂"], true);
        e.must_include = vec!["2019年".to_string()];
        let r = ret(&["成立于2019年，中韩合资"]); // different valid chunk, no golden substring
        let c = cit(&[(0, "成立于2019年，中韩合资")]);
        let card = score_query(&r, &c, "公司于2019年建厂[[cite:c0]]", &e, 15);
        assert_eq!(card.retrieval.recall, 0.0); // golden substring not in retrieved
        assert_eq!(card.label, DiagnosticLabel::Pass); // but answer is correct
    }

    #[test]
    fn label_retrieval_miss_when_answer_wrong_and_recall_zero() {
        // Answer wrong (missing must_include) AND retrieval missed → RETRIEVAL_MISS.
        let mut e = ex("q", &["550万"], true);
        e.must_include = vec!["550万".to_string()];
        let r = ret(&["noise about marketing"]);
        let c = cit(&[]);
        let card = score_query(&r, &c, "文档中未提及营收数据", &e, 15);
        assert_eq!(card.label, DiagnosticLabel::RetrievalMiss);
    }

    #[test]
    fn label_pass_for_correct_refusal_without_cite() {
        // Adversarial: should refuse; model refuses cleanly without citation.
        // Contract must not penalize the missing cite on a refusal.
        let mut e = ex("q", &["x"], false);
        e.expected_should_answer = false;
        let r = ret(&["some off-topic chunk"]);
        let c = cit(&[]);
        let card = score_query(&r, &c, "文档中未提及保修年限", &e, 15);
        assert_eq!(card.label, DiagnosticLabel::Pass);
    }

    #[test]
    fn label_synthesis_contract_for_english_fallback() {
        // The runtime emits this English string when synthesis JSON fails to
        // parse. It is a contract failure, NOT a refusal — root cause is JSON,
        // so it must label SYNTHESIS_CONTRACT (not REFUSAL_WRONG, not PASS),
        // whether the query should-answer or should-refuse.
        let mut e_ans = ex("q", &["550万"], true);
        e_ans.must_include = vec!["550万".to_string()];
        let r = ret(&["成立于2019年，营收550万"]);
        let c = cit(&[(0, "成立于2019年，营收550万")]);
        let card = score_query(
            &r,
            &c,
            "I found relevant material but could not format a validated cited answer. \
             Please try asking again.",
            &e_ans,
            15,
        );
        assert_eq!(card.label, DiagnosticLabel::SynthesisContract);
        assert!(card.contract.issues.iter().any(|i| i == "synthesis_fallback"));

        // Same fallback on a should-refuse query is still a contract failure.
        let mut e_ref = ex("q", &["x"], false);
        e_ref.expected_should_answer = false;
        let card2 = score_query(
            &r,
            &c,
            "I found relevant material but could not format a validated cited answer. \
             Please try asking again.",
            &e_ref,
            15,
        );
        assert_eq!(card2.label, DiagnosticLabel::SynthesisContract);
    }

    #[test]
    fn must_not_include_blocks_pass_when_forbidden_token_present() {
        // should-answer: must_include satisfied, but answer leaks a forbidden
        // figure (must_not_include). Correctness must be FALSE → not PASS.
        let mut e = ex("q", &["2019年"], true);
        e.must_include = vec!["2019年".to_string()];
        e.must_not_include = vec!["1467亿".to_string()];
        let r = ret(&["2019年行业规模"]);
        let c = cit(&[(0, "2019年行业规模")]);
        let card = score_query(&r, &c, "2019年行业规模为1467亿[[cite:c0]]", &e, 15);
        assert!(!card.refusal.is_refusal);
        assert!(!answer_correctness("2019年行业规模为1467亿", &e, &card.refusal));
        assert_ne!(card.label, DiagnosticLabel::Pass);
    }

    #[test]
    fn must_not_include_blocks_refusal_that_leaks_forbidden_fact() {
        // should-refuse: model refuses BUT concedes the wrong figure → still wrong.
        let mut e = ex("q", &["x"], false);
        e.expected_should_answer = false;
        e.must_not_include = vec!["1467亿".to_string()];
        let r = ret(&["some chunk"]);
        let c = cit(&[]);
        let card = score_query(&r, &c, "文档中未提及2019年规模，仅2020年为1467亿", &e, 15);
        assert!(!answer_correctness("文档中未提及2019年规模，仅2020年为1467亿", &e, &card.refusal));
        assert_ne!(card.label, DiagnosticLabel::Pass);
    }
}
