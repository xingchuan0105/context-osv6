//! Channel workers: worker digests + post-run evidence finalization.
//!
//! `finalize_answer_evidence` is the single point where the chat exit's
//! `[[E:id]]` markers become product output: valid E-ids are rewritten to
//! product markers (`[[cite:chunk_id]]` / `[[web:n]]`) and mapped 1:1 to
//! `contracts::Citation` from the store; dangling or off-protocol markers
//! (`[[E99]]`, raw `[[web:1]]`…) are stripped with a warning — an empty
//! channel can never fabricate citations (2026-07-17 incident). Markers
//! pointing at targeted (DocProfile, orientation-only) entries are stripped
//! silently.
//!
//! Sub-agent observability (2026-07-24): channel workers keep their **raw**
//! tool trajectory + thinking process in [`WorkerRunObservability`]. The
//! chat-exit store bridge still collapses evidence to `dense_retrieval` for
//! eval recall@k — do **not** use `ChatResponse.tool_results` alone to audit
//! RAG/Search sub-agent behaviour; read `mode_debug.general.workers`.

use agent_loop::events::AgentEvent;
use agent_loop::runtime::{AgentRunResult, FinalDecision};
use contracts::chat::{AnswerBlock, Citation, SourceRef};
use serde::{Deserialize, Serialize};

use super::store::{EvidenceKind, EvidenceStore};
use super::types::{Channel, PremiseMismatch, WorkerHandoff};

const MAX_NOTE_CHARS: usize = 2000;
const MAX_GAPS: usize = 12;
const MAX_KEY_FACTS: usize = 16;
/// Cap stored worker reasoning text in mode_debug (full CoT can be large).
const MAX_REASONING_OBS_CHARS: usize = 8_000;
/// Cap per-tool data_summary string previews.
const MAX_DATA_PREVIEW_CHARS: usize = 400;
const MAX_THINKING_STEPS: usize = 48;
const MAX_ITERATIONS_OBS: usize = 32;

/// Compact tool row for sub-agent observability (not the eval store bridge).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerToolObs {
    pub tool: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degrade_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_hit_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hydrated_hit_count: Option<usize>,
    /// Compact shape (hit counts, code preview, graph source) — never full chunks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_summary: Option<serde_json::Value>,
}

/// One plan / eval / terminal thinking step from the worker event sink.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerThinkingStep {
    /// `plan` | `eval` | `terminal` | `tool_call` | `codegen`
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reasoning: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
}

/// Per-iteration white-box row (plan snapshot + exit decision).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerIterationObs {
    pub iteration: u8,
    pub decision: String,
    pub elapsed_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_evaluation: Option<serde_json::Value>,
}

/// White-box snapshot of one channel sub-agent run (tools **and** thinking).
///
/// Surfaced on `mode_debug.general.workers[]`. Independent of the store→eval
/// bridge that labels host evidence as `dense_retrieval`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerRunObservability {
    pub channel: Channel,
    /// Real worker tool names (`lexical_retrieval`, `graph_retrieval`, …).
    pub tools: Vec<WorkerToolObs>,
    /// Accumulated model reasoning / CoT summary from the ReAct loop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_summary: Option<String>,
    /// Plan/eval/terminal/codegen thinking steps (from sink + iterations).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub thinking: Vec<WorkerThinkingStep>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub iterations: Vec<WorkerIterationObs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_decision: Option<String>,
    pub total_tool_calls: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_elapsed_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff_summary: Option<String>,
}

/// Harvest plan/eval/codegen thinking from a worker's local event sink into
/// `AgentRunResult.debug_payload.worker_thinking` so dispatch can build
/// [`WorkerRunObservability`] without changing the executor trait.
pub fn attach_worker_thinking_events(run: &mut AgentRunResult, events: &[AgentEvent]) {
    let steps = thinking_steps_from_events(events);
    if steps.is_empty() {
        return;
    }
    let thinking_val = serde_json::to_value(&steps).unwrap_or(serde_json::json!([]));
    match run.debug_payload.as_mut() {
        Some(serde_json::Value::Object(map)) => {
            map.insert("worker_thinking".into(), thinking_val);
        }
        Some(other) => {
            *other = serde_json::json!({
                "prior": other.clone(),
                "worker_thinking": thinking_val,
            });
        }
        None => {
            run.debug_payload = Some(serde_json::json!({
                "worker_thinking": thinking_val,
            }));
        }
    }
}

/// Build a compact observability snapshot for one channel worker run.
pub fn worker_observability_from_run(
    channel: Channel,
    run: &AgentRunResult,
) -> WorkerRunObservability {
    let handoff_summary = worker_handoff_from_run(run).map(|h| h.summary);
    let mut thinking = thinking_steps_from_debug_payload(run.debug_payload.as_ref());
    // Iterations are themselves a thinking timeline when sink harvest is empty.
    if thinking.is_empty() {
        for it in run.iterations.iter().take(MAX_ITERATIONS_OBS) {
            thinking.push(WorkerThinkingStep {
                kind: "iteration".into(),
                decision: Some(it.decision.clone()),
                reasoning: it
                    .plan
                    .get("observation_preview")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                skills: it
                    .plan
                    .get("disclosed_skills")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
                tool: it
                    .plan
                    .get("action_type")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                detail: Some(it.plan.clone()),
            });
        }
    }
    WorkerRunObservability {
        channel,
        tools: run
            .tool_results
            .iter()
            .map(tool_obs_from_result)
            .collect(),
        reasoning_summary: run
            .reasoning_summary
            .as_ref()
            .map(|s| truncate_chars(s, MAX_REASONING_OBS_CHARS)),
        thinking,
        iterations: run
            .iterations
            .iter()
            .take(MAX_ITERATIONS_OBS)
            .map(|it| WorkerIterationObs {
                iteration: it.iteration,
                decision: it.decision.clone(),
                elapsed_ms: it.elapsed_ms,
                plan: Some(it.plan.clone()),
                llm_evaluation: it.llm_evaluation.clone(),
            })
            .collect(),
        final_decision: run.final_decision.as_ref().map(final_decision_label),
        total_tool_calls: run.total_tool_calls,
        total_elapsed_ms: run.total_elapsed_ms,
        handoff_summary,
    }
}

fn final_decision_label(d: &FinalDecision) -> String {
    match d {
        FinalDecision::Synthesized => "synthesized".into(),
        FinalDecision::DirectAnswer => "direct_answer".into(),
        FinalDecision::Clarified { .. } => "clarified".into(),
        FinalDecision::Degraded { reason } => format!("degraded:{reason:?}"),
    }
}

fn tool_obs_from_result(tr: &contracts::ToolResult) -> WorkerToolObs {
    let status = format!("{:?}", tr.status).to_lowercase();
    let (degrade_reason, elapsed_ms, raw_hit_count, hydrated_hit_count) = match &tr.trace {
        Some(t) => (
            t.degrade_reason.clone(),
            t.elapsed_ms,
            t.raw_hit_count,
            t.hydrated_hit_count,
        ),
        None => (None, None, None, None),
    };
    WorkerToolObs {
        tool: tr.tool.clone(),
        status,
        degrade_reason,
        elapsed_ms,
        raw_hit_count,
        hydrated_hit_count,
        data_summary: tr.data.as_ref().map(summarize_tool_data),
    }
}

fn summarize_tool_data(data: &serde_json::Value) -> serde_json::Value {
    if let Some(arr) = data.as_array() {
        return serde_json::json!({
            "kind": "array",
            "len": arr.len(),
        });
    }
    let obj = match data.as_object() {
        Some(o) => o,
        None => {
            return serde_json::json!({
                "kind": "scalar",
                "preview": truncate_chars(&data.to_string(), MAX_DATA_PREVIEW_CHARS),
            });
        }
    };
    let mut out = serde_json::Map::new();
    if let Some(src) = obj.get("source").and_then(|v| v.as_str()) {
        out.insert("source".into(), serde_json::json!(src));
    }
    for key in ["chunks", "results", "items", "graph_context", "hits"] {
        if let Some(arr) = obj.get(key).and_then(|v| v.as_array()) {
            out.insert(format!("{key}_len"), serde_json::json!(arr.len()));
        }
    }
    if let Some(code) = obj.get("code").and_then(|v| v.as_str()) {
        out.insert(
            "code_preview".into(),
            serde_json::json!(truncate_chars(code, MAX_DATA_PREVIEW_CHARS)),
        );
    }
    if let Some(result) = obj.get("result").and_then(|v| v.as_str()) {
        out.insert(
            "result_preview".into(),
            serde_json::json!(truncate_chars(result, MAX_DATA_PREVIEW_CHARS)),
        );
    }
    if let Some(lang) = obj.get("language").and_then(|v| v.as_str()) {
        out.insert("language".into(), serde_json::json!(lang));
    }
    if let Some(err) = obj.get("error").and_then(|v| v.as_str()) {
        out.insert(
            "error".into(),
            serde_json::json!(truncate_chars(err, MAX_DATA_PREVIEW_CHARS)),
        );
    }
    if out.is_empty() {
        out.insert(
            "keys".into(),
            serde_json::json!(obj.keys().cloned().collect::<Vec<_>>()),
        );
    }
    serde_json::Value::Object(out)
}

fn thinking_steps_from_debug_payload(
    payload: Option<&serde_json::Value>,
) -> Vec<WorkerThinkingStep> {
    payload
        .and_then(|p| p.get("worker_thinking"))
        .and_then(|v| serde_json::from_value::<Vec<WorkerThinkingStep>>(v.clone()).ok())
        .unwrap_or_default()
}

fn thinking_steps_from_events(events: &[AgentEvent]) -> Vec<WorkerThinkingStep> {
    let mut steps = Vec::new();
    for ev in events {
        if steps.len() >= MAX_THINKING_STEPS {
            break;
        }
        match ev {
            AgentEvent::PlanDecision {
                selected_skills,
                reasoning,
                selected_tools,
                behavior_mode,
                ..
            } => {
                steps.push(WorkerThinkingStep {
                    kind: "plan".into(),
                    decision: behavior_mode.clone(),
                    reasoning: truncate_chars(reasoning, MAX_REASONING_OBS_CHARS),
                    skills: selected_skills.clone(),
                    tool: None,
                    detail: Some(serde_json::json!({
                        "selected_tools": selected_tools.iter().map(|t| &t.tool).collect::<Vec<_>>(),
                    })),
                });
            }
            AgentEvent::Evaluation {
                decision,
                reasoning,
                signals,
            } => {
                steps.push(WorkerThinkingStep {
                    kind: "eval".into(),
                    decision: Some(decision.clone()),
                    reasoning: truncate_chars(reasoning, MAX_REASONING_OBS_CHARS),
                    skills: signals
                        .as_ref()
                        .and_then(|s| s.get("disclosed_skills"))
                        .and_then(|v| v.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|x| x.as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default(),
                    tool: signals
                        .as_ref()
                        .and_then(|s| s.get("action_type"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    detail: signals.clone(),
                });
            }
            AgentEvent::Terminal { decision, reason } => {
                steps.push(WorkerThinkingStep {
                    kind: "terminal".into(),
                    decision: Some(decision.clone()),
                    reasoning: reason.clone().unwrap_or_default(),
                    skills: Vec::new(),
                    tool: None,
                    detail: None,
                });
            }
            AgentEvent::ToolCall { tool, args } => {
                // code_gen / native tools as thinking timeline (args truncated).
                let is_codegen = tool == "code_gen" || tool == "code_execution";
                steps.push(WorkerThinkingStep {
                    kind: if is_codegen {
                        "codegen".into()
                    } else {
                        "tool_call".into()
                    },
                    decision: None,
                    reasoning: String::new(),
                    skills: Vec::new(),
                    tool: Some(tool.clone()),
                    detail: args.as_ref().map(|a| summarize_tool_data(a)),
                });
            }
            AgentEvent::ToolResult {
                tool,
                status,
                data,
                elapsed_ms,
            } if tool == "code_gen" || tool == "code_execution" => {
                steps.push(WorkerThinkingStep {
                    kind: "codegen".into(),
                    decision: Some(format!("{status:?}").to_lowercase()),
                    reasoning: data
                        .as_ref()
                        .and_then(|d| d.get("result").and_then(|r| r.as_str()))
                        .map(|s| truncate_chars(s, MAX_DATA_PREVIEW_CHARS))
                        .unwrap_or_default(),
                    skills: Vec::new(),
                    tool: Some(tool.clone()),
                    detail: Some(serde_json::json!({ "elapsed_ms": elapsed_ms })),
                });
            }
            _ => {}
        }
    }
    steps
}

fn truncate_chars(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

/// Non-Ok tool outcomes from a worker run, as short descriptions
/// (`web_search: Timeout (detail)`). Used to distinguish "检索失败" (Error)
/// from "未命中" (Empty) instead of silently collapsing both to Empty.
pub fn tool_failures(results: &[contracts::ToolResult]) -> Vec<String> {
    results
        .iter()
        .filter(|tr| tr.status != contracts::ToolStatus::Ok)
        .map(|tr| {
            let detail = tr
                .data
                .as_ref()
                .and_then(|d| d.get("error").and_then(|e| e.as_str()))
                .map(|s| format!(" ({s})"))
                .unwrap_or_default();
            format!("{}: {:?}{detail}", tr.tool, tr.status)
        })
        .collect()
}

/// Parse structured worker handoff from the worker's final message.
///
/// K3 (design 2026-07-28 §4.3): the handoff contract is "分析散文 + 可选
/// SELECTED 行" — prose is a legal delivery. This is the post-loop compile
/// of the SAME channel the loop uses at the `direct_content` decision point
/// (agent-loop `output_compiler`): a cheap safety net with no continuation.
/// Degraded now means ONLY: E104 fabrication stripped, or E105 rejected
/// (insufficient declared with zero retrieval calls) — never "not JSON".
pub fn worker_handoff_from_run(result: &AgentRunResult) -> Option<WorkerHandoff> {
    if result.answer.trim().is_empty() {
        return None;
    }
    let outcome =
        agent_loop::output_compiler::compile_handoff(&agent_loop::output_compiler::HandoffCompileInput {
            raw: &result.answer,
            has_tool_results: !result.tool_results.is_empty(),
        });
    handoff_from_compile(&result.answer, outcome)
}

/// Worker channel summary (flat string) — prefers structured handoff summary.
pub fn channel_note_from_run(result: &AgentRunResult) -> Option<String> {
    worker_handoff_from_run(result).map(|h| h.summary)
}

/// Parse a worker final message into [`WorkerHandoff`] without run context
/// (pure parse; E105's zero-retrieval gate needs the run's tool trail and is
/// skipped here). Prose becomes the summary (K3); JSON yields structured
/// fields when it carries them.
pub fn parse_worker_handoff(raw: &str) -> Option<WorkerHandoff> {
    if raw.trim().is_empty() {
        return None;
    }
    let outcome =
        agent_loop::output_compiler::compile_handoff(&agent_loop::output_compiler::HandoffCompileInput {
            raw,
            has_tool_results: false,
        });
    handoff_from_compile(raw, outcome)
}

/// Map a compile outcome to a [`WorkerHandoff`] (K3):
/// - JSON value that reads as a handoff → structured fields (summary /
///   coverage / gaps / premise_mismatch; key_facts is deprecated — facts are
///   owned by SELECTED + hydration);
/// - anything else (prose, SELECTED-only, non-envelope JSON) → the raw
///   message becomes the prose summary, coverage defaults to "partial"
///   (same default the old `freeform_summary` used — least surprising);
/// - degraded ONLY when E104 stripped a fabrication or E105 rejected —
///   codes ride along in `compile_diagnostics`.
fn handoff_from_compile(
    raw: &str,
    outcome: agent_loop::output_compiler::CompileOutcome<serde_json::Value>,
) -> Option<WorkerHandoff> {
    let codes = outcome.diagnostic_codes();
    let mut h = match outcome.value.as_ref().and_then(handoff_from_value) {
        Some(h) => h,
        None => {
            // K3: prose path — the message itself is the summary. E104's
            // fabrication stripping applies to prose too.
            let summary = agent_loop::output_compiler::strip_code_execution_blocks(raw.trim());
            let mut h = WorkerHandoff::freeform_summary(summary.clone());
            if summary != raw.trim() {
                h.handoff_degraded = true;
                if !codes.iter().any(|c| c == "E104") {
                    h.compile_diagnostics.push("E104".to_string());
                }
            }
            h
        }
    };
    // Degraded = any Error diagnostic (E105) or a transformation (E104
    // strip); warnings alone never degrade; prose handoffs are NOT degraded.
    let transformed = outcome
        .diagnostics
        .iter()
        .any(|d| d.code == "E104");
    h.handoff_degraded = h.handoff_degraded || outcome.has_errors() || transformed;
    for code in codes {
        if !h.compile_diagnostics.contains(&code) {
            h.compile_diagnostics.push(code);
        }
    }
    Some(cap_handoff(h))
}

// K3 note: the envelope/pointer validation that used to live here (E101
// envelope check, E102 key_facts presence, E103 pointer truthfulness) is
// retired with the K3 rule-table slimming — prose handoffs are legal and
// evidence pointers are code-hydrated (K2 `selected.rs`). What remains is
// the thin JSON→WorkerHandoff glue; E104/E105 still fire.

fn handoff_from_value(v: &serde_json::Value) -> Option<WorkerHandoff> {
    let schema = v
        .get("schema_version")
        .and_then(|s| s.as_str())
        .unwrap_or("");

    // Preferred: internal_worker_handoff_v1 (summary required).
    if schema == "internal_worker_handoff_v1" || v.get("summary").and_then(|s| s.as_str()).is_some()
    {
        let summary = v
            .get("summary")
            .and_then(|s| s.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())?;
        let coverage = v
            .get("coverage")
            .and_then(|s| s.as_str())
            .unwrap_or("partial")
            .trim()
            .to_string();
        let gaps = string_list(v.get("gaps"));
        let premise_mismatch = premise_mismatch_from(v.get("premise_mismatch"));
        return Some(WorkerHandoff {
            summary: summary.to_string(),
            // K3: key_facts parsing is dropped — facts are owned by the
            // SELECTED log + code hydration. The field stays for serde
            // compat with old artifacts but is no longer populated here.
            key_facts: Vec::new(),
            coverage: if coverage.is_empty() {
                "partial".into()
            } else {
                coverage
            },
            gaps,
            handoff_degraded: false,
            compile_diagnostics: Vec::new(),
            premise_mismatch,
        });
    }

    // Legacy unified answer envelope: map answer_text → summary.
    if schema == "internal_answer_v1" || v.get("answer_text").is_some() {
        let summary = v
            .get("answer_text")
            .and_then(|s| s.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())?;
        let coverage = v
            .get("coverage")
            .and_then(|s| s.as_str())
            .unwrap_or("partial")
            .to_string();
        let mut gaps = string_list(v.get("gaps"));
        if let Some(reason) = v
            .get("refusal_reason")
            .and_then(|s| s.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            gaps.push(reason.to_string());
        }
        return Some(WorkerHandoff {
            summary: summary.to_string(),
            key_facts: Vec::new(),
            coverage,
            gaps,
            handoff_degraded: false,
            compile_diagnostics: Vec::new(),
            premise_mismatch: None,
        });
    }

    None
}

/// S3: parse the optional `premise_mismatch` block. Tolerant: an incomplete
/// block (missing kind/detail) is ignored rather than failing the handoff.
fn premise_mismatch_from(v: Option<&serde_json::Value>) -> Option<PremiseMismatch> {
    let pm = v?;
    let kind = pm
        .get("kind")
        .and_then(|s| s.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())?
        .to_string();
    let detail = pm
        .get("detail")
        .and_then(|s| s.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())?
        .to_string();
    let actual_subject = pm
        .get("actual_subject")
        .and_then(|s| s.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    Some(PremiseMismatch {
        kind,
        detail,
        actual_subject,
    })
}

fn string_list(v: Option<&serde_json::Value>) -> Vec<String> {
    v.and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| e.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn cap_handoff(mut h: WorkerHandoff) -> WorkerHandoff {
    if h.summary.chars().count() > MAX_NOTE_CHARS {
        h.summary = h.summary.chars().take(MAX_NOTE_CHARS).collect();
    }
    if h.gaps.len() > MAX_GAPS {
        h.gaps.truncate(MAX_GAPS);
    }
    if h.key_facts.len() > MAX_KEY_FACTS {
        h.key_facts.truncate(MAX_KEY_FACTS);
    }
    h
}

/// Attach store evidence as retrieval `tool_results` (eval / clients).
/// Does **not** rewrite E-markers or build citations — safe for `mode=direct`.
///
/// V2 may finish with `finish_answer(mode=direct)` even after workers filled the
/// store; without this, full_eval sees `tool_results_count=0` despite
/// `dispatches[].item_count > 0` (G-16 / 2026-07-20 fail-fast).
pub fn attach_store_retrieval_tool_results(
    answer_result: &mut AgentRunResult,
    store: &EvidenceStore,
) {
    let mut retrieval = store.as_retrieval_tool_results();
    if retrieval.is_empty() {
        return;
    }
    retrieval.append(&mut answer_result.tool_results);
    answer_result.tool_results = retrieval;
}

/// Normalize common LLM slips around E-ids into canonical `[[En]]` form.
///
/// Real models often emit markdown-bold or single-bracket variants
/// (`**[E3]**`, `[**[E3]**]`, bare `[E3]`) instead of protocol `[[E3]]`.
/// Without this pass, finalize drops them and `expect_citations` fails even
/// when the model *did* ground on store ids (full_eval Q142, 2026-07-20).
/// S7: full-width `【E3】` / `【E:3】` / `【E：3】` get the same treatment —
/// they used to leak raw into the user-facing answer (q087 附, 2026-07-27).
fn normalize_loose_e_markers(answer: &str) -> String {
    use std::sync::OnceLock;
    use regex::Regex;

    struct Patterns {
        fullwidth: Regex,
        bracket_bold: Regex,
        bold_double: Regex,
        bold_single: Regex,
        double_bold_inner: Regex,
        protected: Regex,
        bare_single: Regex,
    }
    static PATS: OnceLock<Patterns> = OnceLock::new();
    let p = PATS.get_or_init(|| Patterns {
        // S7: full-width 【E3】 / 【E:3】 / 【E：3】 (half- or full-width colon)
        fullwidth: Regex::new(r"【E[:：]?(\d+)】").expect("e-marker re"),
        // [**[E3]**] / [**[E:3]**]
        bracket_bold: Regex::new(r"\[\*\*\[E:?(\d+)\]\*\*\]").expect("e-marker re"),
        // **[[E3]]** / **[[E:3]]**
        bold_double: Regex::new(r"\*\*\[\[E:?(\d+)\]\]\*\*").expect("e-marker re"),
        // **[E3]** / **[E:3]**
        bold_single: Regex::new(r"\*\*\[E:?(\d+)\]\*\*").expect("e-marker re"),
        // [[**E3**]] / [[**E:3**]]
        double_bold_inner: Regex::new(r"\[\[\*\*E:?(\d+)\*\*\]\]").expect("e-marker re"),
        // already-canonical [[E3]] / [[E:3]]
        protected: Regex::new(r"\[\[E:?(\d+)\]\]").expect("e-marker re"),
        // bare [E3] / [E:3]
        bare_single: Regex::new(r"\[E:?(\d+)\]").expect("e-marker re"),
    });

    // Full-width first: they contain no ASCII brackets, so converting them up
    // front lets the canonical passes below treat them uniformly.
    let mut s = p.fullwidth.replace_all(answer, "[[E$1]]").into_owned();
    s = p
        .bracket_bold
        .replace_all(&s, "[[E$1]]")
        .into_owned();
    s = p.bold_double.replace_all(&s, "[[E$1]]").into_owned();
    s = p.bold_single.replace_all(&s, "[[E$1]]").into_owned();
    s = p.double_bold_inner.replace_all(&s, "[[E$1]]").into_owned();

    // Protect canonical doubles, convert remaining singles, restore.
    let mut placeholders: Vec<String> = Vec::new();
    let tmp = p
        .protected
        .replace_all(&s, |caps: &regex::Captures| {
            let i = placeholders.len();
            placeholders.push(format!("[[E{}]]", &caps[1]));
            format!("\u{E000}{i}\u{E001}")
        })
        .into_owned();
    let mut out = p.bare_single.replace_all(&tmp, "[[E$1]]").into_owned();
    for (i, ph) in placeholders.iter().enumerate() {
        out = out.replace(&format!("\u{E000}{i}\u{E001}"), ph);
    }
    out
}

/// Rewrite E-markers to product markers, rebuild citations/sources, and attach
/// store evidence as retrieval `tool_results` so eval / clients can score the
/// host-decided retrieval layer (not the chat-exit tool trace alone).
pub fn finalize_answer_evidence(answer_result: &mut AgentRunResult, store: &EvidenceStore) {
    let raw_answer = answer_result.answer.clone();
    let normalized = normalize_loose_e_markers(&raw_answer);
    let (rewritten, citations, stripped) = rewrite_markers(&normalized, store);
    if stripped > 0 {
        tracing::warn!(
            stripped,
            "orchestrator chat exit emitted dangling/off-protocol citation markers"
        );
    }
    answer_result.answer = rewritten.clone();
    for block in &mut answer_result.answer_blocks {
        if let AnswerBlock::Text { text, .. } = block {
            if *text == raw_answer || *text == normalized {
                *text = rewritten.clone();
            } else if text.contains('[') {
                // Divergent block text: normalize + rewrite standalone.
                let norm = normalize_loose_e_markers(text);
                let (t, _, _) = rewrite_markers(&norm, store);
                *text = t;
            }
        }
    }
    answer_result.citations = citations;
    answer_result.sources = store
        .entries()
        .iter()
        .filter(|e| e.kind == EvidenceKind::WebPage)
        .map(|e| SourceRef {
            id: e.url.clone().unwrap_or_else(|| e.eid.clone()),
            title: e.title.clone().unwrap_or_default(),
            snippet: Some(e.preview.clone()),
            doc_id: None,
            page: None,
        })
        .collect();
    // Prepend store-backed retrieval results so ChatResponse.tool_results
    // reflects the orchestrator-decided evidence set (eval extract_retrieved_chunks).
    attach_store_retrieval_tool_results(answer_result, store);
}

/// Scan `[[...]]` tokens; returns (rewritten text, citations, stripped count).
fn rewrite_markers(
    answer: &str,
    store: &EvidenceStore,
) -> (String, Vec<Citation>, usize) {
    let mut out = String::with_capacity(answer.len());
    let mut citations: Vec<Citation> = Vec::new();
    let mut seen_eids = std::collections::HashSet::new();
    let mut stripped = 0usize;
    // Single global appearance-order counter: citation_id must be unique
    // across doc AND web citations — the lookup endpoint finds by citation_id
    // alone, and the frontend resolves `[[web:n]]` by array position (which
    // equals this counter). Two per-kind counters collided (2026-07-18).
    let mut next_index: i64 = 1;

    let mut rest = answer;
    while let Some(start) = rest.find("[[") {
        let (head, tail) = rest.split_at(start);
        out.push_str(head);
        let Some(end) = tail.find("]]") else {
            // Unclosed [[… at end: keep as plain text (do not invent a close).
            out.push_str(tail);
            rest = "";
            break;
        };
        // Malformed open: another `[[` appears before the first `]]`
        // (e.g. `[[E15]目录]…[[E3]]`). Treating the span greedily would
        // swallow the later valid marker into one token. Emit just `[[`
        // and rescan so the inner marker can be rewritten.
        if let Some(next_open) = tail[2..].find("[[") {
            if next_open + 2 < end {
                out.push_str("[[");
                rest = &tail[2..];
                continue;
            }
        }
        let token = tail[2..end].trim();

        if let Some(eid) = parse_e_marker(token) {
            match store.get(&eid) {
                Some(entry) if entry.kind == EvidenceKind::DocProfile => {
                    // Targeted (orientation) entry: not citable — strip silently
                    // (the model was told not to cite it; no warning needed).
                }
                Some(entry) => {
                    if seen_eids.insert(eid.clone()) {
                        match entry.kind {
                            EvidenceKind::DocChunk => {
                                let chunk = entry.chunk_id.clone().unwrap_or_default();
                                out.push_str(&format!("[[cite:{chunk}]]"));
                                citations.push(Citation {
                                    citation_id: next_index,
                                    doc_id: entry.doc_id.clone().unwrap_or_default(),
                                    chunk_id: Some(chunk),
                                    page: entry.page,
                                    doc_name: entry
                                        .doc_name
                                        .clone()
                                        .or_else(|| entry.doc_id.clone())
                                        .unwrap_or_default(),
                                    preview: Some(entry.preview.clone()),
                                    content: Some(entry.full_text.clone()),
                                    score: entry.score.unwrap_or(0.0) as f32,
                                    layer: Some("dense_retrieval".to_string()),
                                    chunk_type: Some("text".to_string()),
                                    asset_id: None,
                                    caption: None,
                                    image_url: None,
                                    parser_backend: None,
                                    source_locator: None,
                                    parse_run_id: None,
                                });
                                next_index += 1;
                            }
                            EvidenceKind::WebPage => {
                                let url = entry.url.clone().unwrap_or_default();
                                out.push_str(&format!("[[web:{next_index}]]"));
                                citations.push(Citation {
                                    citation_id: next_index,
                                    doc_id: url.clone(),
                                    chunk_id: None,
                                    page: None,
                                    doc_name: entry.title.clone().unwrap_or_default(),
                                    preview: Some(entry.preview.clone()),
                                    content: Some(entry.full_text.clone()),
                                    score: 1.0,
                                    layer: Some("search".to_string()),
                                    chunk_type: Some("web".to_string()),
                                    asset_id: None,
                                    caption: None,
                                    image_url: None,
                                    parser_backend: None,
                                    source_locator: Some(serde_json::json!({
                                        "url": url,
                                        "title": entry.title.clone().unwrap_or_default(),
                                    })),
                                    parse_run_id: None,
                                });
                                next_index += 1;
                            }
                            EvidenceKind::DocProfile => {
                                // Unreachable: handled by the match guard above.
                            }
                        }
                    } else {
                        // Repeat citation of the same entry: reuse its product marker.
                        let existing = citations
                            .iter()
                            .find(|c| marker_source_matches(c, &eid, store));
                        match existing {
                            Some(c) if c.chunk_id.is_some() => {
                                out.push_str(&format!(
                                    "[[cite:{}]]",
                                    c.chunk_id.as_deref().unwrap_or_default()
                                ));
                            }
                            Some(c) => {
                                out.push_str(&format!("[[web:{}]]", c.citation_id));
                            }
                            None => {}
                        }
                    }
                }
                None => stripped += 1, // dangling E-id: drop the marker
            }
        } else if is_raw_citation_marker(token) {
            // Off-protocol [[cite:…]] / [[web:n]] / [[n]]: ungrounded here → drop.
            stripped += 1;
        } else {
            // Not a citation token (e.g. [[image:…]]) — pass through untouched.
            out.push_str(&rest_after_token_prefix(tail));
        }
        rest = &tail[end + 2..];
    }
    out.push_str(rest);
    (out, citations, stripped)
}

/// `E7` / `E:7` → "E7" (store id form).
fn parse_e_marker(token: &str) -> Option<String> {
    let t = token.strip_prefix('E').or_else(|| token.strip_prefix('e'))?;
    let t = t.strip_prefix(':').unwrap_or(t);
    if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) {
        Some(format!("E{t}"))
    } else {
        None
    }
}

fn is_raw_citation_marker(token: &str) -> bool {
    if token.starts_with("cite:") {
        return true;
    }
    let t = token.strip_prefix("web:").unwrap_or(token);
    !t.is_empty()
        && t.split(',')
            .all(|p| p.trim().chars().all(|c| c.is_ascii_digit()) && !p.trim().is_empty())
}

/// Whether an existing citation came from this store entry (for repeat refs).
fn marker_source_matches(
    citation: &Citation,
    eid: &str,
    store: &EvidenceStore,
) -> bool {
    let Some(entry) = store.get(eid) else {
        return false;
    };
    match entry.kind {
        EvidenceKind::DocChunk => citation.chunk_id == entry.chunk_id,
        EvidenceKind::WebPage => citation.doc_id == entry.url.clone().unwrap_or_default(),
        EvidenceKind::DocProfile => false,
    }
}

/// Re-emit the untouched `[[token]]` text (helper for pass-through).
fn rest_after_token_prefix(tail: &str) -> String {
    let end = tail.find("]]").map(|i| i + 2).unwrap_or(tail.len());
    tail[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::store::EvidenceStore;
    use crate::orchestrator::types::Channel;
    use contracts::{ToolResult, ToolStatus};

    fn store_with_both() -> EvidenceStore {
        let mut store = EvidenceStore::default();
        store.insert_from_tool_results(
            Channel::Rag,
            &[ToolResult {
                tool: "dense_retrieval".into(),
                version: "1".into(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!([
                    {"chunk_id": "chunk-a", "doc_id": "d1", "text": "doc evidence", "score": 0.9, "page": 3}
                ])),
                trace: None,
            }],
        );
        store.insert_from_tool_results(
            Channel::Search,
            &[ToolResult {
                tool: "web_search".into(),
                version: "1".into(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!({
                    "results": [{"url": "https://a.example", "title": "A", "snippet": "web evidence"}]
                })),
                trace: None,
            }],
        );
        store
    }

    #[test]
    fn valid_markers_become_product_markers_and_citations() {
        let store = store_with_both();
        let mut r = AgentRunResult::default();
        r.answer = "文档证据 [[E1]]，网页佐证 [[E2]]。重复 [[E1]]".into();
        finalize_answer_evidence(&mut r, &store);

        assert!(r.answer.contains("[[cite:chunk-a]]"), "{}", r.answer);
        // Single global counter: doc first (1), web second (2).
        assert!(r.answer.contains("[[web:2]]"), "{}", r.answer);
        assert!(!r.answer.contains("[[E"), "E-ids must be gone: {}", r.answer);
        assert_eq!(r.citations.len(), 2, "repeat ref dedupes");
        assert_eq!(r.citations[0].chunk_id.as_deref(), Some("chunk-a"));
        assert_eq!(r.citations[0].page, Some(3));
        assert_eq!(r.citations[1].layer.as_deref(), Some("search"));
        assert_eq!(r.citations[1].doc_id, "https://a.example");
        assert_eq!(r.sources.len(), 1);
    }

    #[test]
    fn loose_markdown_e_markers_are_normalized_and_cited() {
        // full_eval Q142 style: model wraps E-ids in markdown bold / single brackets.
        let store = store_with_both();
        let mut r = AgentRunResult::default();
        r.answer =
            "直接根源[**[E1]**]。模式层**[E1]**。底层[E1]。规范 [[E1]] 不双写。".into();
        finalize_answer_evidence(&mut r, &store);

        assert!(
            r.answer.contains("[[cite:chunk-a]]"),
            "loose E markers must rewrite: {}",
            r.answer
        );
        assert!(
            !r.answer.contains("[E1]") && !r.answer.contains("**[E"),
            "loose forms must be gone: {}",
            r.answer
        );
        assert_eq!(r.citations.len(), 1, "dedupe across loose forms");
    }

    // ---- S7: full-width 【E:n】 markers -------------------------------------

    #[test]
    fn fullwidth_valid_markers_are_rewritten() {
        let store = store_with_both();
        let mut r = AgentRunResult::default();
        r.answer = "文档证据【E1】，网页佐证【E:2】，全角冒号【E：1】。".into();
        finalize_answer_evidence(&mut r, &store);

        assert!(r.answer.contains("[[cite:chunk-a]]"), "{}", r.answer);
        assert!(r.answer.contains("[[web:2]]"), "{}", r.answer);
        assert!(
            !r.answer.contains('【'),
            "full-width markers must be gone: {}",
            r.answer
        );
        assert_eq!(r.citations.len(), 2, "dedupe across full-width forms");
    }

    #[test]
    fn fullwidth_dangling_markers_are_stripped() {
        let store = store_with_both();
        let mut r = AgentRunResult::default();
        r.answer = "编造的【E9】和不存在的【E:42】都剥离。".into();
        finalize_answer_evidence(&mut r, &store);

        assert!(!r.answer.contains('【'), "{}", r.answer);
        assert!(!r.answer.contains("[["), "{}", r.answer);
        assert!(r.citations.is_empty());
    }

    #[test]
    fn mixed_half_and_full_width_in_one_answer() {
        let store = store_with_both();
        let mut r = AgentRunResult::default();
        r.answer = "半角 [[E1]] 与全角【E2】并存，悬空【E8】剥离。".into();
        finalize_answer_evidence(&mut r, &store);

        assert!(r.answer.contains("[[cite:chunk-a]]"), "{}", r.answer);
        assert!(r.answer.contains("[[web:2]]"), "{}", r.answer);
        assert!(!r.answer.contains('【'), "{}", r.answer);
        assert_eq!(r.citations.len(), 2);
    }

    #[test]
    fn citation_ids_are_unique_across_doc_and_web() {
        // 2026-07-18 incident: per-kind counters collided — lookup by
        // citation_id returned the wrong entry (web before doc → 404).
        let store = store_with_both();
        let mut r = AgentRunResult::default();
        r.answer = "网页 [[E2]] 在前，文档 [[E1]] 在后。".into();
        finalize_answer_evidence(&mut r, &store);

        let ids: Vec<i64> = r.citations.iter().map(|c| c.citation_id).collect();
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(ids.len(), unique.len(), "citation ids must be unique: {ids:?}");
        // `[[web:n]]` n == citation_id == array position (frontend resolves by index).
        assert_eq!(r.citations[0].citation_id, 1);
        assert_eq!(r.citations[0].layer.as_deref(), Some("search"));
        assert!(r.answer.contains("[[web:1]]"), "{}", r.answer);
        assert_eq!(r.citations[1].citation_id, 2);
        assert_eq!(r.citations[1].chunk_id.as_deref(), Some("chunk-a"));
    }

    #[test]
    fn dangling_and_raw_markers_are_stripped() {
        let store = store_with_both();
        let mut r = AgentRunResult::default();
        r.answer = "编造的 [[E9]] 和原生 [[web:7]] 与 [[cite:fake]] 都剥离。".into();
        finalize_answer_evidence(&mut r, &store);

        assert!(!r.answer.contains("[["), "{}", r.answer);
        assert!(r.citations.is_empty());
    }

    #[test]
    fn finalize_prepends_store_as_retrieval_tool_results() {
        let store = store_with_both();
        let mut r = AgentRunResult::default();
        r.answer = "ok".into();
        r.tool_results = vec![ToolResult {
            tool: "user_context".into(),
            version: "1".into(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({})),
            trace: None,
        }];
        finalize_answer_evidence(&mut r, &store);
        assert!(
            r.tool_results
                .iter()
                .any(|t| t.tool == "dense_retrieval"),
            "store doc chunks must surface as dense_retrieval: {:?}",
            r.tool_results.iter().map(|t| &t.tool).collect::<Vec<_>>()
        );
        let dense = r
            .tool_results
            .iter()
            .find(|t| t.tool == "dense_retrieval")
            .unwrap();
        let arr = dense.data.as_ref().and_then(|d| d.as_array()).unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["chunk_id"], "chunk-a");
        assert_eq!(arr[0]["text"], "doc evidence");
        // Original chat tools preserved after retrieval results.
        assert_eq!(r.tool_results.last().unwrap().tool, "user_context");
    }

    /// Direct exit path: attach retrieval for eval without inventing citations.
    #[test]
    fn attach_store_retrieval_without_cite_rewrite() {
        let store = store_with_both();
        let mut r = AgentRunResult::default();
        r.answer = "直接回答，无引用。".into();
        attach_store_retrieval_tool_results(&mut r, &store);
        assert!(
            r.tool_results.iter().any(|t| t.tool == "dense_retrieval"),
            "{:?}",
            r.tool_results.iter().map(|t| &t.tool).collect::<Vec<_>>()
        );
        assert!(r.citations.is_empty());
        assert_eq!(r.answer, "直接回答，无引用。");
    }

    #[test]
    fn targeted_entry_markers_stripped_silently() {
        let mut store = store_with_both();
        store.insert_from_tool_results(
            Channel::Rag,
            &[ToolResult {
                tool: "doc_profile".into(),
                version: "1".into(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!([
                    {"doc_id": "d1", "genre": "report", "sections": [{"title": "t", "page": 1}]}
                ])),
                trace: None,
            }],
        );
        // E3 = targeted entry; citing it must vanish without a citation.
        let mut r = AgentRunResult::default();
        r.answer = "结构 [[E3]] 证据 [[E1]]".into();
        finalize_answer_evidence(&mut r, &store);

        assert!(!r.answer.contains("[[E3]]"), "{}", r.answer);
        assert!(r.answer.contains("[[cite:chunk-a]]"), "{}", r.answer);
        assert_eq!(r.citations.len(), 1);
        assert!(r.citations.iter().all(|c| c.chunk_id.is_some()));
    }

    #[test]
    fn answer_blocks_text_rewritten() {
        let store = store_with_both();
        let mut r = AgentRunResult::default();
        r.answer = "证据 [[E1]]".into();
        r.answer_blocks = vec![AnswerBlock::Text {
            text: "证据 [[E1]]".into(),
            citations: vec![],
        }];
        finalize_answer_evidence(&mut r, &store);
        let AnswerBlock::Text { text, .. } = &r.answer_blocks[0] else {
            panic!("text block");
        };
        assert!(text.contains("[[cite:chunk-a]]"));
    }

    #[test]
    fn parses_structured_worker_handoff_json() {
        let raw = r#"{
          "schema_version": "internal_worker_handoff_v1",
          "summary": "立项报告，结构：现状→目标→路径",
          "key_facts": [
            {"claim": "采用微服务", "evidence": ["chunk-a"]},
            "预算约 2 亿"
          ],
          "coverage": "partial",
          "gaps": ["未找到投资估算章节"]
        }"#;
        let h = parse_worker_handoff(raw).expect("handoff");
        assert_eq!(h.summary, "立项报告，结构：现状→目标→路径");
        assert_eq!(h.coverage, "partial");
        assert_eq!(h.gaps, vec!["未找到投资估算章节".to_string()]);
        // K3: key_facts parsing dropped (SELECTED + hydration owns facts);
        // the field stays serde-compatible but is no longer populated.
        assert!(h.key_facts.is_empty());
        assert!(!h.is_full_coverage());
        assert!(h.premise_mismatch.is_none());
    }

    // ---- S3: gray-zone schema (premise_mismatch; key_facts deprecated) ------

    #[test]
    fn parses_premise_mismatch() {
        let raw = r#"{
          "schema_version": "internal_worker_handoff_v1",
          "summary": "文档核心框架是 4R",
          "coverage": "partial",
          "gaps": [],
          "premise_mismatch": {
            "kind": "frame",
            "detail": "问题预设的 4P 拆解属于竞争对手南通四方",
            "actual_subject": "Y公司策略为 4R 框架（关联/反应/关系/回报）"
          }
        }"#;
        let h = parse_worker_handoff(raw).expect("handoff");
        assert!(h.key_facts.is_empty(), "K3: facts no longer parsed");
        let pm = h.premise_mismatch.expect("premise_mismatch");
        assert_eq!(pm.kind, "frame");
        assert!(pm.detail.contains("4P"));
        assert_eq!(
            pm.actual_subject.as_deref(),
            Some("Y公司策略为 4R 框架（关联/反应/关系/回报）")
        );
    }

    #[test]
    fn incomplete_premise_mismatch_is_ignored() {
        let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"s","key_facts":[],"coverage":"insufficient","gaps":["x"],"premise_mismatch":{"kind":"frame"}}"#;
        let h = parse_worker_handoff(raw).expect("handoff");
        assert!(h.premise_mismatch.is_none(), "missing detail → ignored");
    }

    #[test]
    fn handoff_serde_roundtrip_keeps_gray_fields() {
        let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"s","key_facts":[{"claim":"c","evidence":[],"basis":"inferred"}],"coverage":"partial","gaps":[],"premise_mismatch":{"kind":"entity","detail":"d"}}"#;
        let h = parse_worker_handoff(raw).expect("handoff");
        let json = serde_json::to_string(&h).unwrap();
        let back: WorkerHandoff = serde_json::from_str(&json).unwrap();
        assert_eq!(h, back);
        // Old outputs without the new fields still deserialize (serde additive).
        let old = r#"{"summary":"s","coverage":"partial"}"#;
        let legacy: WorkerHandoff = serde_json::from_str(old).unwrap();
        assert!(legacy.key_facts.is_empty());
        assert!(legacy.premise_mismatch.is_none());
    }


    #[test]
    fn peels_legacy_internal_answer_v1() {
        let raw = r#"{"schema_version":"internal_answer_v1","answer_text":"结论正文","citations":[],"coverage":"full","refusal_reason":null}"#;
        let h = parse_worker_handoff(raw).expect("handoff");
        assert_eq!(h.summary, "结论正文");
        assert_eq!(h.coverage, "full");
        assert!(h.gaps.is_empty());
        assert!(h.is_full_coverage());
    }

    #[test]
    fn freeform_text_becomes_summary_not_degraded() {
        // K3: prose (or any non-envelope output) is a legal handoff — the
        // message becomes the summary with the partial default, NOT degraded.
        for raw in [
            "散文式摘要：文档讲了三件事",
            "<code language=\"python\">\nchunks = await client.dense_search(query=\"保修\")\n</code>",
        ] {
            let h = parse_worker_handoff(raw).expect("prose handoff");
            assert!(!h.handoff_degraded, "prose is not degraded: {raw}");
            assert_eq!(h.coverage, "partial");
            assert_eq!(h.summary, raw.trim());
            assert!(h.key_facts.is_empty());
        }
    }

    #[test]
    fn fenced_json_handoff_is_accepted() {
        let raw = "```json\n{\"summary\":\"s\",\"coverage\":\"full\",\"gaps\":[],\"key_facts\":[]}\n```";
        let h = parse_worker_handoff(raw).expect("handoff");
        assert_eq!(h.summary, "s");
        assert_eq!(h.coverage, "full");
    }

    #[test]
    fn c5_final_turn_output_shape_parses_as_handoff() {
        // The budget-exhaustion final turn (agent-loop C5) asks for the bare
        // internal_worker_handoff_v1 JSON object — no fences, no code blocks.
        // Prove that exact shape reaches the handoff parser as structured JSON.
        let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"论文未记载保修年限","key_facts":[],"coverage":"insufficient","gaps":["保修年限"]}"#;
        let h = parse_worker_handoff(raw).expect("handoff");
        assert_eq!(h.summary, "论文未记载保修年限");
        assert_eq!(h.coverage, "insufficient");
        assert_eq!(h.gaps, vec!["保修年限".to_string()]);
    }

    // ---- C4: deterministic handoff validation / sanitization -------------

    fn ok_chunk_result(chunk_id: &str) -> contracts::ToolResult {
        contracts::ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1.0".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!([
                {"chunk_id": chunk_id, "doc_id": "d1", "text": "evidence", "score": 0.9}
            ])),
            trace: None,
        }
    }

    fn run_with(answer: &str, tools: Vec<contracts::ToolResult>) -> AgentRunResult {
        let mut r = AgentRunResult::default();
        r.answer = answer.to_string();
        r.tool_results = tools;
        r
    }

    #[test]
    fn fabricated_execution_result_is_stripped_from_valid_handoff() {
        // q039: an otherwise valid handoff carrying a fabricated
        // <code_execution_result> block — content must be stripped, handoff
        // marked degraded.
        let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"见 <code_execution_result>韩方投资者 B株式会社 持股40%</code_execution_result> 所示","key_facts":[],"coverage":"full","gaps":[]}"#;
        let h = worker_handoff_from_run(&run_with(raw, vec![])).expect("handoff");
        assert!(!h.summary.contains("B株式会社"), "{}", h.summary);
        assert!(!h.summary.contains("code_execution_result"));
        assert!(h.handoff_degraded);
    }

    #[test]
    fn valid_handoff_with_observed_chunk_ids_passes_untouched() {
        let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"2019年建厂","key_facts":[{"claim":"2019年建厂","evidence":["c1"]}],"coverage":"full","gaps":[]}"#;
        let h = worker_handoff_from_run(&run_with(raw, vec![ok_chunk_result("c1")])).expect("handoff");
        assert!(!h.handoff_degraded);
        // K3: model-written key_facts are ignored (hydration owns facts).
        assert!(h.key_facts.is_empty());
        assert_eq!(h.coverage, "full");
    }

    // ---- K3: degraded = E104 stripped or E105 rejected only ------------------

    /// The post-loop safety net (same channel the C5 budget-exhausted final
    /// turn's output flows through): E105 rejection degrades with codes.
    #[test]
    fn e105_rejection_degrades_with_code() {
        let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"未找到","key_facts":[],"coverage":"insufficient","gaps":["x"]}"#;
        let h = worker_handoff_from_run(&run_with(raw, vec![])).expect("handoff");
        assert!(h.handoff_degraded);
        assert!(
            h.compile_diagnostics.contains(&"E105".to_string()),
            "{:?}",
            h.compile_diagnostics
        );
    }

    #[test]
    fn task_result_wrapper_is_prose_not_degraded() {
        // K3: q045's self-invented wrapper is no longer an error — the raw
        // message becomes the (JSON-text) summary, not degraded.
        let raw = r#"{"task_result":{"summary":"文中未写明总部城市"}}"#;
        let h = worker_handoff_from_run(&run_with(raw, vec![ok_chunk_result("c1")])).expect("handoff");
        assert!(!h.handoff_degraded);
        assert!(h.summary.contains("task_result"), "{}", h.summary);
        assert!(h.compile_diagnostics.is_empty());
    }

    #[test]
    fn fabricated_block_strips_with_e104_code() {
        let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"见 <code_execution_result>捏造输出</code_execution_result> 所示","key_facts":[],"coverage":"insufficient","gaps":["x"]}"#;
        let h = worker_handoff_from_run(&run_with(raw, vec![])).expect("handoff");
        assert!(h.handoff_degraded, "E104 transformation marks degraded");
        assert!(!h.summary.contains("捏造输出"), "{}", h.summary);
        assert!(
            h.compile_diagnostics.contains(&"E104".to_string()),
            "{:?}",
            h.compile_diagnostics
        );
    }

    #[test]
    fn clean_handoff_has_no_compile_diagnostics() {
        let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"2019年建厂","key_facts":[{"claim":"2019年建厂","evidence":["c1"]}],"coverage":"full","gaps":[]}"#;
        let h = worker_handoff_from_run(&run_with(raw, vec![ok_chunk_result("c1")])).expect("handoff");
        assert!(!h.handoff_degraded);
        assert!(h.compile_diagnostics.is_empty());
    }

    #[test]
    fn broken_open_marker_does_not_swallow_following_valid_eids() {
        // Acceptance defect: model wrote `[[E15]目录]` (one `]`), then valid
        // `[[E1]]` / `[[E2]]`. Greedy `find("]]")` used to glue them into one
        // token and drop the valid citations.
        let store = store_with_both();
        let mut r = AgentRunResult::default();
        r.answer = "坏开 [[E15]目录] 然后合法 [[E1]] 与 [[E2]]。".into();
        finalize_answer_evidence(&mut r, &store);

        assert!(
            r.answer.contains("[[cite:chunk-a]]"),
            "doc marker must survive: {}",
            r.answer
        );
        assert!(
            r.answer.contains("[[web:"),
            "web marker must survive: {}",
            r.answer
        );
        assert_eq!(r.citations.len(), 2, "both valid eids cited: {:?}", r.citations);
        // Broken opener remains as plain text (leading `[[` rescanned); no product markers invented.
        assert!(
            r.answer.contains("[[") || r.answer.contains("E15"),
            "broken fragment still visible as text: {}",
            r.answer
        );
    }

    #[test]
    fn worker_observability_keeps_real_tools_and_thinking() {
        use agent_loop::events::AgentEvent;
        use agent_loop::runtime::{FinalDecision, IterationRecord};
        use contracts::{ToolResult, ToolStatus, ToolTrace};

        let mut run = AgentRunResult::default();
        run.reasoning_summary = Some("先用关键词检索再图扩展".into());
        run.total_tool_calls = 2;
        run.total_elapsed_ms = Some(1200);
        run.final_decision = Some(FinalDecision::Synthesized);
        run.tool_results = vec![
            ToolResult {
                tool: "lexical_retrieval".into(),
                version: "1.0".into(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!([{"chunk_id": "c1"}])),
                trace: Some(ToolTrace {
                    elapsed_ms: Some(40),
                    raw_hit_count: Some(1),
                    hydrated_hit_count: Some(1),
                    degrade_reason: None,
                }),
            },
            ToolResult {
                tool: "graph_retrieval".into(),
                version: "1.0".into(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!({
                    "graph_context": [{"id": "e1"}],
                    "source": "graph_augment",
                })),
                trace: Some(ToolTrace {
                    elapsed_ms: Some(12),
                    raw_hit_count: Some(1),
                    hydrated_hit_count: None,
                    degrade_reason: Some("graph_augment".into()),
                }),
            },
        ];
        run.iterations = vec![IterationRecord {
            iteration: 0,
            plan: serde_json::json!({
                "action_type": "codegen",
                "observation_preview": "lexical hits",
                "disclosed_skills": ["codegen"],
                "exit_reason": "native_tool_call",
            }),
            signals: Default::default(),
            decision: "continue".into(),
            elapsed_ms: 900,
            llm_evaluation: None,
            usage: None,
        }];
        run.answer = r#"{"schema_version":"internal_worker_handoff_v1","summary":"找到了站点编码","coverage":"partial","gaps":[],"key_facts":[]}"#.into();

        let events = vec![
            AgentEvent::PlanDecision {
                selected_tools: vec![],
                selected_skills: vec!["codegen".into()],
                selected_writing_styles: vec![],
                behavior_mode: None,
                reasoning: "retrieve iteration 0, skills: [codegen]".into(),
            },
            AgentEvent::Evaluation {
                signals: Some(serde_json::json!({
                    "action_type": "codegen",
                    "disclosed_skills": ["codegen"],
                })),
                decision: "continue".into(),
                reasoning: "need more evidence".into(),
            },
            AgentEvent::ToolResult {
                tool: "code_gen".into(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!({ "result": "await client.lexical_search(...)" })),
                elapsed_ms: 50,
            },
        ];
        attach_worker_thinking_events(&mut run, &events);

        let obs = worker_observability_from_run(Channel::Rag, &run);
        assert_eq!(obs.channel, Channel::Rag);
        assert_eq!(
            obs.tools
                .iter()
                .map(|t| t.tool.as_str())
                .collect::<Vec<_>>(),
            vec!["lexical_retrieval", "graph_retrieval"]
        );
        assert_eq!(
            obs.tools[1].degrade_reason.as_deref(),
            Some("graph_augment")
        );
        assert_eq!(
            obs.reasoning_summary.as_deref(),
            Some("先用关键词检索再图扩展")
        );
        assert!(
            obs.thinking.iter().any(|s| s.kind == "plan"),
            "plan thinking: {:?}",
            obs.thinking
        );
        assert!(
            obs.thinking.iter().any(|s| s.kind == "eval"),
            "eval thinking: {:?}",
            obs.thinking
        );
        assert!(
            obs.thinking.iter().any(|s| s.kind == "codegen"),
            "codegen thinking: {:?}",
            obs.thinking
        );
        assert_eq!(obs.handoff_summary.as_deref(), Some("找到了站点编码"));
        assert_eq!(obs.final_decision.as_deref(), Some("synthesized"));
        assert_eq!(obs.iterations.len(), 1);

        // Store bridge may collapse citable chunks to dense_retrieval for eval —
        // worker obs must stay independent of that label.
        let mut store = EvidenceStore::default();
        let _ = store.insert_from_tool_results(Channel::Rag, &run.tool_results);
        let bridged = store.as_retrieval_tool_results();
        if !bridged.is_empty() {
            assert!(
                bridged.iter().any(|t| t.tool == "dense_retrieval"),
                "eval bridge labels store evidence as dense_retrieval"
            );
        }
        assert!(
            !obs.tools.iter().any(|t| t.tool == "dense_retrieval"),
            "worker obs must keep real tool names, not store bridge labels"
        );
    }

    #[test]
    fn worker_observability_falls_back_to_iterations_when_no_sink() {
        use agent_loop::runtime::IterationRecord;

        let mut run = AgentRunResult::default();
        run.iterations = vec![IterationRecord {
            iteration: 0,
            plan: serde_json::json!({
                "action_type": "codegen",
                "observation_preview": "empty",
                "disclosed_skills": ["codegen"],
            }),
            signals: Default::default(),
            decision: "synthesize".into(),
            elapsed_ms: 10,
            llm_evaluation: None,
            usage: None,
        }];
        let obs = worker_observability_from_run(Channel::Search, &run);
        assert_eq!(obs.thinking.len(), 1);
        assert_eq!(obs.thinking[0].kind, "iteration");
        assert_eq!(obs.thinking[0].decision.as_deref(), Some("synthesize"));
    }
}
