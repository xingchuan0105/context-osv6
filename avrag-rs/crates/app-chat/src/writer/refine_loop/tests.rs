//! Tests for the WriteRefine ReAct loop.
//!
//! Tests for types, helpers, handlers, and round-counter output.

use super::types::{
    BestSnapshot, FinishReason, RefineContext, RefineLoopBudget,
    WRITE_REFINE_HARD_REACT_CAP, WRITE_REFINE_GATE_MAX_REVISE,
};
use super::WriteRefineLoopRunner;
use super::helpers::{
    build_write_refine_round_counter_zh, strip_task_section,
};

use heavytail::diagnosis::diagnose_pre_refine;
use heavytail::feedforward::fingerprint_workspace;
use heavytail::score::composite;
use heavytail::state::{WriterBudget, WriterState};
use heavytail::StyleParams;
use heavytail::workspace::{DraftWorkspace, ParagraphRecord, RhythmMode, SentenceRecord};
use heavytail::workspace::SentenceId;

use crate::writer::material_pack::MaterialPack;

fn make_workspace() -> DraftWorkspace {
    let mut ws = DraftWorkspace::default();
    ws.sentences = vec![
        SentenceRecord {
            id: SentenceId("s01".into()),
            text: "这是一句长度恰好二十字左右的示例句子。".into(),
            para: 0,
            tombstone: false,
        },
        SentenceRecord {
            id: SentenceId("s02".into()),
            text: "这是另一句差不多长度的中文示例句子。".into(),
            para: 0,
            tombstone: false,
        },
    ];
    ws.paragraphs = vec![ParagraphRecord {
        idx: 0,
        rhythm: RhythmMode::Mixed,
    }];
    ws
}

#[test]
fn refine_loop_budget_defaults_match_plan() {
    let b = RefineLoopBudget::default();
    assert_eq!(b.max_rounds, 5);
    assert_eq!(b.max_react_iterations, WRITE_REFINE_HARD_REACT_CAP);
    assert_eq!(b.max_on_demand_research, 5);
    assert_eq!(b.per_research_worker_tokens, 4_000);
    assert_eq!(b.max_refine_tokens, 40_000);
}

#[test]
fn refine_loop_budget_from_writer_budget() {
    let writer = WriterBudget::default();
    let b = RefineLoopBudget::from_writer_budget(&writer, WRITE_REFINE_HARD_REACT_CAP);
    assert_eq!(b.max_rounds, writer.max_rounds);
    assert_eq!(b.max_react_iterations, WRITE_REFINE_HARD_REACT_CAP);
}

#[test]
fn unlimited_budget_still_caps_react_iterations() {
    let b = RefineLoopBudget::unlimited();
    assert_eq!(b.max_react_iterations, WRITE_REFINE_HARD_REACT_CAP);
    assert_eq!(b.max_rounds, WRITE_REFINE_GATE_MAX_REVISE);
    assert!(b.react_iterations_capped());
    assert!(b.revise_rounds_capped());
}

#[test]
fn write_refine_round_counter_shows_remaining_and_last_round_hint() {
    let budget = RefineLoopBudget::unlimited();
    let mid = build_write_refine_round_counter_zh(
        2,
        6,
        1,
        WRITE_REFINE_GATE_MAX_REVISE,
        0,
        usize::MAX,
        &budget,
    );
    assert!(mid.contains("第 3 / 6 轮"));
    assert!(mid.contains("剩余 3 轮"));
    assert!(mid.contains("<write_refine_round"));

    let last = build_write_refine_round_counter_zh(
        5,
        6,
        3,
        WRITE_REFINE_GATE_MAX_REVISE,
        1,
        usize::MAX,
        &budget,
    );
    assert!(last.contains("最后一轮"));
    assert!(last.contains("remaining=\"0\""));
}

#[test]
fn refine_context_checkpoint_writes_artifacts() {
    // P2.7: refine checkpoint persists counters + workspace + material pack.
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut ctx = make_ctx();
    ctx.revise_rounds_used = 2;
    ctx.research_calls_used = 1;
    ctx.tokens_used = 1234;

    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "heavytail-refine-ckpt-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    ctx.checkpoint(&dir).expect("checkpoint writes");

    let context = std::fs::read_to_string(dir.join("refine").join("context.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&context).unwrap();
    assert_eq!(json["revise_rounds_used"], 2);
    assert_eq!(json["research_calls_used"], 1);
    assert_eq!(json["tokens_used"], 1234);
    assert!(json["workspace"].is_object());
    assert!(dir.join("refine").join("material_pack.json").is_file());
}

#[test]
fn refine_context_new_initializes_counters() {
    let ws = make_workspace();
    let style = StyleParams::default();
    let diag = diagnose_pre_refine(&ws, &style, &[]);
    let pack = MaterialPack::default();
    let ctx = RefineContext::new(ws, diag, pack, None);
    assert_eq!(ctx.research_calls_used, 0);
    assert_eq!(ctx.revise_rounds_used, 0);
    assert_eq!(ctx.react_iteration, 0);
    assert_eq!(ctx.tokens_used, 0);
    assert!(ctx.finish_reason.is_none());
}

#[test]
fn refine_context_recompute_updates_bands_satisfied() {
    let ws = make_workspace();
    let style = StyleParams::default();
    let diag = diagnose_pre_refine(&ws, &style, &[]);
    let pack = MaterialPack::default();
    let mut ctx = RefineContext::new(ws, diag, pack, None);
    // ctx.bands_satisfied starts false (from RefineContext::new).
    assert!(!ctx.bands_satisfied);
    // After recompute, bands_satisfied reflects the validation of the
    // current workspace (a uniform-length draft should not pass all bands).
    ctx.recompute(&style, &[]);
    assert_eq!(ctx.bands_satisfied, ctx.diagnosis.validation.passed);
}

#[test]
fn strip_task_section_removes_task_heading() {
    let brief = "## 指标说明\n\nstuff\n\n## 你的任务\n\nDo things.";
    let stripped = strip_task_section(brief);
    assert!(stripped.contains("指标说明"));
    assert!(!stripped.contains("你的任务"));
}

#[test]
fn strip_task_section_preserves_when_no_task() {
    let brief = "## 指标说明\n\nstuff";
    let stripped = strip_task_section(brief);
    assert_eq!(stripped, brief);
}

// ── Phase 4: handler-level unit tests ──────────────────────────────────

use std::collections::BTreeMap;
use std::sync::Arc;

use avrag_llm::LlmClient;
use avrag_llm::ModelProviderConfig;

use contracts::ToolCall;
use contracts::chat::ToolStatus;

use crate::agents::events::{AgentEventSink, NoopSink};
use crate::agents::runtime::{Agent, AgentRequest, AgentRunResult};
use crate::agents::AgentKind;
use crate::writer::invoker::SubagentInvoker;

/// Stub `Agent` that panics if its `run` is ever called.
/// Used to verify the budget-exhausted path never reaches the sub-worker.
struct NeverCalledAgent;

#[async_trait::async_trait]
impl Agent for NeverCalledAgent {
    async fn run(
        &self,
        _request: AgentRequest,
        _sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, common::AppError> {
        panic!("NeverCalledAgent::run should never be invoked for budget-exhausted path");
    }
}

fn dummy_llm_client() -> LlmClient {
    LlmClient::new(ModelProviderConfig {
        base_url: "http://localhost".to_string(),
        api_key: "dummy".to_string(),
        model: "test-model".to_string(),
        timeout_ms: 1000,
        api_style: None,
        dimensions: None,
        enable_thinking: None,
        enable_cache: None,
        rpm_limit: None,
        tpm_limit: None,
    })
}

fn test_parent_request() -> AgentRequest {
    AgentRequest {
        kind: AgentKind::Write,
        query: "test topic".to_string(),
        notebook_id: None,
        session_id: Some("test-session".to_string()),
        doc_scope: vec![],
        messages: vec![],
        user_preferences: None,
        debug: false,
        stream: false,
        language: Some("zh".to_string()),
        auth_context: serde_json::json!({}),
        docscope_metadata: None,
        metadata: BTreeMap::new(),
        cancellation_token: None,
        guard_pipeline: None,
        preferred_tools: vec![],
        format_hint: None,
        max_iterations: None,
    }
}

fn test_runner<'a>(budget: RefineLoopBudget) -> WriteRefineLoopRunner<'a> {
    let llm = heavytail::llm::WriterLlm::from_client(dummy_llm_client());
    let service = Arc::new(crate::agents::service::UnifiedAgentService::new(
        Box::new(NeverCalledAgent),
    ));
    let invoker = SubagentInvoker::new(service, None);
    // SAFETY: we leak the locals so the runner can borrow them for the
    // duration of the test. The test is short-lived and the leaked
    // memory is reclaimed when the process exits.
    let llm: &'a heavytail::llm::WriterLlm = Box::leak(Box::new(llm));
    let invoker: &'a SubagentInvoker = Box::leak(Box::new(invoker));
    let req: &'a AgentRequest = Box::leak(Box::new(test_parent_request()));
    WriteRefineLoopRunner::new(llm, invoker, req, StyleParams::default(), budget)
}

fn make_ctx() -> RefineContext {
    let ws = make_workspace();
    let style = StyleParams::default();
    let diag = diagnose_pre_refine(&ws, &style, &[]);
    RefineContext::new(ws, diag, MaterialPack::default(), None)
}

fn research_call(kind: &str, query: &str) -> ToolCall {
    ToolCall {
        tool: "write_refine_research".to_string(),
        version: "1".to_string(),
        args: serde_json::json!({
            "kind": kind,
            "query": query,
            "reason": "test",
        }),
    }
}

fn finish_call(reason: &str, bands_satisfied: bool) -> ToolCall {
    ToolCall {
        tool: "write_refine_finish".to_string(),
        version: "1".to_string(),
        args: serde_json::json!({
            "reason": reason,
            "bands_satisfied": bands_satisfied,
        }),
    }
}

#[tokio::test]
async fn handle_revise_counts_effective_round_and_tracks_best_snapshot() {
    // P1.1 + P1.3: an effective revise (≥1 sentence changed) counts as a
    // round, is recorded into state, and updates the best-version snapshot.
    let runner = test_runner(RefineLoopBudget::default());
    let mut ctx = make_ctx();
    let mut state = WriterState::default();

    // Seed best_snapshot like run() does with the diagnosed initial draft.
    let init_fp = fingerprint_workspace(&ctx.workspace);
    let init_score = composite(&init_fp, &StyleParams::default()).s;
    ctx.best_snapshot = Some(BestSnapshot {
        score: init_score,
        workspace: ctx.workspace.clone(),
    });

    let call = ToolCall {
        tool: "write_refine_revise".to_string(),
        version: "1".to_string(),
        args: serde_json::json!({
            "patches": [{ "id": "s01", "text": "这是一句被彻底改写过的全新句子。" }]
        }),
    };
    let result = runner
        .handle_revise(&call, &mut ctx, &[], &mut state)
        .await;

    assert_eq!(result.status, ToolStatus::Ok);
    // Effective revise counted.
    assert_eq!(ctx.revise_rounds_used, 1);
    // Round recorded into state for checkpoint/telemetry fidelity.
    assert_eq!(state.rounds.len(), 1);
    assert_eq!(state.workspace.sentences[0].text, "这是一句被彻底改写过的全新句子。");
    // P1.1 invariant: best_snapshot retains the HIGHER of (initial, revise).
    // A revise that lowers S must not overwrite the better snapshot — that
    // snapshot is what gets restored at loop exit.
    let new_score = ctx.diagnosis.score_s;
    assert_eq!(
        ctx.best_snapshot.as_ref().unwrap().score,
        init_score.max(new_score)
    );
}

#[tokio::test]
async fn handle_research_6th_call_returns_budget_exhausted_without_invoking_worker() {
    // budget cap = 5; pre-set research_calls_used = 5 to simulate the 6th call.
    let runner = test_runner(RefineLoopBudget::default());
    let mut ctx = make_ctx();
    ctx.research_calls_used = 5;

    let call = research_call("web", "test query for 6th call");
    let sink = NoopSink;
    let result = runner.handle_research(&call, &mut ctx, &sink).await;

    assert_eq!(result.status, ToolStatus::Ok);
    let data = result.data.unwrap();
    assert_eq!(data["budget_exhausted"], true);
    assert_eq!(data["research_calls_used"], 5);
    assert_eq!(data["new_cards"].as_array().unwrap().len(), 0);
    // research_calls_used must NOT increment on a rejected call.
    assert_eq!(ctx.research_calls_used, 5);
}

#[tokio::test]
async fn handle_finish_returns_validation_warning_when_bands_not_satisfied() {
    let runner = test_runner(RefineLoopBudget::default());
    let mut ctx = make_ctx();
    // bands_satisfied is false by default (uniform-length draft).
    assert!(!ctx.bands_satisfied);

    let call = finish_call("readability is good enough", false);
    let result = runner.handle_finish(&call, &mut ctx).await;

    assert_eq!(result.status, ToolStatus::Ok);
    let data = result.data.unwrap();
    assert_eq!(data["finish_reason"], "readability is good enough");
    assert_eq!(data["bands_satisfied"], false);
    assert_eq!(data["validation_warning"], true);
}

#[tokio::test]
async fn handle_finish_returns_no_warning_when_bands_satisfied() {
    let runner = test_runner(RefineLoopBudget::default());
    let mut ctx = make_ctx();
    // Simulate bands being satisfied.
    ctx.bands_satisfied = true;

    let call = finish_call("all bands passed", true);
    let result = runner.handle_finish(&call, &mut ctx).await;

    assert_eq!(result.status, ToolStatus::Ok);
    let data = result.data.unwrap();
    assert_eq!(data["bands_satisfied"], true);
    assert_eq!(data["validation_warning"], false);
}

#[tokio::test]
async fn dispatch_tool_call_routes_finish_correctly() {
    let runner = test_runner(RefineLoopBudget::default());
    let mut ctx = make_ctx();
    let mut state = WriterState::default();

    let call = finish_call("done", false);
    let result = runner
        .dispatch_tool_call(&call, &mut ctx, &[], &NoopSink, &mut state)
        .await;

    assert_eq!(result.tool, "write_refine_finish");
    assert_eq!(result.status, ToolStatus::Ok);
}

#[tokio::test]
async fn dispatch_tool_call_unknown_tool_returns_error() {
    let runner = test_runner(RefineLoopBudget::default());
    let mut ctx = make_ctx();
    let mut state = WriterState::default();

    let call = ToolCall {
        tool: "nonexistent_tool".to_string(),
        version: "1".to_string(),
        args: serde_json::json!({}),
    };
    let result = runner
        .dispatch_tool_call(&call, &mut ctx, &[], &NoopSink, &mut state)
        .await;

    assert_eq!(result.status, ToolStatus::Error);
    assert!(result.data.unwrap()["error"]
        .as_str()
        .unwrap()
        .contains("unknown tool"));
}

#[test]
fn finish_reason_variants_are_distinct() {
    // Verify all four exit reasons are distinguishable (telemetry contract).
    assert_ne!(FinishReason::AgentFinish, FinishReason::IterationCap);
    assert_ne!(FinishReason::AgentFinish, FinishReason::TokenCap);
    assert_ne!(FinishReason::AgentFinish, FinishReason::ReviseRoundCap);
    assert_ne!(FinishReason::IterationCap, FinishReason::TokenCap);
    assert_ne!(FinishReason::IterationCap, FinishReason::ReviseRoundCap);
    assert_ne!(FinishReason::TokenCap, FinishReason::ReviseRoundCap);
}
