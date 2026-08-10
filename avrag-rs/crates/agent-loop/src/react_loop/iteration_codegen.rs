use std::sync::Arc;

use avrag_llm::{ChatMessage, LlmResponse};
use common::AppError;
use contracts::ToolResult;

use super::reasoning_emit;
use super::telemetry::ReActIterationRecord;
use super::{ReActLoop, truncate_observation, truncate_preview};
use crate::events::{AgentEvent, AgentEventSink};
use crate::runtime::AgentRequest;

use super::iteration::{
    IterationControl, IterationOutcome, IterationState, disclosed_skill_ids, iteration_llm_usage,
};

impl ReActLoop {
    pub(super) async fn dispatch_codegen(
        &self,
        iteration: u8,
        request: &AgentRequest,
        auth: &contracts::auth_runtime::AuthContext,
        state: &mut IterationState,
        sink: &dyn AgentEventSink,
        llm_response: &LlmResponse,
        _iter_start: std::time::Instant,
        codes: Vec<String>,
        hooks: &dyn super::LoopHooks,
    ) -> Result<IterationOutcome, AppError> {
        let llm_usage = iteration_llm_usage(llm_response);
        let code_start = std::time::Instant::now();
        let interpreter_lock = Arc::clone(&self.deps.code_interpreter);
        let mut combined_result = String::new();
        let mut any_error = false;
        let mut any_output = false;
        let mut bridge_tool_results = Vec::new();
        // K1: adaptive-k hints + per-round retrieval summary.
        let mut all_bridge_calls = Vec::new();

        // E6 (2026-07-28): one code block per round, mechanically enforced —
        // only the FIRST extracted block executes; extra blocks are skipped
        // with a warning observation. A 12-block burst (5 errors) used to
        // trip the consecutive-sandbox-error breaker in a single round.
        let skipped_blocks = codes.len().saturating_sub(1);
        for (idx, code) in codes.iter().take(1).enumerate() {
            // User-facing "running" progress before the sandbox executes
            // (2026-07-23: dispatch-phase silence — retrieval now announces
            // start, not just finish). One step per detected client.* call.
            for (kind, product, query) in preview_codegen_client_calls(code) {
                crate::progress::emit_work_fact(
                    sink,
                    crate::progress::WorkFact::retrieval_started(kind, product, &query),
                )
                .await;
            }
            let (
                block_status,
                block_text,
                is_err,
                mut block_bridge_results,
                mut bridge_calls,
                block_had_output,
            ) = self
                .execute_codegen_block(
                    idx,
                    code,
                    request,
                    auth,
                    &interpreter_lock,
                    &state.retrieval_aliases,
                    &state.evidence.seen_chunk_aliases,
                    &state.evidence.seen_chunk_bodies,
                    &state.session_fs,
                    &state.sdk_allowed,
                    &state.knockout,
                )
                .await;
            // Align captures with bridge filter (no double-count): tool_results +
            // BridgeCallObs used by evidence intake / progress / callouts.
            if let Ok(ko) = state.knockout.lock() {
                ko.align_tool_results_no_count(&mut block_bridge_results);
                for call in &mut bridge_calls {
                    if let Some(data) = call.result.data.as_mut() {
                        ko.align_value_no_count(data);
                    }
                }
            }
            // User-facing progress: one step per bridge client.* call (not codegen itself).
            // B3: same after_tool_call observation surface as native tools.
            for call in &bridge_calls {
                let status = match call.result.status {
                    contracts::ToolStatus::Ok => "ok",
                    contracts::ToolStatus::Timeout => "timeout",
                    contracts::ToolStatus::Error => "error",
                    contracts::ToolStatus::NotFound => "not_found",
                    contracts::ToolStatus::NotImplemented => "not_implemented",
                };
                hooks.after_tool_call(&call.method, status);
                if let Some((kind, product)) = crate::progress::bridge_method_progress(&call.method)
                {
                    // Same guard as native tools: failed calls are not empty results.
                    if call.result.status != contracts::ToolStatus::Ok {
                        continue;
                    }
                    let query = call.query.as_deref().unwrap_or("");
                    let hits = crate::progress::hits_from_tool_data(call.result.data.as_ref());
                    let docs =
                        crate::progress::doc_labels_from_tool_data(call.result.data.as_ref());
                    crate::progress::emit_work_fact(
                        sink,
                        crate::progress::WorkFact::retrieval_finished(
                            kind, product, query, hits, &docs,
                        ),
                    )
                    .await;
                }
            }
            any_output = any_output || block_had_output || !bridge_calls.is_empty();
            all_bridge_calls.extend(bridge_calls);
            bridge_tool_results.extend(block_bridge_results);
            combined_result.push_str(&block_text);
            combined_result.push('\n');
            if is_err {
                any_error = true;
            }

            let _ = sink
                .emit(AgentEvent::ToolResult {
                    tool: "code_gen".to_string(),
                    status: block_status,
                    data: Some(serde_json::json!({ "result": block_text })),
                    elapsed_ms: code_start.elapsed().as_millis() as u64,
                })
                .await;
        }

        let elapsed_ms = code_start.elapsed().as_millis() as u64;
        if skipped_blocks > 0 {
            combined_result.push_str(&super::prompt_assets::blocks_skipped_nudge(
                codes.len(),
                skipped_blocks,
            ));
            combined_result.push('\n');
        }
        // C3: a round with completely empty stdout AND stderr AND zero bridge
        // calls gets an explicit note — otherwise the model guesses why the
        // observation is blank (and typically re-emits the same block).
        let no_output = !any_output && bridge_tool_results.is_empty();
        // Count consecutive failures before building observation so the nudge
        // can report n_fail/n_max as environment facts (not after the fact).
        let n_fail = if any_error {
            state.consecutive_sandbox_errors = state.consecutive_sandbox_errors.saturating_add(1);
            // Re-inject KB L2 api-detail next retrieve round (method recovery).
            state.disclosed.reexpose_kb_api_detail = true;
            state.consecutive_sandbox_errors
        } else {
            0
        };
        // Evidence Intake at Ok-retrieval boundary (durable; not observation format).
        state.evidence.intake_from_bridge_calls(&all_bridge_calls);
        // Knockout reexpose observation (3rd post-knock hit).
        let reexposed = if let Ok(mut ko) = state.knockout.lock() {
            ko.take_reexposed()
        } else {
            Vec::new()
        };
        let knockout_obs = if reexposed.is_empty() {
            String::new()
        } else {
            let list = crate::helpers::KnockoutState::format_reexpose_list(&reexposed);
            format!(
                "\n{}",
                super::prompt_assets::knockout_reexposed_observation(&list)
            )
        };
        let observation = format!(
            "{}{}{}{}",
            format_codegen_observation(
                &combined_result,
                any_error,
                no_output,
                n_fail,
                Self::MAX_CONSECUTIVE_SANDBOX_ERRORS,
            ),
            retrieval_callouts(
                &all_bridge_calls,
                &state.evidence.seen_retrieval_aliases,
            ),
            markitdown_format_hints(&codes),
            knockout_obs,
        );
        self.append_codegen_messages(state, llm_response, &observation);

        if any_error {
            // C2: bridge calls that succeeded before the block errored produced
            // real evidence — preserve it instead of discarding it with the
            // errored round. Error-status items are filtered downstream
            // (EvidenceStore::insert_from_tool_results skips non-Ok results).
            Self::record_bridge_evidence(state, &combined_result, bridge_tool_results);
            // Re-apply KEEP after new aliases land (same-turn KEEP wiring fix).
            Self::apply_ews_from_model_text(state, &llm_response.content);
            if let Some(outcome) = self
                .handle_codegen_error(iteration, state, sink, &llm_usage, elapsed_ms)
                .await
            {
                return Ok(outcome);
            }
        } else {
            self.record_codegen_success(state, &combined_result, bridge_tool_results);
            Self::apply_ews_from_model_text(state, &llm_response.content);
        }

        // E6: count executed blocks only (skipped blocks never ran).
        state.total_tool_calls += codes.len().min(1) as u32;
        let exit_reason = if any_error {
            "code_gen_error".to_string()
        } else {
            "code_gen".to_string()
        };
        Ok(IterationOutcome {
            control: IterationControl::Continue,
            record: Some(ReActIterationRecord {
                iteration,
                disclosed_skills: disclosed_skill_ids(&state.disclosed),
                action_type: exit_reason.clone(),
                observation_preview: truncate_preview(&observation, 200),
                llm_usage: Some(llm_usage),
                elapsed_ms,
                exit_reason,
            }),
            sandbox_break: false,
        })
    }

    fn append_codegen_messages(
        &self,
        state: &mut IterationState,
        llm_response: &LlmResponse,
        combined_result: &str,
    ) {
        state.messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: llm_response.content.clone(),
            name: None,
            tool_call_id: None,
            tool_calls: None,
            multimodal_content: None,
            reasoning_content: llm_response.reasoning_content.clone(),
        });
        state.messages.push(ChatMessage {
            role: "user".to_string(),
            content: format_codegen_result_message(combined_result),
            name: None,
            tool_call_id: None,
            tool_calls: None,
            multimodal_content: None,
            reasoning_content: None,
        });
    }

    /// Host break after this many **consecutive** sandbox failures (Traceback /
    /// non-zero exit / `!success`). Lower values starve model repair turns;
    /// higher values burn round budget. Raised 2→4 (2026-08-05).
    ///
    /// Counter is incremented **before** this call when `any_error`; here we
    /// only decide whether to break.
    pub(crate) const MAX_CONSECUTIVE_SANDBOX_ERRORS: u8 = 4;

    async fn handle_codegen_error(
        &self,
        iteration: u8,
        state: &mut IterationState,
        sink: &dyn AgentEventSink,
        llm_usage: &crate::runtime::AgentRunUsage,
        elapsed_ms: u64,
    ) -> Option<IterationOutcome> {
        if state.consecutive_sandbox_errors < Self::MAX_CONSECUTIVE_SANDBOX_ERRORS {
            return None;
        }
        let disclosed_skills = disclosed_skill_ids(&state.disclosed);
        let msg = format!(
            "consecutive sandbox errors ({}/{}), breaking to synthesis",
            state.consecutive_sandbox_errors,
            Self::MAX_CONSECUTIVE_SANDBOX_ERRORS
        );
        reasoning_emit::emit_evaluation_telemetry(
            sink,
            iteration,
            "sandbox_break_to_synthesis",
            &msg,
            &disclosed_skills,
            "sandbox_break_to_synthesis",
        )
        .await;
        let _ = sink
            .emit(AgentEvent::Activity {
                stage: "sandbox_error".to_string(),
                message: msg.clone(),
                detail: Some(msg),
                counts: Default::default(),
                sources_preview: Vec::new(),
            })
            .await;
        // Record the break so eval loop_rounds is not "silent" (previously
        // sandbox_break skipped TurnEnd and left only the prior code_gen_error).
        let exit_reason = "sandbox_break_to_synthesis".to_string();
        Some(IterationOutcome {
            control: IterationControl::BreakToSynthesis {
                reason: exit_reason.clone(),
            },
            record: Some(ReActIterationRecord {
                iteration,
                disclosed_skills,
                action_type: exit_reason.clone(),
                observation_preview: truncate_preview(
                    &format!(
                        "consecutive sandbox errors {}/{}",
                        state.consecutive_sandbox_errors,
                        Self::MAX_CONSECUTIVE_SANDBOX_ERRORS
                    ),
                    200,
                ),
                llm_usage: Some(llm_usage.clone()),
                elapsed_ms,
                exit_reason,
            }),
            // Keep true so callers can still detect the break path; telemetry
            // now has a record — run_retrieval emits TurnEnd when record is Some
            // only if !sandbox_break. Prefer visible record: clear sandbox_break
            // skip so the break reason lands in mode_debug.exit_reasons.
            sandbox_break: false,
        })
    }

    fn record_codegen_success(
        &self,
        state: &mut IterationState,
        combined_result: &str,
        bridge_tool_results: Vec<ToolResult>,
    ) {
        state.consecutive_sandbox_errors = 0;
        Self::record_bridge_evidence(state, combined_result, bridge_tool_results);
    }

    /// Extend `state.tool_results` with bridge-captured retrieval results (or
    /// the stdout-fallback parse when the bridge captured nothing). Shared by
    /// the success path and the errored-round path (C2): evidence from a
    /// successful bridge call is valid even when the enclosing block errored.
    fn record_bridge_evidence(
        state: &mut IterationState,
        combined_result: &str,
        bridge_tool_results: Vec<ToolResult>,
    ) {
        if !bridge_tool_results.is_empty() {
            state.tool_results.extend(bridge_tool_results);
        } else if let Some(result) =
            crate::helpers::tool_result_from_code_execution_observation(combined_result)
        {
            state.tool_results.push(result);
        }
    }

    async fn execute_codegen_block(
        &self,
        idx: usize,
        code: &str,
        request: &AgentRequest,
        auth: &contracts::auth_runtime::AuthContext,
        interpreter_lock: &Arc<std::sync::Mutex<Option<avrag_code_interpreter::CodeInterpreter>>>,
        retrieval_aliases: &Arc<std::sync::atomic::AtomicU64>,
        seen_chunk_aliases: &Arc<
            std::sync::Mutex<std::collections::HashMap<String, String>>,
        >,
        seen_chunk_bodies: &Arc<
            std::sync::Mutex<std::collections::HashMap<String, String>>,
        >,
        session_fs: &Arc<super::session_fs::SessionFs>,
        sdk_allowed: &Arc<std::collections::HashSet<String>>,
        knockout: &crate::helpers::SharedKnockout,
    ) -> (
        contracts::ToolStatus,
        String,
        bool,
        Vec<ToolResult>,
        Vec<super::deps::BridgeCallObs>,
        bool,
    ) {
        let code = code.to_string();
        let interpreter_lock = Arc::clone(interpreter_lock);
        let exec_result: Result<
            avrag_code_interpreter::ExecutionResult,
            avrag_code_interpreter::InterpreterError,
        >;
        let mut block_observation_stdout: Option<String> = None;
        let mut block_bridge_results = Vec::new();
        let mut bridge_calls = Vec::new();

        if self.deps.sdk_can_bridge(sdk_allowed) {
            // CodegenPort: SaC host (retrieval + base/web/memory + fs) stays inside deps.
            let session_id = request
                .session_id
                .as_deref()
                .and_then(|s| uuid::Uuid::parse_str(s).ok());
            let meta_str = |key: &str| {
                request
                    .metadata
                    .get(key)
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            };
            let bridged = self
                .deps
                .execute_codegen_bridged_with_session(
                    &code,
                    auth,
                    &request.doc_scope,
                    Arc::clone(retrieval_aliases),
                    Arc::clone(seen_chunk_aliases),
                    Arc::clone(seen_chunk_bodies),
                    session_id,
                    Arc::clone(session_fs),
                    meta_str("client_ip"),
                    meta_str("client_local_time"),
                    meta_str("client_timezone"),
                    Arc::clone(sdk_allowed),
                    Some(Arc::clone(knockout)),
                )
                .await;
            block_bridge_results = bridged.bridge_results;
            bridge_calls = bridged.bridge_calls;
            exec_result = match bridged.exec {
                Ok(exec) => {
                    block_observation_stdout = Some(crate::helpers::codegen_observation_stdout(
                        &exec.stdout,
                        &block_bridge_results,
                    ));
                    Ok(exec)
                }
                Err(e) => Err(e),
            };
        } else {
            let interpreter_lock = Arc::clone(&interpreter_lock);
            let join_result = tokio::task::spawn_blocking(move || {
                let mut guard = interpreter_lock.lock().unwrap_or_else(|e| e.into_inner());
                if guard.is_none() {
                    *guard = Some(avrag_code_interpreter::CodeInterpreter::new());
                }
                guard.as_ref().unwrap().execute(&code)
            })
            .await;
            exec_result = match join_result {
                Ok(result) => result,
                Err(e) => Err(avrag_code_interpreter::InterpreterError::Bridge(format!(
                    "interpreter task panicked: {e}"
                ))),
            };
        }

        match exec_result {
            Ok(exec) => {
                let is_err = code_exec_is_error(&exec);
                let status = if is_err {
                    contracts::ToolStatus::Error
                } else {
                    contracts::ToolStatus::Ok
                };
                let stdout_for_observation = block_observation_stdout
                    .as_deref()
                    .unwrap_or(exec.stdout.as_str());
                let text = avrag_rag_core::runtime::markers::format_block(
                    idx,
                    stdout_for_observation,
                    &exec.stderr,
                );
                // C3: whether the block produced ANY visible output (stdout or
                // stderr) — drives the empty-round feedback note.
                let had_output = !exec.stdout.trim().is_empty() || !exec.stderr.trim().is_empty();
                (
                    status,
                    text,
                    is_err,
                    block_bridge_results,
                    bridge_calls,
                    had_output,
                )
            }
            Err(e) => {
                // P3-1: failure form delegates to the same grammar owner as the
                // success form (rag-core markers::format_block_failure).
                let text =
                    avrag_rag_core::runtime::markers::format_block_failure(idx, &e.to_string());
                (
                    contracts::ToolStatus::Error,
                    text,
                    true,
                    block_bridge_results,
                    bridge_calls,
                    false,
                )
            }
        }
    }
}

/// K1: per-round retrieval summary (「本轮检索 N 次，共返回 M 条」) plus the
/// adaptive-k coaching hints carried on the captured calls' data
/// (`retrieval_hint` set by dense/lexical tools). Model-visible suffix of
/// the codegen observation; empty when no retrieval happened this round.
///
/// **Pure format** (no claim board mutation). Durable Intake runs earlier via
/// `EvidencePool::intake_from_bridge_calls`. This still updates
/// `seen_aliases` for the model-visible saturation signal (new vs already-seen).
fn retrieval_callouts(
    bridge_calls: &[super::deps::BridgeCallObs],
    seen_aliases: &std::sync::Mutex<std::collections::HashSet<String>>,
) -> String {
    use super::evidence_pool::RETRIEVAL_INTAKE_METHODS;
    let call_count = bridge_calls
        .iter()
        .filter(|c| RETRIEVAL_INTAKE_METHODS.contains(&c.method.as_str()))
        .count();
    let mut total_chunks = 0usize;
    let mut hints = String::new();
    let mut aliases: Vec<String> = Vec::new();
    let mut any_truncated = false;
    let mut any_grep_zero = false;
    let mut expanded = 0usize;
    let mut cards = 0usize;
    let mut stubs = 0usize;
    let mut expand_chars = 0usize;
    for call in bridge_calls {
        if call.result.status != contracts::ToolStatus::Ok {
            continue;
        }
        total_chunks += crate::progress::hits_from_tool_data(call.result.data.as_ref());
        if let Some(data) = call.result.data.as_ref() {
            collect_aliases_from_tool_data(data, &mut aliases);
            if data.get("truncated").and_then(|v| v.as_bool()) == Some(true) {
                any_truncated = true;
            }
            if call.method == "grep"
                && data
                    .get("total_hits")
                    .and_then(|v| v.as_u64())
                    .is_some_and(|n| n == 0)
            {
                any_grep_zero = true;
            }
            if let Some(hint) = data.get("retrieval_hint").and_then(|v| v.as_str()) {
                hints.push_str("\n\n[retrieval_hint] ");
                hints.push_str(hint);
            }
            expanded += data
                .get("visibility_expanded_n")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            cards += data
                .get("visibility_card_n")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            stubs += data
                .get("visibility_stub_n")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            expand_chars += data
                .get("visibility_expand_chars")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
        }
    }
    if call_count == 0 && hints.is_empty() {
        return String::new();
    }
    let mut seen = seen_aliases.lock().unwrap_or_else(|e| e.into_inner());
    let new_aliases: Vec<&String> = aliases.iter().filter(|a| !seen.contains(*a)).collect();
    let seen_count = aliases.len() - new_aliases.len();
    seen.extend(aliases.iter().cloned());
    drop(seen);
    let detail = build_retrieval_summary_detail(
        &aliases,
        any_truncated,
        any_grep_zero,
        new_aliases.len(),
        seen_count,
    );
    let mut out = String::from("\n\n");
    out.push_str(&super::prompt_assets::retrieval_summary(
        call_count,
        total_chunks,
        &detail,
    ));
    if expanded + cards + stubs > 0 {
        out.push_str("\n\n");
        out.push_str(&super::prompt_assets::evidence_index(
            expanded,
            cards,
            stubs,
            expand_chars,
            aliases.len().saturating_add(seen_count),
        ));
    }
    out.push_str(&hints);
    out
}

fn collect_aliases_from_tool_data(data: &serde_json::Value, out: &mut Vec<String>) {
    let push_alias = |item: &serde_json::Value, out: &mut Vec<String>| {
        if let Some(a) = item.get("alias").and_then(|v| v.as_str()) {
            if !out.iter().any(|x| x == a) {
                out.push(a.to_string());
            }
        }
    };
    match data {
        serde_json::Value::Array(arr) => {
            for item in arr {
                push_alias(item, out);
            }
        }
        serde_json::Value::Object(_) => {
            if let Some(arr) = data.get("chunks").and_then(|v| v.as_array()) {
                for item in arr {
                    push_alias(item, out);
                }
            }
            if let Some(arr) = data.get("hits").and_then(|v| v.as_array()) {
                for item in arr {
                    push_alias(item, out);
                }
            }
        }
        _ => {}
    }
}

/// Thin wrapper: observation prose is authored under `prompts/loop/retrieval-summary-detail-*`.
fn build_retrieval_summary_detail(
    aliases: &[String],
    any_truncated: bool,
    any_grep_zero: bool,
    new_aliases: usize,
    seen_aliases: usize,
) -> String {
    super::prompt_assets::retrieval_summary_detail(
        aliases,
        any_truncated,
        any_grep_zero,
        new_aliases,
        seen_aliases,
    )
}

/// markitdown 静态格式校验（2026-07-29，spec §5 静态层）：执行前扫代码块中
/// 的格式符号，与 markitdown 输出契约（SKILL「markitdown 输出契约」节）比对，
/// 高置信不符才提醒（宁缺勿滥——误报=新一轮误导）。规则 v1：
///   ① `|值`（管道后紧跟 CJK，无空格）——markitdown 单元格恒为空格填充；
///   ② key=value 过滤形（`阶段=值`）——markitdown 表格无 key=value 形式。
/// 每条规则至多报一次；提醒进 observation，不阻断执行。
fn markitdown_format_hints(codes: &[String]) -> String {
    use std::sync::OnceLock;
    fn rules() -> &'static [(&'static str, regex::Regex, &'static str)] {
        static RULES: OnceLock<[(&'static str, regex::Regex, &'static str); 2]> = OnceLock::new();
        RULES.get_or_init(|| {
            [
                (
                    "no_space_pipe",
                    regex::Regex::new(r"\|\p{Han}").expect("valid regex"),
                    "no_space_pipe",
                ),
                (
                    "key_value_form",
                    regex::Regex::new(r"\p{Han}{2,}=\p{Han}").expect("valid regex"),
                    "key_value_form",
                ),
            ]
        })
    }
    let mut out = String::new();
    for (_, re, key) in rules() {
        if codes.iter().any(|code| re.is_match(code)) {
            out.push_str("\n\n");
            out.push_str(match *key {
                "no_space_pipe" => super::prompt_assets::format_hint_no_space_pipe(),
                "key_value_form" => super::prompt_assets::format_hint_key_value(),
                _ => "",
            });
        }
    }
    out
}

/// Maximum number of chars (not bytes) of sandbox/tool output re-injected into the LLM
/// context. Bounds untrusted content (which may include retrieved document text) so a
/// single malicious or oversized document cannot dominate the prompt.
const CODEGEN_OBSERVATION_MAX_CHARS: usize = 8000;

/// Wrap a codegen sandbox/tool observation for re-injection into the LLM, applying a
/// length cap and an explicit untrusted-content marker. This is a defense-in-depth measure
/// against indirect prompt injection: retrieved document text lives inside the sandbox
/// observation and must not be treated as system/user instructions.
///
/// The outer `<code_execution_result> ... </code_execution_result>` tag name is preserved
/// (the opening tag carries an `untrusted="true"` attribute) so downstream parsers such as
/// `code_execution_has_evidence` in `exit_policy.rs` still recognize the block.
pub(crate) fn format_codegen_result_message(combined_result: &str) -> String {
    let safe = truncate_observation(combined_result, CODEGEN_OBSERVATION_MAX_CHARS);
    let prefix = super::prompt_assets::codegen_untrusted_prefix();
    // 备案表中的空格形态（`<code_execution_result `）承载 untrusted 属性：
    // 发射端拼接 `untrusted="true">` 还原完整开标签；闭合形态（
    // `<code_execution_result>`）留给仿造外壳检测。
    let open = super::host_markers::HOST_OBSERVATION_MARKERS
        .iter()
        .find(|m| m.tag == "<code_execution_result ")
        .expect("code_execution_result (space form) marker registered")
        .tag;
    format!(
        "{open}untrusted=\"true\">\n\
         {prefix}\n\
         \n\
         {safe}\n\
         </code_execution_result>"
    )
}

/// Append sandbox error recovery hints so the next LLM turn can fix bad API calls.
/// `no_output` (C3): the round produced nothing at all — empty stdout AND
/// stderr AND zero bridge calls — so the model gets told instead of guessing.
/// `n_fail` / `n_max`: consecutive failure count and host break threshold.
fn format_codegen_observation(
    combined_result: &str,
    had_error: bool,
    no_output: bool,
    n_fail: u8,
    n_max: u8,
) -> String {
    let mut out = combined_result.to_string();
    if no_output {
        out.push_str("\n\n");
        out.push_str(super::prompt_assets::codegen_no_output_nudge());
    }
    if !had_error {
        return out;
    }
    // Always show `client.*` form — bare method names pollute the model into
    // inventing native tool_calls (dense_search as tool schema). Body from
    // prompts/loop/codegen-sandbox-error.nudge.md
    out.push_str("\n\n");
    out.push_str(&super::prompt_assets::codegen_sandbox_error_nudge(n_fail, n_max));
    out
}

/// Pre-scan a `<code>` block for `client.<method>(` calls so a "running"
/// progress step can be emitted before sandbox execution (2026-07-23).
/// Returns (kind, product label, best-effort query arg) per detected call.
fn preview_codegen_client_calls(
    code: &str,
) -> Vec<(crate::progress::ProgressKind, &'static str, String)> {
    let mut out = Vec::new();
    let mut rest = code;
    while let Some(pos) = rest.find("client.") {
        let after = &rest[pos + "client.".len()..];
        let name_end = after
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(after.len());
        let name = &after[..name_end];
        if let Some((kind, product)) = crate::progress::bridge_method_progress(name) {
            out.push((kind, product, extract_query_arg(&after[name_end..])));
        }
        rest = &after[name_end.max(1)..];
    }
    out
}

/// Naive best-effort `query="..."` extraction for a progress label — display
/// only, never parsed for execution.
fn extract_query_arg(s: &str) -> String {
    // Char-safe window: never slice mid multi-byte UTF-8 (Chinese progress labels).
    let window: String = s.chars().take(240).collect();
    let Some(pos) = window.find("query=") else {
        return String::new();
    };
    let rest = window[pos + "query=".len()..].trim_start();
    let Some(q) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else {
        return String::new();
    };
    let body = &rest[1..];
    let end = body
        .find(q)
        .unwrap_or_else(|| body.chars().take(48).map(|c| c.len_utf8()).sum());
    // `end` is a byte index into `body` only when found via `find`; for the
    // char-count fallback it is already a safe sum of utf8 lengths.
    body.get(..end).unwrap_or("").to_string()
}

/// Decide whether a sandbox execution should be treated as a failure.
///
/// Python routinely writes benign diagnostics (e.g. `DeprecationWarning`, pandas future
/// warnings) to stderr, and flagging any non-empty stderr as fatal caused false
/// sandbox-error classification and premature break-to-synthesis. So a non-empty stderr
/// alone is NOT treated as an error.
///
/// However, the Python sandbox wrapper catches all exceptions and reports them ONLY via a
/// traceback printed to stderr — `success` stays `true` and `exit_code` stays `0` even on a
/// `raise`. To keep detecting real errors (and thus the consecutive-error break-to-synthesis
/// safety net), we look for a `"Traceback"` marker, which appears for raised exceptions but
/// never for benign warnings.
fn code_exec_is_error(exec: &avrag_code_interpreter::ExecutionResult) -> bool {
    !exec.success || exec.exit_code.unwrap_or(0) != 0 || exec.stderr.contains("Traceback")
}

#[cfg(test)]
mod tests {
    use super::*;
    // B4: SDK-as-native reject lives in agent_tools (single execute entry).
    use agent_tools::{is_codegen_sdk_method_as_native_tool, reject_native_tool_surface};

    #[test]
    fn sandbox_error_observation_includes_sdk_reminder() {
        let raw = "[block 0] stdout: \nstderr: AttributeError: no attribute 'hybrid_search'\n";
        let obs = format_codegen_observation(raw, true, false, 1, 4);
        assert!(obs.contains("[sandbox_error]"), "{obs}");
        // Observational: lists available client methods (not "please use").
        assert!(
            obs.contains("dense") && obs.contains("lexical") && obs.contains("grep"),
            "{obs}"
        );
        assert!(
            obs.contains("client.方法名") || obs.contains("client."),
            "{obs}"
        );
        assert!(
            obs.contains("1/4") && obs.contains("print") && obs.contains("AttributeError"),
            "{obs}"
        );
        assert!(!obs.contains("{n_fail}") && !obs.contains("{n_max}"), "{obs}");
    }

    #[test]
    fn no_output_round_gets_feedback_note() {
        // C3: empty stdout + empty stderr + zero bridge calls → explicit note.
        let raw = "[block 0] stdout: \nstderr: \n";
        let obs = format_codegen_observation(raw, false, true, 0, 4);
        assert!(obs.contains("[no_output]"));
        assert!(
            obs.contains("stdout") && obs.contains("stderr") && obs.contains("client.*"),
            "{obs}"
        );
        // A round that produced output must NOT get the note.
        let obs = format_codegen_observation("[block 0] stdout: 42\nstderr: ", false, false, 0, 4);
        assert!(!obs.contains("[no_output]"));
        // Note composes with the sandbox-error hint.
        let obs = format_codegen_observation(raw, true, true, 2, 4);
        assert!(obs.contains("[no_output]") && obs.contains("[sandbox_error]"));
        assert!(obs.contains("2/4"), "{obs}");
    }

    #[test]
    fn codegen_sdk_method_names_are_detected_as_fake_native_tools() {
        assert!(is_codegen_sdk_method_as_native_tool("dense"));
        assert!(is_codegen_sdk_method_as_native_tool("web"));
        assert!(is_codegen_sdk_method_as_native_tool("doc_scan"));
        assert!(is_codegen_sdk_method_as_native_tool("doc_chunks"));
        assert!(!is_codegen_sdk_method_as_native_tool("dense_retrieval"));
        assert!(!is_codegen_sdk_method_as_native_tool("web_search"));
        let r = reject_native_tool_surface("dense");
        assert_eq!(r.status, contracts::ToolStatus::Error);
        let hint = r.data.as_ref().unwrap()["hint"].as_str().unwrap();
        assert!(hint.contains("client.dense") || hint.contains("await client.dense"));
        assert!(hint.contains("<code"));
    }

    #[test]
    fn stderr_with_success_exit_code_is_not_an_error() {
        // A DeprecationWarning/pandas future warning on stderr must not be treated as a
        // sandbox error when the process exited cleanly (success=true, exit_code=0) and
        // the stderr carries no traceback.
        let exec = avrag_code_interpreter::ExecutionResult {
            stdout: "42\n".to_string(),
            stderr: "DeprecationWarning: invalid escape sequence\n".to_string(),
            result: Some("42".to_string()),
            success: true,
            exit_code: Some(0),
            killed: false,
        };
        assert!(
            !code_exec_is_error(&exec),
            "benign stderr warnings must not flip a clean run into an error"
        );
    }

    #[test]
    fn nonzero_exit_code_is_an_error_even_with_stderr() {
        let exec = avrag_code_interpreter::ExecutionResult {
            stdout: String::new(),
            stderr: "AttributeError: no attribute 'hybrid_search'\n".to_string(),
            result: None,
            success: false,
            exit_code: Some(1),
            killed: false,
        };
        assert!(code_exec_is_error(&exec));
    }

    #[test]
    fn stderr_traceback_with_clean_exit_is_an_error() {
        // The sandbox swallows raised exceptions (always success=true/exit_code=0) and
        // surfaces them ONLY via a "Traceback" on stderr. This must still count as an
        // error so the consecutive-sandbox-error break-to-synthesis net stays effective.
        let exec = avrag_code_interpreter::ExecutionResult {
            stdout: String::new(),
            stderr: "Traceback (most recent call last):\n  File \"<sandbox>\", line 1\nRuntimeError: fail\n".to_string(),
            result: None,
            success: true,
            exit_code: Some(0),
            killed: false,
        };
        assert!(code_exec_is_error(&exec));
    }

    #[test]
    fn long_observation_is_truncated() {
        // Over the 8000-char budget: the injected message must mark it as truncated.
        let raw = "[block 0] stdout: ".to_string() + &"x".repeat(20_000) + &"\nstderr: \n";
        let msg = format_codegen_result_message(&raw);
        assert!(
            msg.contains("[truncated"),
            "expected a truncation marker in the injected message"
        );
        // The full 20k-char payload must not survive intact.
        let payload = "x".repeat(20_000);
        assert!(!msg.contains(&payload));
    }

    #[test]
    fn short_observation_not_truncated() {
        let raw = "[block 0] stdout: small result\nstderr: \n";
        let msg = format_codegen_result_message(raw);
        assert!(!msg.contains("[truncated"));
        assert!(msg.contains("small result"));
    }

    #[test]
    fn injected_message_has_untrusted_marker() {
        let raw = "[block 0] stdout: ok\nstderr: \n";
        let msg = format_codegen_result_message(raw);
        assert!(
            msg.contains("untrusted=\"true\""),
            "expected the untrusted attribute on the opening tag"
        );
        assert!(
            msg.contains("不可信") || msg.contains("untrusted"),
            "expected untrusted-content observation text"
        );
    }

    #[test]
    fn injected_message_keeps_code_execution_result_tags() {
        // The outer tags must remain so exit_policy parsing still matches the block.
        let raw = "[block 0] stdout: ok\nstderr: \n";
        let msg = format_codegen_result_message(raw);
        assert!(msg.contains("<code_execution_result"));
        assert!(msg.contains("</code_execution_result>"));
    }

    fn test_iteration_state() -> IterationState {
        IterationState {
            messages: vec![],
            disclosed: crate::react_loop::assembler::DisclosedState::default(),
            tool_results: vec![],
            total_tool_calls: 0,
            consecutive_sandbox_errors: 0,
            reasoning_acc: String::new(),
            answer_deltas_streamed: false,
            compile_continuations: 0,
            retrieval_aliases: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            evidence: crate::react_loop::evidence_pool::EvidencePool::new(),
            knockout: crate::helpers::shared_knockout(),
            ews: crate::helpers::EwsState::new(),
            session_fs: std::sync::Arc::new(crate::react_loop::session_fs::SessionFs::new()),
            sdk_allowed: std::sync::Arc::new(std::collections::HashSet::new()),
            query_card: None,
            max_iterations: 100,
        }
    }

    fn ok_bridge_result() -> ToolResult {
        ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1.0".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!([{"chunk_id": "c1", "text": "alpha beta"}])),
            trace: None,
        }
    }

    #[test]
    fn errored_round_still_preserves_captured_bridge_evidence() {
        // C2: bridge calls that succeeded before the block errored must extend
        // state.tool_results even though the round is counted as an error.
        let mut state = test_iteration_state();
        state.consecutive_sandbox_errors = 1;
        ReActLoop::record_bridge_evidence(&mut state, "", vec![ok_bridge_result()]);
        assert_eq!(state.tool_results.len(), 1);
        assert_eq!(state.tool_results[0].tool, "dense_retrieval");
        // The error path must NOT reset the consecutive-error counter (that
        // reset stays exclusive to record_codegen_success).
        assert_eq!(state.consecutive_sandbox_errors, 1);
    }

    #[test]
    fn stdout_fallback_parse_used_when_bridge_captured_nothing() {
        // Shared helper keeps the pre-C2 fallback: no bridge results → parse
        // chunk JSON out of the combined stdout observation.
        let mut state = test_iteration_state();
        let combined = "[block 0] stdout: [{\"chunk_id\": \"c9\", \"text\": \"alpha\"}]\nstderr: ";
        // Only assert the call path exists and does not panic; the fallback
        // parser decides whether this shape yields a result.
        ReActLoop::record_bridge_evidence(&mut state, combined, Vec::new());
        let _ = state.tool_results.len();
    }

    // ---- E3: lexical 0-hit coaching hints -----------------------------------

    fn bridge_call(
        method: &str,
        query: &str,
        status: contracts::ToolStatus,
        data: serde_json::Value,
    ) -> super::super::deps::BridgeCallObs {
        super::super::deps::BridgeCallObs {
            method: method.to_string(),
            query: Some(query.to_string()),
            result: ToolResult {
                tool: "lexical_retrieval".to_string(),
                version: "1.0".to_string(),
                status,
                data: Some(data),
                trace: None,
            },
        }
    }

    // ---- K1: retrieval summary + adaptive-k hints ---------------------------

    #[test]
    fn markitdown_hints_fire_on_no_space_pipe_and_kv_form() {
        let bad_pipe = markitdown_format_hints(&[r#"client.grep("|概念阶段|")"#.to_string()]);
        assert!(bad_pipe.contains("管道后无空格"), "{bad_pipe}");
        let good_pipe = markitdown_format_hints(&[r#"client.grep("| 概念阶段 |")"#.to_string()]);
        assert!(good_pipe.is_empty(), "{good_pipe}");
        let escaped = markitdown_format_hints(&[
            r#"client.grep(r"\|\s*概念阶段\s*\|", regex=True)"#.to_string(),
        ]);
        assert!(
            escaped.is_empty(),
            "regex escape form must not fire: {escaped}"
        );
        let kv = markitdown_format_hints(&[r#"client.grep("阶段=概念阶段")"#.to_string()]);
        assert!(kv.contains("key=value"), "{kv}");
        let ascii = markitdown_format_hints(&["x = foo|bar".to_string()]);
        assert!(ascii.is_empty(), "{ascii}");
    }

    #[test]
    fn retrieval_callouts_render_summary_and_hints() {
        let calls = vec![
            bridge_call(
                "dense",
                "q1",
                contracts::ToolStatus::Ok,
                serde_json::json!({
                    "chunks": [
                        {"chunk_id": "c1", "alias": "#1"},
                        {"chunk_id": "c2", "alias": "#2"}
                    ],
                    "retrieval_hint": "命中明确（top 分数梯度大）。可进入分析；若需交叉验证可换角度再查一次。",
                }),
            ),
            bridge_call(
                "lexical",
                "q2",
                contracts::ToolStatus::Ok,
                serde_json::json!({
                    "chunks": [{"chunk_id": "c3", "alias": "#3"}],
                    "retrieval_hint": "结果区分度低（分数平均）。建议换更具体的词。",
                }),
            ),
            bridge_call(
                "grep",
                "p",
                contracts::ToolStatus::Ok,
                serde_json::json!({
                    "total_hits": 0,
                    "truncated": false,
                    "hits": []
                }),
            ),
        ];
        let seen: std::sync::Mutex<std::collections::HashSet<String>> =
            std::sync::Mutex::new(std::collections::HashSet::new());
        let out = retrieval_callouts(&calls, &seen);
        assert!(out.contains("本轮检索 3 次，共返回 3 条"), "{out}");
        assert!(out.contains("可见 alias: #1, #2, #3"), "{out}");
        assert!(out.contains("3 个为本轮新增、0 个为历史已见"), "{out}");
        assert!(out.contains("grep total_hits=0"), "{out}");
        assert!(out.contains("SELECTED 仅能引用已出现的 alias"), "{out}");
        assert!(out.contains("命中明确"), "{out}");
        assert!(out.contains("区分度低"), "{out}");
        // A second round returning the same aliases must report saturation.
        let out2 = retrieval_callouts(&calls, &seen);
        assert!(out2.contains("0 个为本轮新增、3 个为历史已见"), "{out2}");
    }

    #[test]
    fn retrieval_callouts_empty_without_retrieval() {
        let seen: std::sync::Mutex<std::collections::HashSet<String>> =
            std::sync::Mutex::new(std::collections::HashSet::new());
        assert!(retrieval_callouts(&[], &seen).is_empty());
        // A non-retrieval method (doc_summary) produces no summary line.
        let calls = vec![bridge_call(
            "doc_summary",
            "",
            contracts::ToolStatus::Ok,
            serde_json::json!({"chunks": [{"chunk_id": "c1"}]}),
        )];
        assert!(retrieval_callouts(&calls, &seen).is_empty());
    }

    #[test]
    fn evidence_pool_intake_accumulates_claim_notes_from_expanded() {
        let calls = vec![bridge_call(
            "dense",
            "q",
            contracts::ToolStatus::Ok,
            serde_json::json!({
                "chunks": [{
                    "chunk_id": "c1",
                    "alias": "#1",
                    "visibility": "expanded",
                    "text": "tok_expanded marker_beta round_fact"
                }],
                "visibility_expanded_n": 1,
            }),
        )];
        let mut pool = crate::react_loop::evidence_pool::EvidencePool::new();
        pool.intake_from_bridge_calls(&calls);
        assert_eq!(pool.claim_notes.len(), 1);
        assert_eq!(pool.claim_notes[0].alias, "#1");
        assert!(pool.claim_notes[0].excerpt.contains("marker_beta"));
        // Format path stays pure (no claim mutation).
        let out = retrieval_callouts(&calls, &pool.seen_retrieval_aliases);
        assert!(!out.is_empty());
        assert_eq!(pool.claim_notes.len(), 1);
    }

    #[test]
    fn retrieval_summary_detail_notes_truncation() {
        let d = build_retrieval_summary_detail(&["#1".into()], true, false, 1, 0);
        assert!(d.contains("truncated=true"), "{d}");
        assert!(d.contains("#1"), "{d}");
    }
}
