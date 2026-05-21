//! Red Team Service — callable integration point for adversarial evaluation.
//!
//! Wraps the redteam framework (dataset loading + scheduler + evaluator) into
//! a service that can be invoked from admin endpoints or background jobs.

use super::{RedTeamDataset, load_datasets_from_dir};
use super::scheduler::RedTeamSchedule;
use crate::agents::events::AgentEventSink;
use crate::agents::eval_framework::EvalRun;
use crate::agents::runtime::Agent;
use common::AppError;
use std::path::PathBuf;

/// Service that runs red-team evaluation against an agent.
pub struct RedTeamService {
    agent: Box<dyn Agent>,
    dataset_dir: Option<PathBuf>,
}

impl RedTeamService {
    /// Create a new RedTeamService with the given agent.
    pub fn new(agent: Box<dyn Agent>) -> Self {
        Self {
            agent,
            dataset_dir: None,
        }
    }

    /// Set the directory from which to load red-team datasets.
    pub fn with_dataset_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.dataset_dir = Some(dir.into());
        self
    }

    /// Run red-team evaluation with the given schedule.
    ///
    /// Loads datasets from the configured directory (if any), filters them
    /// according to `schedule`, runs each case through the agent, and
    /// evaluates results with the redteam evaluator.
    pub async fn run_evaluation(
        &self,
        schedule: &RedTeamSchedule,
        sink: &dyn AgentEventSink,
    ) -> Result<Vec<EvalRun>, AppError> {
        let datasets = self.load_datasets().await?;
        if datasets.is_empty() {
            tracing::warn!("No red-team datasets loaded; skipping evaluation");
            return Ok(Vec::new());
        }

        super::scheduler::run_scheduled_redteam(&datasets, schedule, self.agent.as_ref(), sink)
            .await
    }

    /// Run evaluation and return a formatted text report.
    pub async fn run_evaluation_report(
        &self,
        schedule: &RedTeamSchedule,
        sink: &dyn AgentEventSink,
    ) -> Result<String, AppError> {
        let runs = self.run_evaluation(schedule, sink).await?;
        Ok(super::scheduler::format_report(&runs))
    }

    async fn load_datasets(&self) -> Result<Vec<RedTeamDataset>, AppError> {
        match &self.dataset_dir {
            Some(dir) => {
                if !dir.exists() {
                    tracing::warn!(dir = %dir.display(), "Red-team dataset directory does not exist");
                    return Ok(Vec::new());
                }
                load_datasets_from_dir(dir)
            }
            None => Ok(Vec::new()),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::events::CollectingSink;
    use crate::agents::runtime::{AgentRequest, AgentRunResult};
    use async_trait::async_trait;

    struct StubAgent;

    #[async_trait]
    impl Agent for StubAgent {
        async fn run(
            &self,
            _request: AgentRequest,
            _sink: &dyn AgentEventSink,
        ) -> Result<AgentRunResult, AppError> {
            Ok(AgentRunResult {
                answer: "safe response".to_string(),
                ..Default::default()
            })
        }
    }

    #[tokio::test]
    async fn service_with_no_dataset_dir_returns_empty() {
        let svc = RedTeamService::new(Box::new(StubAgent));
        let sink = CollectingSink::new();
        let runs = svc.run_evaluation(&RedTeamSchedule::Full, &sink).await.unwrap();
        assert!(runs.is_empty());
    }

    #[tokio::test]
    async fn service_with_missing_dataset_dir_returns_empty() {
        let svc = RedTeamService::new(Box::new(StubAgent))
            .with_dataset_dir("/nonexistent/redteam/dir");
        let sink = CollectingSink::new();
        let runs = svc.run_evaluation(&RedTeamSchedule::Full, &sink).await.unwrap();
        assert!(runs.is_empty());
    }

    #[tokio::test]
    async fn service_report_returns_string() {
        let svc = RedTeamService::new(Box::new(StubAgent));
        let sink = CollectingSink::new();
        let report = svc
            .run_evaluation_report(&RedTeamSchedule::Full, &sink)
            .await
            .unwrap();
        assert!(report.contains("Red Team Evaluation Report"));
    }
}
