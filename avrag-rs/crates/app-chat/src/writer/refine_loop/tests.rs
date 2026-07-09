//! Handler-level WriteRefine tests via write-core runner + app-chat ports.

use super::{
    BestSnapshot, FinishReason, RefineContext, RefineLoopBudget, WriteRefineLoopRunner,
    WRITE_REFINE_HARD_REACT_CAP, WRITE_REFINE_GATE_MAX_REVISE,
};

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use avrag_llm::LlmClient;
use avrag_llm::ModelProviderConfig;
use contracts::ToolCall;
use contracts::chat::ToolStatus;
use heavytail::diagnosis::diagnose_pre_refine;
use heavytail::feedforward::fingerprint_workspace;
use heavytail::score::composite;
use heavytail::state::WriterState;
use heavytail::StyleParams;
use heavytail::workspace::{DraftWorkspace, ParagraphRecord, RhythmMode, SentenceRecord};
use heavytail::workspace::SentenceId;
use write_core::{
    WriteActivitySink, WriteParentMeta, WriteRefineModeHost, WriteResearchHit, WriteResearchKind,
    WriteResearchPort,
};

use agent_loop::events::{AgentEventSink, NoopSink};
use agent_loop::runtime::{Agent, AgentRequest, AgentRunResult};
use crate::agents::AgentKind;
use crate::writer::adapters::{
    parent_meta_from_request, AgentWriteActivitySink, AppWriteRefineMode, SubagentResearchPort,
};
use crate::writer::invoker::SubagentInvoker;
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

struct NeverCalledAgent;

#[async_trait]
impl Agent for NeverCalledAgent {
    async fn run(
        &self,
        _request: AgentRequest,
        _sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, common::AppError> {
        panic!("NeverCalledAgent::run should never be invoked for budget-exhausted path");
    }
}

/// Research port that panics if called (budget-exhausted path).
struct PanicResearch;

#[async_trait]
impl WriteResearchPort for PanicResearch {
    async fn research(
        &self,
        _kind: WriteResearchKind,
        _query: &str,
        _token_budget: usize,
    ) -> Result<WriteResearchHit, String> {
        panic!("research should not be invoked");
    }
}

/// Minimal mode host for unit tests (no real yaml required for handler tests).
struct TestModeHost;

impl WriteRefineModeHost for TestModeHost {
    fn temperature(&self) -> f32 {
        0.4
    }
    fn tool_specs(&self) -> Vec<contracts::ToolSpec> {
        vec![]
    }
    fn max_react_iterations(&self, _user_tier: Option<&str>, hard_cap: u8) -> u8 {
        hard_cap
    }
    fn system_prompt(
        &self,
        _iteration: u8,
        _max_iterations: u8,
        _persona: Option<&heavytail::persona::PersonaCard>,
        _revise_rounds_used: usize,
        _research_calls_used: usize,
        _budget: &RefineLoopBudget,
    ) -> String {
        "test".into()
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
        auth: agent_loop::runtime::stub_agent_auth(),
        docscope_metadata: None,
        metadata: BTreeMap::new(),
        cancellation_token: None,
        guard_pipeline: None,
        preferred_tools: vec![],
        format_hint: None,
        max_iterations: None,
    }
}

fn make_ctx() -> RefineContext {
    let ws = make_workspace();
    let style = StyleParams::default();
    let diag = diagnose_pre_refine(&ws, &style, &[]);
    RefineContext::new(ws, diag, MaterialPack::default(), None)
}

struct TestHarness {
    // keep leaked pieces so runner refs stay valid
    _hold: Vec<*const ()>,
}

fn test_runner(budget: RefineLoopBudget) -> WriteRefineLoopRunner<'static> {
    let llm = heavytail::llm::WriterLlm::from_client(dummy_llm_client());
    let llm: &'static heavytail::llm::WriterLlm = Box::leak(Box::new(llm));
    let research: &'static PanicResearch = Box::leak(Box::new(PanicResearch));
    let mode: &'static TestModeHost = Box::leak(Box::new(TestModeHost));
    WriteRefineLoopRunner::new(
        llm,
        research,
        mode,
        WriteParentMeta::default(),
        StyleParams::default(),
        budget,
    )
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
    let runner = test_runner(RefineLoopBudget::default());
    let mut ctx = make_ctx();
    let mut state = WriterState::default();

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
    assert_eq!(ctx.revise_rounds_used, 1);
    assert_eq!(state.rounds.len(), 1);
    assert_eq!(state.workspace.sentences[0].text, "这是一句被彻底改写过的全新句子。");
    let new_score = ctx.diagnosis.score_s;
    assert_eq!(
        ctx.best_snapshot.as_ref().unwrap().score,
        init_score.max(new_score)
    );
}

#[tokio::test]
async fn handle_research_6th_call_returns_budget_exhausted_without_invoking_worker() {
    let runner = test_runner(RefineLoopBudget::default());
    let mut ctx = make_ctx();
    ctx.research_calls_used = 5;

    let call = research_call("web", "test query for 6th call");
    let sink = AgentWriteActivitySink {
        inner: &NoopSink,
    };
    let result = runner.handle_research(&call, &mut ctx, &sink).await;

    assert_eq!(result.status, ToolStatus::Ok);
    let data = result.data.unwrap();
    assert_eq!(data["budget_exhausted"], true);
    assert_eq!(data["research_calls_used"], 5);
    assert_eq!(data["new_cards"].as_array().unwrap().len(), 0);
    assert_eq!(ctx.research_calls_used, 5);
}

#[tokio::test]
async fn handle_finish_returns_validation_warning_when_bands_not_satisfied() {
    let runner = test_runner(RefineLoopBudget::default());
    let mut ctx = make_ctx();
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
    ctx.bands_satisfied = true;

    let call = finish_call("all bands passed", true);
    let result = runner.handle_finish(&call, &mut ctx).await;

    assert_eq!(result.status, ToolStatus::Ok);
    let data = result.data.unwrap();
    assert_eq!(data["bands_satisfied"], true);
    assert_eq!(data["validation_warning"], false);
}

#[tokio::test]
async fn dispatch_tool_call_unknown_tool_returns_error() {
    let runner = test_runner(RefineLoopBudget::default());
    let mut ctx = make_ctx();
    let mut state = WriterState::default();
    let sink = AgentWriteActivitySink {
        inner: &NoopSink,
    };
    let call = ToolCall {
        tool: "not_a_real_tool".into(),
        version: "1".into(),
        args: serde_json::json!({}),
    };
    let result = runner
        .dispatch_tool_call(&call, &mut ctx, &[], &sink, &mut state)
        .await;
    assert_eq!(result.status, ToolStatus::Error);
}

#[tokio::test]
async fn dispatch_tool_call_routes_finish_correctly() {
    let runner = test_runner(RefineLoopBudget::default());
    let mut ctx = make_ctx();
    ctx.bands_satisfied = true;
    let mut state = WriterState::default();
    let sink = AgentWriteActivitySink {
        inner: &NoopSink,
    };
    let call = finish_call("done", true);
    let result = runner
        .dispatch_tool_call(&call, &mut ctx, &[], &sink, &mut state)
        .await;
    assert_eq!(result.status, ToolStatus::Ok);
}

#[test]
fn finish_reason_variants_are_distinct() {
    assert_ne!(FinishReason::AgentFinish, FinishReason::IterationCap);
}

// Silence unused import warnings for adapter types used when wiring real invoker paths.
#[allow(dead_code)]
fn _adapter_types_compile() {
    let _ = WRITE_REFINE_HARD_REACT_CAP;
    let _ = WRITE_REFINE_GATE_MAX_REVISE;
    let service = Arc::new(crate::agents::service::UnifiedAgentService::new(
        Box::new(NeverCalledAgent),
    ));
    let invoker = SubagentInvoker::new(service, None);
    let req = test_parent_request();
    let _ = parent_meta_from_request(&req);
    let _ = SubagentResearchPort {
        invoker: &invoker,
        parent: &req,
    };
    let _ = AppWriteRefineMode::load();
}
