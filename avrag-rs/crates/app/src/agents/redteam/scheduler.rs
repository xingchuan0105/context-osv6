//! Red Team Scheduler — periodic adversarial testing runner.
//!
//! Supports pre-release (full dataset) and post-release (random sample)
//! execution schedules.

use super::{RedTeamCase, RedTeamDataset};
use crate::agents::eval_framework::EvalRun;
use common::AppError;

// ---------------------------------------------------------------------------
// Schedule configuration
// ---------------------------------------------------------------------------

/// When and how to run red-team tests.
#[derive(Debug, Clone)]
pub enum RedTeamSchedule {
    /// Run the full dataset (pre-release gate).
    Full,
    /// Run a random sample of N cases (weekly regression).
    RandomSample { count: usize },
    /// Run only cases matching given tags.
    Tagged { tags: Vec<String> },
}

impl Default for RedTeamSchedule {
    fn default() -> Self {
        RedTeamSchedule::RandomSample { count: 100 }
    }
}

// ---------------------------------------------------------------------------
// Scheduler
// ---------------------------------------------------------------------------

/// Selects cases from a dataset according to a schedule.
pub struct RedTeamScheduler;

impl RedTeamScheduler {
    /// Select cases from a dataset based on the schedule configuration.
    pub fn select_cases(
        &self,
        dataset: &RedTeamDataset,
        schedule: &RedTeamSchedule,
    ) -> Vec<RedTeamCase> {
        match schedule {
            RedTeamSchedule::Full => dataset.cases.clone(),
            RedTeamSchedule::RandomSample { count } => {
                let sample_size = (*count).min(dataset.cases.len());
                if sample_size == dataset.cases.len() {
                    return dataset.cases.clone();
                }
                // Deterministic sampling: pick every N-th case based on hash.
                let mut indexed: Vec<(usize, u64)> = dataset
                    .cases
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (i, hash_case_id(&c.case_id)))
                    .collect();
                indexed.sort_by(|a, b| a.1.cmp(&b.1));
                indexed.truncate(sample_size);
                indexed.sort_by(|a, b| a.0.cmp(&b.0)); // restore original order
                indexed
                    .into_iter()
                    .map(|(i, _)| dataset.cases[i].clone())
                    .collect()
            }
            RedTeamSchedule::Tagged { tags } => {
                let tag_set: std::collections::HashSet<_> = tags.iter().collect();
                dataset
                    .cases
                    .iter()
                    .filter(|c| c.tags.iter().any(|t| tag_set.contains(t)))
                    .cloned()
                    .collect()
            }
        }
    }

    /// Build a sub-dataset from selected cases.
    pub fn build_sub_dataset(
        &self,
        dataset: &RedTeamDataset,
        schedule: &RedTeamSchedule,
    ) -> RedTeamDataset {
        let cases = self.select_cases(dataset, schedule);
        let suffix = match schedule {
            RedTeamSchedule::Full => "full".to_string(),
            RedTeamSchedule::RandomSample { count } => {
                format!("sample-{}", (*count).min(cases.len()))
            }
            RedTeamSchedule::Tagged { tags } => format!("tagged-{}", tags.join(",")),
        };
        RedTeamDataset {
            name: format!("{}-{}", dataset.name, suffix),
            cases,
        }
    }
}

/// Simple deterministic hash of a case ID for sampling.
fn hash_case_id(case_id: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    case_id.hash(&mut hasher);
    hasher.finish()
}

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

/// Run red-team evaluation with a schedule.
pub async fn run_scheduled_redteam(
    datasets: &[RedTeamDataset],
    schedule: &RedTeamSchedule,
    agent: &dyn crate::agents::runtime::Agent,
    sink: &dyn crate::agents::events::AgentEventSink,
) -> Result<Vec<EvalRun>, AppError> {
    let scheduler = RedTeamScheduler;
    let mut results = Vec::new();

    for dataset in datasets {
        let sub = scheduler.build_sub_dataset(dataset, schedule);
        if sub.cases.is_empty() {
            tracing::info!(dataset = %dataset.name, "No cases matched schedule, skipping");
            continue;
        }
        tracing::info!(
            dataset = %dataset.name,
            selected = sub.cases.len(),
            "Running red-team evaluation"
        );
        let run = super::evaluator::run_redteam_evaluation(&sub, agent, sink).await?;
        results.push(run);
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

/// Simple text report of red-team results.
pub fn format_report(results: &[EvalRun]) -> String {
    let mut lines = vec!["# Red Team Evaluation Report".to_string()];
    lines.push("".to_string());

    for run in results {
        lines.push(format!("## {}", run.run_name));
        if let Some(summary) = &run.summary {
            lines.push(format!("- Total cases: {}", summary.total_cases));
            lines.push(format!("- Passed: {}", summary.passed_cases));
            lines.push(format!("- Failed: {}", summary.failed_cases));
            lines.push(format!("- Overall score: {:.2}", summary.overall_score));
        }
        lines.push("".to_string());
    }

    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_dataset(n: usize) -> RedTeamDataset {
        let cases = (0..n)
            .map(|i| RedTeamCase {
                case_id: format!("c{}", i),
                attack_vector: super::super::AttackVector::PromptInjection,
                input: crate::agents::runtime::AgentRequest {
                    kind: crate::agents::AgentKind::Chat,
                    query: format!("q{}", i),
                    resolved_query: format!("q{}", i),
                    query_resolution: None,
                    notebook_id: None,
                    session_id: None,
                    doc_scope: vec![],
                    messages: vec![],
                    user_preferences: None,
                    debug: false,
                    stream: false,
                    language: None,
                    auth_context: serde_json::json!({}),
                    docscope_metadata: None,
                    metadata: Default::default(),
                    cancellation_token: None,
                    guard_pipeline: None,
                    preferred_tools: vec![],
                    format_hint: None,
                    max_iterations: None,
                },
                expected: super::super::ExpectedBehavior::Blocked,
                description: None,
                tags: if i % 2 == 0 {
                    vec!["jailbreak".to_string()]
                } else {
                    vec!["encoding".to_string()]
                },
            })
            .collect();
        RedTeamDataset {
            name: "test".to_string(),
            cases,
        }
    }

    #[test]
    fn scheduler_full_selects_all() {
        let dataset = dummy_dataset(10);
        let scheduler = RedTeamScheduler;
        let selected = scheduler.select_cases(&dataset, &RedTeamSchedule::Full);
        assert_eq!(selected.len(), 10);
    }

    #[test]
    fn scheduler_sample_selects_subset() {
        let dataset = dummy_dataset(10);
        let scheduler = RedTeamScheduler;
        let selected =
            scheduler.select_cases(&dataset, &RedTeamSchedule::RandomSample { count: 5 });
        assert_eq!(selected.len(), 5);
    }

    #[test]
    fn scheduler_sample_respects_dataset_size() {
        let dataset = dummy_dataset(3);
        let scheduler = RedTeamScheduler;
        let selected =
            scheduler.select_cases(&dataset, &RedTeamSchedule::RandomSample { count: 100 });
        assert_eq!(selected.len(), 3);
    }

    #[test]
    fn scheduler_tagged_filters() {
        let dataset = dummy_dataset(10);
        let scheduler = RedTeamScheduler;
        let selected = scheduler.select_cases(
            &dataset,
            &RedTeamSchedule::Tagged {
                tags: vec!["jailbreak".to_string()],
            },
        );
        assert_eq!(selected.len(), 5); // even indices have jailbreak
    }

    #[test]
    fn format_report_contains_scores() {
        let run = EvalRun {
            run_id: "r1".to_string(),
            run_name: "test-run".to_string(),
            strategy: "RedTeam".to_string(),
            strategy_version: "v1".to_string(),
            started_at_ms: 0,
            completed_at_ms: Some(0),
            cases: vec![],
            summary: Some(crate::agents::eval_framework::EvalSummary {
                total_cases: 10,
                passed_cases: 8,
                failed_cases: 2,
                metric_averages: Default::default(),
                overall_score: 0.8,
            }),
            trigger: None,
            failures: vec![],
        };
        let report = format_report(&[run]);
        assert!(report.contains("test-run"));
        assert!(report.contains("0.80"));
    }
}
