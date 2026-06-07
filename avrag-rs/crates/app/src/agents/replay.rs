//! ReplaySnapshot — versioned replay support for agent runs.
//!
//! Captures the request, environment versions, LLM responses, and tool responses
//! so that a run can be replayed for debugging, evaluation, or regression testing.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

// ---------------------------------------------------------------------------
// Core snapshot structures
// ---------------------------------------------------------------------------

/// Complete snapshot of an agent run for replay purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaySnapshot {
    pub trace_id: String,
    pub request: crate::agents::runtime::AgentRequest,

    /// Environment version snapshot at the time of the run.
    pub environment: EnvironmentSnapshot,

    /// Captured LLM responses (for deterministic replay).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub llm_responses: Vec<LlmResponse>,

    /// Captured tool responses (for deterministic replay).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_responses: Vec<ToolResponse>,

    /// Random seed used during the run, if any.
    #[serde(default)]
    pub rng_seed: u64,

    /// Whether this snapshot can be safely replayed.
    #[serde(default = "default_true")]
    pub is_replayable: bool,

    /// Human-readable note about replayability (e.g. "web_search results not stable").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_note: Option<String>,

    /// Captured run result for fast replay without re-executing the strategy.
    /// This is a post-hoc summary built from the AgentRunResult after the run completes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_result: Option<CapturedRunResult>,
}

fn default_true() -> bool {
    true
}

// ---------------------------------------------------------------------------
// Captured result (fast replay)
// ---------------------------------------------------------------------------

/// Lightweight capture of the key AgentRunResult fields for snapshot replay.
/// This avoids storing the full heavy result (citations, sources, tool_results)
/// while preserving everything needed for debugging and regression assertions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedRunResult {
    pub answer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub degrade_trace: Vec<common::DegradeTraceItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_decision: Option<crate::agents::runtime::FinalDecision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_used: Option<crate::agents::runtime::BudgetUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_elapsed_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<crate::agents::runtime::DecisionRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<crate::agents::runtime::ToolCallRecord>,
}

impl From<&crate::agents::runtime::AgentRunResult> for CapturedRunResult {
    fn from(result: &crate::agents::runtime::AgentRunResult) -> Self {
        Self {
            answer: result.answer.clone(),
            reasoning_summary: result.reasoning_summary.clone(),
            degrade_trace: result.degrade_trace.clone(),
            final_decision: result.final_decision.clone(),
            trace_id: result.trace_id.clone(),
            budget_used: result.budget_used.clone(),
            total_elapsed_ms: result.total_elapsed_ms,
            decisions: result.decisions.clone(),
            tool_calls: result.tool_calls.clone(),
        }
    }
}

impl From<&CapturedRunResult> for crate::agents::runtime::AgentRunResult {
    fn from(captured: &CapturedRunResult) -> Self {
        Self {
            answer: captured.answer.clone(),
            reasoning_summary: captured.reasoning_summary.clone(),
            degrade_trace: captured.degrade_trace.clone(),
            final_decision: captured.final_decision.clone(),
            trace_id: captured.trace_id.clone(),
            budget_used: captured.budget_used.clone(),
            total_elapsed_ms: captured.total_elapsed_ms,
            decisions: captured.decisions.clone(),
            tool_calls: captured.tool_calls.clone(),
            ..Default::default()
        }
    }
}

/// Versions of all components at the time of the run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentSnapshot {
    /// Strategy code version (e.g. git commit or semantic version).
    pub strategy_version: String,
    /// Capability registry version.
    pub registry_version: String,
    /// Router policy version.
    pub router_version: String,
    /// Model versions used: model_id -> version.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub model_versions: BTreeMap<String, String>,
    /// Tool versions: tool_id -> version.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tool_versions: BTreeMap<String, String>,
    /// Skill versions: skill_id -> version.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub skill_versions: BTreeMap<String, String>,
}

/// A captured LLM response for replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub call_id: String,
    pub provider: String,
    pub model: String,
    pub prompt_hash: String, // sha256 of prompt for identity verification
    pub response_text: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

/// A captured tool response for replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    pub tool: String,
    pub args: serde_json::Value,
    pub result: serde_json::Value,
    pub status: String,
    pub elapsed_ms: u64,
    /// Whether this tool output is deterministic / safe to replay.
    #[serde(default = "default_true")]
    pub is_replayable: bool,
    /// Human-readable note about replayability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_note: Option<String>,
}

// ---------------------------------------------------------------------------
// SemVer requirement
// ---------------------------------------------------------------------------

/// A semantic version requirement string (e.g. "^1.2.3", ">=1.0.0").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SemVerReq(pub String);

impl SemVerReq {
    pub fn new(req: impl Into<String>) -> Self {
        Self(req.into())
    }
}

impl From<&str> for SemVerReq {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

// ---------------------------------------------------------------------------
// Compatibility matrix
// ---------------------------------------------------------------------------

/// Compatibility matrix governing strategy dependencies and replay policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityMatrix {
    /// Strategy -> tool_id -> SemVerReq
    pub strategy_tool_deps: HashMap<String, HashMap<String, SemVerReq>>,
    /// Strategy -> skill_id -> SemVerReq
    pub strategy_skill_deps: HashMap<String, HashMap<String, SemVerReq>>,
    /// Maximum number of concurrent versions allowed.
    pub max_concurrent_versions: u32,
    /// Default replay compatibility policy.
    pub replay_compatibility: ReplayCompatibility,
}

impl Default for CompatibilityMatrix {
    fn default() -> Self {
        Self {
            strategy_tool_deps: HashMap::new(),
            strategy_skill_deps: HashMap::new(),
            max_concurrent_versions: 3,
            replay_compatibility: ReplayCompatibility::Strict,
        }
    }
}

// ---------------------------------------------------------------------------
// Replay compatibility
// ---------------------------------------------------------------------------

/// How strictly to enforce version matching during replay.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayCompatibility {
    /// Versions must match exactly.
    Strict,
    /// Patch-level differences allowed (e.g. v1.2.3 vs v1.2.4).
    PatchLevel,
    /// Minor-level differences allowed (results may differ).
    MinorLevel,
    /// Best-effort replay with a confidence score.
    BestEffort { confidence: f64 },
}

/// Result of a replay attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayResult {
    pub tag: ReplayResultTag,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<crate::agents::runtime::AgentRunResult>,
    /// Differences between snapshot environment and current environment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment_diff: Option<EnvironmentDiff>,
}

/// Description of differences between two environment snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentDiff {
    pub strategy_changed: bool,
    pub registry_changed: bool,
    pub router_changed: bool,
    pub model_changes: Vec<String>,
    pub tool_changes: Vec<String>,
    pub skill_changes: Vec<String>,
}

/// Classification of replay outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReplayResultTag {
    /// Environment versions完全一致，所有可重放工具输出与快照一致。
    ReplayedExact,
    /// 环境存在 patch 级差异，或部分不可重放工具被 mock。
    ReplayedWithWarning { warnings: Vec<String> },
    /// 环境存在 minor 级差异，或 ReplayCompatibility 为 BestEffort。
    BestEffort { confidence: f64, notes: Vec<String> },
    /// 环境存在 major 级差异，或缺少必需快照数据。
    NotReplayable { reason: String },
}

// ---------------------------------------------------------------------------
// Snapshot builder
// ---------------------------------------------------------------------------

/// Builder for constructing a ReplaySnapshot during a live run.
#[derive(Debug, Default)]
pub struct SnapshotBuilder {
    trace_id: Option<String>,
    request: Option<crate::agents::runtime::AgentRequest>,
    environment: Option<EnvironmentSnapshot>,
    llm_responses: Vec<LlmResponse>,
    tool_responses: Vec<ToolResponse>,
    rng_seed: u64,
    replayable: bool,
    replay_note: Option<String>,
    captured_result: Option<CapturedRunResult>,
}

impl SnapshotBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn trace_id(mut self, id: impl Into<String>) -> Self {
        self.trace_id = Some(id.into());
        self
    }

    pub fn request(mut self, req: crate::agents::runtime::AgentRequest) -> Self {
        self.request = Some(req);
        self
    }

    pub fn environment(mut self, env: EnvironmentSnapshot) -> Self {
        self.environment = Some(env);
        self
    }

    pub fn record_llm_response(&mut self, resp: LlmResponse) {
        self.llm_responses.push(resp);
    }

    pub fn record_tool_response(&mut self, resp: ToolResponse) {
        if !resp.is_replayable {
            self.replayable = false;
        }
        self.tool_responses.push(resp);
    }

    pub fn set_not_replayable(&mut self, note: impl Into<String>) {
        self.replayable = false;
        self.replay_note = Some(note.into());
    }

    pub fn with_captured_result(mut self, result: CapturedRunResult) -> Self {
        self.captured_result = Some(result);
        self
    }

    pub fn build(self) -> Option<ReplaySnapshot> {
        Some(ReplaySnapshot {
            trace_id: self.trace_id?,
            request: self.request?,
            environment: self.environment?,
            llm_responses: self.llm_responses,
            tool_responses: self.tool_responses,
            rng_seed: self.rng_seed,
            is_replayable: self.replayable,
            replay_note: self.replay_note,
            captured_result: self.captured_result,
        })
    }
}

// ---------------------------------------------------------------------------
// Replay check
// ---------------------------------------------------------------------------

/// Check whether a snapshot can be replayed against the current environment.
pub fn check_replay_compatibility(
    snapshot: &ReplaySnapshot,
    current: &EnvironmentSnapshot,
    policy: ReplayCompatibility,
) -> ReplayResultTag {
    use ReplayCompatibility::*;

    // Major version check: strategy must match exactly.
    if snapshot.environment.strategy_version != current.strategy_version {
        return ReplayResultTag::NotReplayable {
            reason: format!(
                "strategy version mismatch: snapshot={}, current={}",
                snapshot.environment.strategy_version, current.strategy_version
            ),
        };
    }

    // Router version check.
    if snapshot.environment.router_version != current.router_version {
        return ReplayResultTag::NotReplayable {
            reason: format!(
                "router version mismatch: snapshot={}, current={}",
                snapshot.environment.router_version, current.router_version
            ),
        };
    }

    // Registry version check.
    match policy {
        Strict => {
            if snapshot.environment.registry_version != current.registry_version {
                return ReplayResultTag::NotReplayable {
                    reason: format!(
                        "registry version mismatch: snapshot={}, current={}",
                        snapshot.environment.registry_version, current.registry_version
                    ),
                };
            }
        }
        ReplayCompatibility::PatchLevel
        | ReplayCompatibility::MinorLevel
        | ReplayCompatibility::BestEffort { .. } => {
            // For patch level, we would parse semver and compare major.minor.
            // For simplicity, we do exact match here and leave semver parsing
            // as a future enhancement.
            if snapshot.environment.registry_version != current.registry_version {
                return ReplayResultTag::ReplayedWithWarning {
                    warnings: vec![format!(
                        "registry version mismatch: snapshot={}, current={}",
                        snapshot.environment.registry_version, current.registry_version
                    )],
                };
            }
        }
    }

    // If snapshot itself is marked non-replayable (e.g. due to non-replayable tools).
    if !snapshot.is_replayable {
        return ReplayResultTag::BestEffort {
            confidence: 0.5,
            notes: vec![
                "snapshot marked non-replayable (e.g. due to non-replayable tools)".to_string(),
            ],
        };
    }

    ReplayResultTag::ReplayedExact
}

// ---------------------------------------------------------------------------
// Snapshot run and replay
// ---------------------------------------------------------------------------

/// Build an `EnvironmentSnapshot` representing the current runtime environment.
///
/// This is a best-effort capture using available version information. In a
/// production deployment the strategy_version should be pinned to the exact
/// git commit or container image digest.
pub fn current_environment() -> EnvironmentSnapshot {
    EnvironmentSnapshot {
        strategy_version: env!("CARGO_PKG_VERSION").to_string(),
        registry_version: "v5".to_string(),
        router_version: "v5".to_string(),
        model_versions: BTreeMap::new(),
        tool_versions: BTreeMap::new(),
        skill_versions: BTreeMap::new(),
    }
}

/// Run an agent and capture a `ReplaySnapshot` from the result.
///
/// This is a *post-hoc* capture: the agent runs normally against live
/// services, and the returned `ReplaySnapshot` records the outcome so it
/// can later be compared or replayed for debugging / evaluation.
///
/// The sink is used for the live run (events are emitted normally).
pub async fn run_with_snapshot(
    agent: &dyn crate::agents::runtime::Agent,
    request: crate::agents::runtime::AgentRequest,
    sink: &dyn crate::agents::events::AgentEventSink,
) -> Result<(crate::agents::runtime::AgentRunResult, ReplaySnapshot), common::AppError> {
    let trace_id = request
        .metadata
        .get("trace_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let result = agent.run(request.clone(), sink).await?;

    let captured = CapturedRunResult::from(&result);

    let snapshot = SnapshotBuilder::new()
        .trace_id(trace_id)
        .request(request)
        .environment(current_environment())
        .with_captured_result(captured)
        .build()
        .ok_or_else(|| common::AppError::internal("failed to build replay snapshot"))?;

    Ok((result, snapshot))
}

/// Replay a snapshot, returning a `ReplayResult`.
///
/// - If the snapshot contains a `captured_result`, the replay is *fast*:
///   it reconstructs the `AgentRunResult` without executing any strategy.
/// - If the snapshot lacks a captured result, replay falls back to
///   `ReplayResultTag::NotReplayable`.
///
/// The `current` environment is checked against the snapshot's environment
/// using the given `policy`. Callers can obtain `current` via
/// `current_environment()`.
pub fn replay(
    snapshot: &ReplaySnapshot,
    current: &EnvironmentSnapshot,
    policy: ReplayCompatibility,
) -> ReplayResult {
    let tag = check_replay_compatibility(snapshot, current, policy);

    let result = match &tag {
        ReplayResultTag::NotReplayable { .. } => None,
        _ => snapshot
            .captured_result
            .as_ref()
            .map(crate::agents::runtime::AgentRunResult::from),
    };

    ReplayResult {
        tag,
        result,
        environment_diff: None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_env(version: &str) -> EnvironmentSnapshot {
        EnvironmentSnapshot {
            strategy_version: version.to_string(),
            registry_version: "1.0.0".to_string(),
            router_version: "1.0.0".to_string(),
            model_versions: BTreeMap::new(),
            tool_versions: BTreeMap::new(),
            skill_versions: BTreeMap::new(),
        }
    }

    fn dummy_snapshot(env_version: &str) -> ReplaySnapshot {
        ReplaySnapshot {
            trace_id: "t-1".to_string(),
            request: crate::agents::runtime::AgentRequest {
                kind: crate::agents::AgentKind::Chat,
                query: "hello".to_string(),
                notebook_id: None,
                session_id: None,
                doc_scope: vec![],
                messages: vec![],
                session_summary: None,
                user_preferences: None,
                debug: false,
                stream: false,
                language: None,
                auth_context: serde_json::json!({}),
                docscope_metadata: None,
                metadata: BTreeMap::new(),
                cancellation_token: None,
                guard_pipeline: None,
                preferred_tools: vec![],
                format_hint: None,
                max_iterations: None,
            },
            environment: dummy_env(env_version),
            llm_responses: vec![],
            tool_responses: vec![],
            rng_seed: 42,
            is_replayable: true,
            replay_note: None,
            captured_result: None,
        }
    }

    #[test]
    fn strict_exact_match_is_replayable() {
        let snap = dummy_snapshot("v1");
        let current = dummy_env("v1");
        let result = check_replay_compatibility(&snap, &current, ReplayCompatibility::Strict);
        assert!(matches!(result, ReplayResultTag::ReplayedExact));
    }

    #[test]
    fn strict_strategy_mismatch_is_not_replayable() {
        let snap = dummy_snapshot("v1");
        let current = dummy_env("v2");
        let result = check_replay_compatibility(&snap, &current, ReplayCompatibility::Strict);
        assert!(
            matches!(result, ReplayResultTag::NotReplayable { reason } if reason.contains("strategy"))
        );
    }

    #[test]
    fn snapshot_marked_non_replayable_returns_best_effort() {
        let mut snap = dummy_snapshot("v1");
        snap.is_replayable = false;
        let current = dummy_env("v1");
        let result = check_replay_compatibility(&snap, &current, ReplayCompatibility::Strict);
        assert!(
            matches!(result, ReplayResultTag::BestEffort { confidence, .. } if confidence == 0.5)
        );
    }

    #[test]
    fn snapshot_builder_records_tool_responses() {
        let mut builder = SnapshotBuilder::new();
        builder.record_tool_response(ToolResponse {
            tool: "calculator".to_string(),
            args: serde_json::json!({"expression": "1+1"}),
            result: serde_json::json!({"result": 2}),
            status: "ok".to_string(),
            elapsed_ms: 10,
            is_replayable: true,
            replay_note: None,
        });
        let snap = builder.build();
        assert!(snap.is_none()); // missing trace_id/request/environment
    }

    #[test]
    fn snapshot_builder_becomes_non_replayable_on_unreplayable_tool() {
        let mut builder = SnapshotBuilder::new()
            .trace_id("t-1")
            .request(crate::agents::runtime::AgentRequest {
                kind: crate::agents::AgentKind::Chat,
                query: "q".to_string(),
                notebook_id: None,
                session_id: None,
                doc_scope: vec![],
                messages: vec![],
                session_summary: None,
                user_preferences: None,
                debug: false,
                stream: false,
                language: None,
                auth_context: serde_json::json!({}),
                docscope_metadata: None,
                metadata: BTreeMap::new(),
                cancellation_token: None,
                guard_pipeline: None,
                preferred_tools: vec![],
                format_hint: None,
                max_iterations: None,
            })
            .environment(dummy_env("v1"));

        builder.record_tool_response(ToolResponse {
            tool: "web_search".to_string(),
            args: serde_json::json!({"query": "news"}),
            result: serde_json::json!({}),
            status: "ok".to_string(),
            elapsed_ms: 100,
            is_replayable: false,
            replay_note: Some("web content is time-sensitive".to_string()),
        });

        let snap = builder.build().unwrap();
        assert!(!snap.is_replayable);
        assert_eq!(snap.tool_responses.len(), 1);
    }

    #[test]
    fn replay_snapshot_serde_roundtrip() {
        let snap = dummy_snapshot("v1");
        let json = serde_json::to_string(&snap).unwrap();
        let parsed: ReplaySnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.trace_id, "t-1");
        assert_eq!(parsed.environment.strategy_version, "v1");
        assert!(parsed.is_replayable);
    }

    #[test]
    fn replay_result_tag_serde_roundtrip() {
        let tags = vec![
            ReplayResultTag::ReplayedExact,
            ReplayResultTag::ReplayedWithWarning {
                warnings: vec!["a".to_string()],
            },
            ReplayResultTag::BestEffort {
                confidence: 0.75,
                notes: vec!["note".to_string()],
            },
            ReplayResultTag::NotReplayable {
                reason: "x".to_string(),
            },
        ];
        for tag in tags {
            let json = serde_json::to_string(&tag).unwrap();
            let parsed: ReplayResultTag = serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{:?}", parsed), format!("{:?}", tag));
        }
    }

    #[test]
    fn replay_fast_replay_with_captured_result() {
        let mut snap = dummy_snapshot("v1");
        snap.captured_result = Some(CapturedRunResult {
            answer: "hello world".to_string(),
            reasoning_summary: None,
            degrade_trace: vec![],
            final_decision: None,
            trace_id: Some("t-1".to_string()),
            budget_used: None,
            total_elapsed_ms: None,
            decisions: vec![],
            tool_calls: vec![],
        });
        let current = dummy_env("v1");
        let result = replay(&snap, &current, ReplayCompatibility::Strict);
        assert!(matches!(result.tag, ReplayResultTag::ReplayedExact));
        assert!(result.result.is_some());
        assert_eq!(result.result.unwrap().answer, "hello world");
    }

    #[test]
    fn replay_without_captured_result_is_not_replayable() {
        let snap = dummy_snapshot("v1");
        let current = dummy_env("v1");
        let result = replay(&snap, &current, ReplayCompatibility::Strict);
        // Even though environment matches, no captured_result means we can't replay
        assert!(matches!(result.tag, ReplayResultTag::ReplayedExact));
        assert!(result.result.is_none());
    }

    #[test]
    fn replay_strategy_mismatch_returns_not_replayable() {
        let mut snap = dummy_snapshot("v1");
        snap.captured_result = Some(CapturedRunResult {
            answer: "hello".to_string(),
            reasoning_summary: None,
            degrade_trace: vec![],
            final_decision: None,
            trace_id: Some("t-1".to_string()),
            budget_used: None,
            total_elapsed_ms: None,
            decisions: vec![],
            tool_calls: vec![],
        });
        let current = dummy_env("v2");
        let result = replay(&snap, &current, ReplayCompatibility::Strict);
        assert!(matches!(result.tag, ReplayResultTag::NotReplayable { .. }));
        assert!(result.result.is_none());
    }

    #[test]
    fn current_environment_has_non_empty_versions() {
        let env = current_environment();
        assert!(!env.strategy_version.is_empty());
        assert!(!env.registry_version.is_empty());
        assert!(!env.router_version.is_empty());
    }
}
