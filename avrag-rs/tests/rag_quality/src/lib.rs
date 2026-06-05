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
//! report.assert_passing()?; // Gate: Recall@15 >= 97%, Citation Accuracy >= 95%, Hallucination <= 2%
//! ```

pub mod golden_set;
pub mod harness;
pub mod metrics;

pub use golden_set::{GoldenDataset, GoldenExample, GoldenSubset};
pub use harness::{EvaluationHarness, HarnessConfig, RagEvaluator};
pub use metrics::{CitationAccuracyResult, EvaluationMetrics, HallucinationResult, RecallResult};
