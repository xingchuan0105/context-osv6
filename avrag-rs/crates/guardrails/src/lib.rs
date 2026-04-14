//! Guardrails crate — input and output guards for the RAG pipeline.
//!
//! Execution order: Input Guards → Retrieval → Generation → Output Guards
//!
//! # Guards
//!
//! - **Input Guards**: Prompt injection, privilege escalation, scope enforcement
//! - **Output Guards**: Citation provability, PII scrubbing, harmful content

pub mod input;
pub mod output;

use common::{DegradeTraceItem, GuardReport, GuardResult};
use uuid::Uuid;

/// Main guard pipeline — orchestrates input and output guards.
pub struct GuardPipeline {
    input: input::InputGuardPipeline,
    output: output::OutputGuardPipeline,
}

impl GuardPipeline {
    /// Create a new GuardPipeline with all sub-guards initialized.
    pub fn new() -> Self {
        Self {
            input: input::InputGuardPipeline::new(),
            output: output::OutputGuardPipeline::new(),
        }
    }

    /// Run all input guards against a user query.
    ///
    /// Returns the first blocking `GuardResult` if any guard blocks,
    /// or a passing result if all guards allow.
    pub fn check_input(
        &self,
        query: &str,
        org_id: Uuid,
        user_id: Uuid,
        doc_scope: &[String],
        notebook_id: Option<Uuid>,
        trace_id: Option<String>,
    ) -> GuardResult {
        let input_ctx = input::InputGuardContext {
            query,
            org_id,
            user_id,
            doc_scope,
            notebook_id,
            trace_id: trace_id.clone(),
        };

        // Run all input guards
        if let Some(result) = self.input.run(&input_ctx) {
            return result;
        }

        GuardResult::pass("input:all")
    }

    /// Run all output guards against the synthesizer response.
    ///
    /// Returns a tuple of `(sanitized_response, guard_report)`.
    /// The sanitized response may be truncated/redacted if a guard triggered.
    pub fn check_output(
        &self,
        response: &str,
        citations: &[common::Citation],
        chunk_ids: &[Uuid],
        trace_id: Option<String>,
    ) -> (String, GuardReport) {
        let mut degrade_trace = Vec::new();
        let mut output_results = Vec::new();
        let mut sanitized = response.to_string();

        // Citation provability check
        let citation_result = self.output.citation_provability.check(
            &sanitized,
            citations,
            chunk_ids,
            trace_id.clone(),
        );
        output_results.push(citation_result.clone());
        if !citation_result.passed {
            degrade_trace.push(DegradeTraceItem {
                stage: "output_guard:citation_provability".into(),
                reason: citation_result.reason.clone(),
                impact: citation_result.action.to_string(),
            });
        }

        // PII scrubbing — always runs, may redact in place
        let pii_result = self.output.pii_scrubber.check(&sanitized, trace_id.clone());
        sanitized = self.output.pii_scrubber.scrub(&sanitized);
        output_results.push(pii_result.clone());
        if let Some(redacted) = pii_result
            .details
            .and_then(|d| d.get("redacted_count").cloned())
        {
            if redacted.as_i64().unwrap_or(0) > 0 {
                degrade_trace.push(DegradeTraceItem {
                    stage: "output_guard:pii_scrubber".into(),
                    reason: format!("{} PII instances redacted", redacted),
                    impact: "redact".into(),
                });
            }
        }

        // Harmful content check
        let harmful_result = self
            .output
            .harmful_content
            .check(&sanitized, trace_id.clone());
        output_results.push(harmful_result.clone());
        if !harmful_result.passed {
            degrade_trace.push(DegradeTraceItem {
                stage: "output_guard:harmful_content".into(),
                reason: harmful_result.reason.clone(),
                impact: harmful_result.action.to_string(),
            });
            // Truncate response if blocked
            sanitized = "[Content filtered due to policy violation]".to_string();
        }

        let blocked = degrade_trace.iter().any(|d| d.impact == "block");

        (
            sanitized,
            GuardReport {
                input_results: Vec::new(),
                output_results,
                blocked,
                degrade_trace,
            },
        )
    }
}

impl Default for GuardPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guard_pipeline_check_input_passes_normal_query() {
        let pipeline = GuardPipeline::new();
        let org_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let result = pipeline.check_input(
            "What is machine learning?",
            org_id,
            user_id,
            &[],
            None,
            Some("test-trace".into()),
        );
        assert!(result.passed);
    }

    #[test]
    fn test_guard_pipeline_check_input_blocks_sql_injection() {
        let pipeline = GuardPipeline::new();
        let org_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let result = pipeline.check_input(
            "'; DROP TABLE users; --",
            org_id,
            user_id,
            &[],
            None,
            Some("test-trace".into()),
        );
        assert!(!result.passed);
    }

    #[test]
    fn test_guard_pipeline_check_output_passes_clean_response() {
        let pipeline = GuardPipeline::new();
        let (sanitized, report) = pipeline.check_output(
            "The capital of France is Paris.",
            &[],
            &[],
            Some("test-trace".into()),
        );
        assert_eq!(sanitized, "The capital of France is Paris.");
        assert!(!report.blocked);
    }

    #[test]
    fn test_guard_pipeline_check_output_redacts_pii() {
        let pipeline = GuardPipeline::new();
        let (sanitized, report) = pipeline.check_output(
            "My email is alice@example.com and SSN is 123-45-6789",
            &[],
            &[],
            Some("test-trace".into()),
        );
        assert!(sanitized.contains("[EMAIL_REDACTED]"));
        assert!(sanitized.contains("[SSN_REDACTED]"));
        assert!(!report.blocked);
    }

    #[test]
    fn test_guard_pipeline_check_output_blocks_harmful_content() {
        let pipeline = GuardPipeline::new();
        let (sanitized, report) = pipeline.check_output(
            "Here is how to make a bomb to hurt people",
            &[],
            &[],
            Some("test-trace".into()),
        );
        assert!(report.blocked);
        assert_eq!(sanitized, "[Content filtered due to policy violation]");
    }

    #[test]
    fn test_guard_pipeline_check_output_blocks_invalid_citations() {
        let pipeline = GuardPipeline::new();
        let chunk_ids = vec![Uuid::new_v4(), Uuid::new_v4()];
        let (sanitized, report) = pipeline.check_output(
            "Answer [citation:0] and [citation:5] text",
            &[],
            &chunk_ids,
            Some("test-trace".into()),
        );
        // Citation out of bounds should be recorded in degrade_trace
        assert!(report.blocked);
    }
}
