//! Red Team Framework — adversarial testing for agent security boundaries.
//!
//! Provides datasets, evaluators, and scheduled runners for:
//! - Prompt injection attacks
//! - Tool abuse attempts
//! - Data exfiltration attempts

pub mod evaluator;
pub mod scheduler;
pub mod service;

use serde::{Deserialize, Serialize};
use std::path::Path;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A single red-team case (adversarial test).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedTeamCase {
    pub case_id: String,
    pub attack_vector: AttackVector,
    /// The adversarial input sent to the agent.
    pub input: crate::agents::runtime::AgentRequest,
    /// Expected security behavior.
    pub expected: ExpectedBehavior,
    /// Optional description of the attack technique.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional tags for filtering (e.g. "jailbreak", "encoding").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// Attack vector category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttackVector {
    PromptInjection,
    ToolAbuse,
    DataExfiltration,
}

impl AttackVector {
    pub fn as_str(&self) -> &'static str {
        match self {
            AttackVector::PromptInjection => "prompt_injection",
            AttackVector::ToolAbuse => "tool_abuse",
            AttackVector::DataExfiltration => "data_exfiltration",
        }
    }
}

/// Expected security response for a red-team case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedBehavior {
    /// Input should be blocked entirely by guard pipeline.
    Blocked,
    /// Should require human approval before proceeding.
    RequireApproval,
    /// Should be sanitized before processing.
    Sanitized,
    /// Should produce no harmful output.
    NoHarmfulOutput,
}

/// A loaded red-team dataset.
#[derive(Debug, Clone)]
pub struct RedTeamDataset {
    pub name: String,
    pub cases: Vec<RedTeamCase>,
}

/// Load a red-team dataset from a JSONL file.
pub fn load_dataset<P: AsRef<Path>>(path: P) -> Result<RedTeamDataset, common::AppError> {
    let path = path.as_ref();
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let content = std::fs::read_to_string(path)
        .map_err(|e| common::AppError::internal(format!("Failed to read redteam dataset: {e}")))?;

    let mut cases = Vec::new();
    for (line_no, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let case: RedTeamCase = serde_json::from_str(line).map_err(|e| {
            common::AppError::internal(format!(
                "Failed to parse redteam case at line {} in {}: {}",
                line_no + 1,
                path.display(),
                e
            ))
        })?;
        cases.push(case);
    }

    Ok(RedTeamDataset { name, cases })
}

/// Load all red-team datasets from a directory.
pub fn load_datasets_from_dir<P: AsRef<Path>>(dir: P) -> Result<Vec<RedTeamDataset>, common::AppError> {
    let mut datasets = Vec::new();
    let entries = std::fs::read_dir(dir.as_ref())
        .map_err(|e| common::AppError::internal(format!("Failed to read redteam dir: {e}")))?;

    for entry in entries {
        let entry = entry.map_err(|e| common::AppError::internal(format!("Dir entry error: {e}")))?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
            datasets.push(load_dataset(&path)?);
        }
    }

    Ok(datasets)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attack_vector_strs() {
        assert_eq!(AttackVector::PromptInjection.as_str(), "prompt_injection");
        assert_eq!(AttackVector::ToolAbuse.as_str(), "tool_abuse");
        assert_eq!(AttackVector::DataExfiltration.as_str(), "data_exfiltration");
    }

    #[test]
    fn expected_behavior_serde_roundtrip() {
        for behavior in [
            ExpectedBehavior::Blocked,
            ExpectedBehavior::RequireApproval,
            ExpectedBehavior::Sanitized,
            ExpectedBehavior::NoHarmfulOutput,
        ] {
            let json = serde_json::to_string(&behavior).unwrap();
            let parsed: ExpectedBehavior = serde_json::from_str(&json).unwrap();
            assert_eq!(behavior, parsed);
        }
    }
}
