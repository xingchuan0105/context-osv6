//! RAG Quality Evaluation Pipeline — §13 PRD
//!
//! Provides offline evaluation of retrieval and generation quality:
//! - Recall@K (retrieval quality)
//! - Citation Accuracy (generation quality)
//! - Hallucination Rate (generation faithfulness)
//!
//! # Golden Set Format
//!
//! Each entry is a `{ query, expected_answer, source_chunks, expected_citations }` triplet.
//! The harness runs the RAG pipeline and compares outputs against the golden set.
//!
//! # Usage
//!
//! ```rust,ignore
//! let harness = EvaluationHarness::new(golden_set_path)?;
//! let report = harness.run_all().await?;
//! report.assert_passing()?; // Gate: Recall@15 regression only; generation gates live in metrics_v2
//! ```

pub mod golden_set;
pub mod harness;
pub mod harness_extract;
pub mod judge;
pub mod metrics;
pub mod metrics_v2;
pub mod tool_coverage;

pub use golden_set::{GoldenDataset, GoldenDifficulty, GoldenExample, GoldenSubset};
pub use harness::{EvaluationHarness, HarnessConfig, RagEvaluator};
pub use harness_extract::{
    CitedChunk, CitedChunks, RetrievedChunk, RetrievedChunks, extract_cited_chunks,
    extract_retrieved_chunks,
};
pub use judge::{
    FaithfulnessInput, FaithfulnessJudge, FaithfulnessJudgment, LlmNliJudge,
    SubstringFaithfulnessJudge, cohen_kappa_binary,
};
pub use metrics::{CitationAccuracyResult, EvaluationMetrics, HallucinationResult, RecallResult};
pub use metrics_v2::{
    ContractResult, DiagnosticLabel, FaithfulnessReport, PerQueryScorecard, RefusalResult,
    RetrievalScore, ScorecardSummary, SelectionScore, contract_compliance, refusal_correctness,
    score_query, score_retrieval, score_selection, substring_faithfulness,
};
pub use tool_coverage::{
    ToolCoverageScore, ToolCoverageSummary, extract_tool_trace, tool_present,
    tool_sequence_matches,
};
