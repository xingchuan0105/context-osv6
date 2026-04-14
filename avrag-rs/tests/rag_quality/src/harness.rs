//! RAG Quality Evaluation Harness — runs the full pipeline against a golden set.
//!
//! PRD §13.1: "离线评估：检索、生成、端到端"

use crate::golden_set::{GoldenDataset, GoldenExample};
use crate::metrics::EvaluationMetrics;
use anyhow::Result;
use std::path::Path;
use tracing::{info, warn};

/// Configuration for the evaluation harness.
#[derive(Debug, Clone)]
pub struct HarnessConfig {
    /// Recall@K cutoff (default: 15)
    pub recall_k: usize,
    /// Baseline recall to compare against (for regression gate)
    pub baseline_recall: f64,
    /// Maximum examples to run per subset (None = all)
    pub max_examples_per_subset: Option<usize>,
    /// Whether to print per-example results
    pub verbose: bool,
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            recall_k: 15,
            baseline_recall: 0.97,
            max_examples_per_subset: None,
            verbose: true,
        }
    }
}

/// The evaluation harness.
#[derive(Debug)]
pub struct EvaluationHarness {
    dataset: GoldenDataset,
    config: HarnessConfig,
}

impl EvaluationHarness {
    /// Create a harness from a golden-set JSON file.
    pub fn from_file(path: impl AsRef<Path>, config: HarnessConfig) -> Result<Self> {
        let dataset = GoldenDataset::load(path)?;
        Ok(Self { dataset, config })
    }

    /// Create a harness from an in-memory dataset.
    pub fn new(dataset: GoldenDataset, config: HarnessConfig) -> Self {
        Self { dataset, config }
    }

    /// Run evaluation across the entire golden set.
    ///
    /// For each example:
    /// 1. Run the RAG pipeline (retrieval + generation)
    /// 2. Compute Recall@K against golden chunks
    /// 3. Compute Citation Accuracy
    /// 4. Compute Hallucination Rate
    ///
    /// Returns a summary report.
    ///
    /// NOTE: This is the *integration test* interface.
    /// It calls into `avrag_rag_core::RagRuntime::execute()` with mocked auth.
    /// Production use would inject a real `PgAppRepository` + `HttpQdrantBackend`.
    pub async fn run_all(&self) -> Result<EvaluationReport> {
        let HarnessConfig {
            recall_k,
            verbose,
            max_examples_per_subset,
            ..
        } = self.config;

        let mut recall_results = Vec::new();
        let mut citation_results = Vec::new();
        let mut hallucination_results = Vec::new();
        let mut failures = Vec::new();

        for subset in &self.dataset.subsets {
            let examples: Vec<_> = max_examples_per_subset
                .map(|n| subset.examples.iter().take(n).collect())
                .unwrap_or_else(|| subset.examples.iter().collect());

            info!(
                subset = subset.name,
                count = examples.len(),
                "running evaluation on subset"
            );

            for example in examples {
                if verbose {
                    info!(query = %example.query, mode = %example.mode, "evaluating example");
                }

                let result = self.evaluate_example(example, recall_k).await;

                match result {
                    Ok((recall_res, citation_res, halluc_res)) => {
                        recall_results.push(recall_res);
                        citation_results.push(citation_res);
                        hallucination_results.push(halluc_res);
                    }
                    Err(e) => {
                        warn!(error = %e, query = %example.query, "evaluation failed for example");
                        failures.push((example.query.clone(), e.to_string()));
                    }
                }
            }
        }

        let metrics =
            EvaluationMetrics::aggregate(recall_results, citation_results, hallucination_results);

        Ok(EvaluationReport {
            dataset_version: self.dataset.version.clone(),
            metrics,
            failures,
        })
    }

    /// Evaluate a single golden-set example.
    ///
    /// In a full integration test, this would:
    /// 1. Construct a ChatRequest from the golden query
    /// 2. Call `RagRuntime::execute()` with a real or mocked auth context
    /// 3. Extract retrieved chunks, citations, and the generated answer
    /// 4. Run the three metrics
    ///
    /// Currently this is a *skeleton* — it requires a real `AppState`
    /// to be injected. The actual test binary (examples/run_eval.rs) wires this up.
    async fn evaluate_example(
        &self,
        example: &GoldenExample,
        recall_k: usize,
    ) -> Result<(
        crate::metrics::RecallResult,
        crate::metrics::CitationAccuracyResult,
        crate::metrics::HallucinationResult,
    )> {
        // ─── RETRIEVAL ─────────────────────────────────────────────────────────
        // In integration tests, retrieve from the RAG runtime:
        // let retrieved_chunks = runtime.retrieve(&example.query, TopK(recall_k)).await?;
        let retrieved_chunks: Vec<String> = Vec::new(); // TODO: wire to RagRuntime

        // ─── GENERATION ────────────────────────────────────────────────────────
        // let answer = runtime.synthesize(&example.query, &retrieved_chunks).await?;
        let answer = String::new(); // TODO: wire to RagRuntime

        // ─── CITATION EXTRACTION ───────────────────────────────────────────────
        let citation_indices = crate::metrics::EvaluationMetrics::extract_citation_indices(&answer);

        // ─── METRICS ───────────────────────────────────────────────────────────
        let recall_res = crate::metrics::EvaluationMetrics::recall_at_k(
            &example.query,
            &retrieved_chunks,
            example,
            recall_k,
        );
        let citation_res = crate::metrics::EvaluationMetrics::citation_accuracy(
            &example.query,
            &citation_indices,
            example,
        );
        let halluc_res = crate::metrics::EvaluationMetrics::hallucination_check(
            &example.query,
            &answer,
            &retrieved_chunks,
        );

        Ok((recall_res, citation_res, halluc_res))
    }

    /// Number of examples in the loaded dataset.
    pub fn example_count(&self) -> usize {
        self.dataset.len()
    }
}

/// Full evaluation report.
#[derive(Debug)]
pub struct EvaluationReport {
    pub dataset_version: String,
    pub metrics: EvaluationMetrics,
    pub failures: Vec<(String, String)>, // (query, error)
}

impl EvaluationReport {
    /// Print a human-readable summary.
    pub fn summary(&self) -> String {
        let m = &self.metrics;
        let gate_status = m.assert_passing(0.97);

        let gate_str = match gate_status {
            Ok(()) => "✅ ALL GATES PASSING".to_string(),
            Err(ref e) => {
                format!("❌ GATES FAILED:\n{}", e)
            }
        };

        format!(
            r##"## RAG Quality Evaluation Report

Dataset version: {}
Total examples: {}

### Metrics
- Recall@15:    {:.1}%  (baseline 97%, gate ≤3% drop)
- Citation Acc: {:.1}%  (gate ≥95%)
- Hallucination: {:.1}%  (gate ≤2%)

### Gate Status
{}

### Failures: {}
"##,
            self.dataset_version,
            m.total_examples,
            m.recall_at_15 * 100.0,
            m.citation_accuracy * 100.0,
            m.hallucination_rate * 100.0,
            gate_str,
            self.failures.len()
        )
    }
}
