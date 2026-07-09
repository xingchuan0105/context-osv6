//! Tool-call handlers for the WriteRefine ReAct loop.
//!
//! Each handler is an `impl<'a> WriteRefineLoopRunner<'a>` method dispatched
//! from `dispatch_tool_call`.

use std::time::Duration;

use contracts::chat::ToolStatus;
use contracts::{ToolCall, ToolResult};
use heavytail::feedforward::fingerprint_workspace;
use heavytail::lexical_apply::{self, LexicalApplyResult};
use heavytail::patch::{self, AllowSet, Patch};
use heavytail::score::composite;
use heavytail::skeleton::MaterialCard;
use heavytail::state::{RoundRecord, WriterState};

use crate::agents::events::{AgentEvent, AgentEventSink};

use super::helpers;
use super::WriteRefineLoopRunner;
use super::types::RefineContext;

impl<'a> WriteRefineLoopRunner<'a> {
    /// Dispatch a single `ToolCall`, intercepting the 3 write_refine ids.
    pub(super) async fn dispatch_tool_call(
        &self,
        call: &ToolCall,
        ctx: &mut RefineContext,
        reservoir: &[String],
        sink: &dyn AgentEventSink,
        state: &mut WriterState,
    ) -> ToolResult {
        match call.tool.as_str() {
            "write_refine_revise" => self.handle_revise(call, ctx, reservoir, state).await,
            "write_refine_lexical" => self.handle_lexical(call, ctx, reservoir, state).await,
            "write_refine_research" => self.handle_research(call, ctx, sink).await,
            "write_refine_finish" => self.handle_finish(call, ctx).await,
            other => ToolResult {
                tool: other.to_string(),
                version: "1".to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({
                    "error": format!("unknown tool in write_refine loop: {other}")
                })),
                trace: None,
            },
        }
    }

    /// `write_refine_revise` — apply sentence-level patches (plan §4.2).
    pub(super) async fn handle_revise(
        &self,
        call: &ToolCall,
        ctx: &mut RefineContext,
        reservoir: &[String],
        state: &mut WriterState,
    ) -> ToolResult {
        let patches = match call.args.get("patches").and_then(|v| v.as_array()) {
            Some(arr) => arr,
            None => {
                return helpers::tool_error(
                    "write_refine_revise",
                    "missing 'patches' array",
                );
            }
        };
        if patches.is_empty() {
            return helpers::tool_error("write_refine_revise", "patches array is empty");
        }
        if patches.len() > 12 {
            return helpers::tool_error("write_refine_revise", "patches array exceeds 12 items");
        }

        // Build a raw patch text `s<id>| <text>` per line and reuse `parse_patch`.
        let mut raw_lines = Vec::new();
        for p in patches {
            let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let text = p.get("text").and_then(|v| v.as_str()).unwrap_or("");
            if id.is_empty() || text.is_empty() {
                return helpers::tool_error(
                    "write_refine_revise",
                    &format!("patch missing id or text: {p}"),
                );
            }
            raw_lines.push(format!("{id}| {text}"));
        }
        let raw = raw_lines.join("\n");

        let allow = AllowSet::all_live(&ctx.workspace);
        let patch: Patch = match patch::parse_patch(&raw, &allow) {
            Ok(p) => p,
            Err(e) => {
                return ToolResult {
                    tool: "write_refine_revise".to_string(),
                    version: "1".to_string(),
                    status: ToolStatus::Error,
                    data: Some(serde_json::json!({
                        "error": format!("patch parse failed: {e:?}"),
                        "applied": [],
                    })),
                    trace: None,
                };
            }
        };

        let changed = patch::apply_patch(&mut ctx.workspace, &patch, &allow);

        // Recompute diagnosis + bands (always, so the observation reflects the
        // post-attempt state even when nothing changed).
        ctx.recompute(&self.style, reservoir);

        // Only an *effective* revise (≥1 sentence actually changed) counts as a
        // round and is eligible for best-version tracking (plan §3.3, §4.4). A
        // patch that parsed but applied to nothing is a free retry.
        if !changed.is_empty() {
            ctx.revise_rounds_used += 1;

            // Update the best-version snapshot when composite S improved.
            let new_score = ctx.diagnosis.score_s;
            let prev_best = ctx.best_snapshot.as_ref().map(|b| b.score).unwrap_or(f64::NEG_INFINITY);
            if new_score > prev_best {
                ctx.best_snapshot = Some(super::types::BestSnapshot {
                    score: new_score,
                    workspace: ctx.workspace.clone(),
                });
            }

            // Keep WriterState bookkeeping consistent: sync the workspace so
            // record_round captures the correct canonical, then append the round.
            state.workspace = ctx.workspace.clone();
            let fp = fingerprint_workspace(&ctx.workspace);
            let score = composite(&fp, &self.style);
            state.record_round(RoundRecord {
                fingerprint: fp,
                directives_json: String::new(),
                patch_raw: raw.clone(),
                compliance: Vec::new(),
                score,
            });
        }

        let diag = &ctx.diagnosis;
        ToolResult {
            tool: "write_refine_revise".to_string(),
            version: "1".to_string(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({
                "applied": changed.iter().map(|id| id.0.clone()).collect::<Vec<_>>(),
                "diagnosis_delta": {
                    "score_s": diag.score_s,
                    "bands_passed": ctx.bands_satisfied,
                    "metric_results": diag.validation.metric_results.iter().map(|m| {
                        serde_json::json!({
                            "metric": m.metric,
                            "actual": m.actual,
                            "target": [m.target.0, m.target.1],
                            "passed": m.passed,
                        })
                    }).collect::<Vec<_>>(),
                },
                "revise_rounds_used": ctx.revise_rounds_used,
            })),
            trace: None,
        }
    }

    /// `write_refine_lexical` — deterministic repeat/replace term edits.
    pub(super) async fn handle_lexical(
        &self,
        call: &ToolCall,
        ctx: &mut RefineContext,
        reservoir: &[String],
        state: &mut WriterState,
    ) -> ToolResult {
        let op = match call.args.get("op").and_then(|v| v.as_str()) {
            Some("replace_term") | Some("repeat_term") => call.args.get("op").and_then(|v| v.as_str()).unwrap(),
            Some(other) => {
                return helpers::tool_error(
                    "write_refine_lexical",
                    &format!("invalid op '{other}' (expected replace_term or repeat_term)"),
                );
            }
            None => return helpers::tool_error("write_refine_lexical", "missing 'op' field"),
        };
        let sentence_ids = helpers::parse_sentence_id_args(call.args.get("sentence_ids"));

        let apply_result: LexicalApplyResult = match op {
            "replace_term" => {
                let from = match call.args.get("from").and_then(|v| v.as_str()) {
                    Some(s) if !s.is_empty() => s,
                    _ => return helpers::tool_error("write_refine_lexical", "replace_term requires non-empty 'from'"),
                };
                let to = match call.args.get("to").and_then(|v| v.as_str()) {
                    Some(s) if !s.is_empty() => s,
                    _ => return helpers::tool_error("write_refine_lexical", "replace_term requires non-empty 'to'"),
                };
                let max = call
                    .args
                    .get("max_replacements")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(5)
                    .min(12) as usize;
                lexical_apply::apply_replace_term(
                    &mut ctx.workspace,
                    from,
                    to,
                    &sentence_ids,
                    max,
                )
            }
            "repeat_term" => {
                let term = match call.args.get("term").and_then(|v| v.as_str()) {
                    Some(s) if s.chars().count() >= 2 => s,
                    _ => return helpers::tool_error("write_refine_lexical", "repeat_term requires 'term' (≥2 chars)"),
                };
                let max = call
                    .args
                    .get("max_edits")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(5)
                    .min(12) as usize;
                lexical_apply::apply_repeat_term(&mut ctx.workspace, term, &sentence_ids, max)
            }
            _ => unreachable!(),
        };

        if !apply_result.errors.is_empty() && apply_result.edits.is_empty() {
            return ToolResult {
                tool: "write_refine_lexical".to_string(),
                version: "1".to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({
                    "error": apply_result.errors.join("; "),
                    "edits": [],
                })),
                trace: None,
            };
        }

        ctx.recompute(&self.style, reservoir);
        if !apply_result.edits.is_empty() {
            ctx.revise_rounds_used += 1;
            let new_score = ctx.diagnosis.score_s;
            let prev_best = ctx
                .best_snapshot
                .as_ref()
                .map(|b| b.score)
                .unwrap_or(f64::NEG_INFINITY);
            if new_score > prev_best {
                ctx.best_snapshot = Some(super::types::BestSnapshot {
                    score: new_score,
                    workspace: ctx.workspace.clone(),
                });
            }
            state.workspace = ctx.workspace.clone();
            let fp = fingerprint_workspace(&ctx.workspace);
            let score = composite(&fp, &self.style);
            let patch_raw = apply_result
                .edits
                .iter()
                .map(|e| format!("{}| {}", e.id, e.after))
                .collect::<Vec<_>>()
                .join("\n");
            state.record_round(RoundRecord {
                fingerprint: fp,
                directives_json: format!(r#"{{"lexical_op":"{op}"}}"#),
                patch_raw,
                compliance: Vec::new(),
                score,
            });
        }

        let diag = &ctx.diagnosis;
        ToolResult {
            tool: "write_refine_lexical".to_string(),
            version: "1".to_string(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({
                "edits": apply_result.edits.iter().map(|e| {
                    serde_json::json!({
                        "id": e.id,
                        "before": e.before,
                        "after": e.after,
                    })
                }).collect::<Vec<_>>(),
                "errors": apply_result.errors,
                "reservoir_terms": reservoir,
                "diagnosis_delta": {
                    "score_s": diag.score_s,
                    "bands_passed": ctx.bands_satisfied,
                    "metric_results": diag.validation.metric_results.iter().map(|m| {
                        serde_json::json!({
                            "metric": m.metric,
                            "actual": m.actual,
                            "target": [m.target.0, m.target.1],
                            "passed": m.passed,
                        })
                    }).collect::<Vec<_>>(),
                },
                "revise_rounds_used": ctx.revise_rounds_used,
            })),
            trace: None,
        }
    }

    /// `write_refine_research` — on-demand RAG/Web sub-worker (plan §4.3).
    pub(super) async fn handle_research(
        &self,
        call: &ToolCall,
        ctx: &mut RefineContext,
        sink: &dyn AgentEventSink,
    ) -> ToolResult {
        if self.budget.research_capped()
            && ctx.research_calls_used >= self.budget.max_on_demand_research
        {
            return ToolResult {
                tool: "write_refine_research".to_string(),
                version: "1".to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!({
                    "budget_exhausted": true,
                    "research_calls_used": ctx.research_calls_used,
                    "new_cards": [],
                    "terms": [],
                })),
                trace: None,
            };
        }

        let kind = match call.args.get("kind").and_then(|v| v.as_str()) {
            Some("rag") => crate::agents::AgentKind::Rag,
            Some("web") => crate::agents::AgentKind::Search,
            Some(k) => {
                return helpers::tool_error(
                    "write_refine_research",
                    &format!("invalid kind: {k} (expected 'rag' or 'web')"),
                );
            }
            None => {
                return helpers::tool_error("write_refine_research", "missing 'kind' field");
            }
        };
        let query = match call.args.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.is_empty() => q.to_string(),
            _ => return helpers::tool_error("write_refine_research", "missing or empty 'query'"),
        };
        let reason = call
            .args
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Emit refine_research activity.
        let _ = sink
            .emit(AgentEvent::Activity {
                stage: "refine_research".to_string(),
                message: format!("Researching ({kind:?}): {query}"),
            })
            .await;

        // Build a sub-worker request with reduced budgets.
        let mut worker_req =
            crate::writer::invoker::SubagentInvoker::worker_request(self.parent_request, kind, &query);
        worker_req.max_iterations = Some(2); // plan §4.3: sub-worker max_iterations=2
        worker_req.query = query.clone();

        let result = match self
            .invoker
            .run_worker(
                worker_req,
                self.budget.per_research_worker_tokens,
                Duration::from_secs(60),
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                return helpers::tool_error(
                    "write_refine_research",
                    &format!("research worker failed: {e}"),
                );
            }
        };

        // Extract material cards from the worker result.
        let guard = self.parent_request.guard_pipeline.as_deref();
        let trace_id = self.parent_request.session_id.as_deref();
        let extraction = crate::writer::cards::extract_material_cards(
            &result,
            kind,
            guard,
            trace_id,
        );
        let new_cards = extraction.cards;
        ctx.research_calls_used += 1;

        // Merge new cards into the material pack and capture exactly which
        // views this call inserted (deduped). The observation reports only
        // those, so the agent can tell whether the search produced new info.
        let workspace_text = ctx.workspace.render_plain();
        let inserted = ctx
            .material_pack
            .merge_new_cards(new_cards.clone(), &workspace_text, 3);

        // Build a compressed observation: the inserted card views + terms.
        let new_views: Vec<serde_json::Value> = inserted
            .iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "kind": c.kind,
                    "content": c.content,
                    "source_label": c.source_label,
                    "rare_terms": c.rare_terms,
                })
            })
            .collect();
        let terms: Vec<String> = new_cards
            .iter()
            .flat_map(|c: &MaterialCard| c.rare_terms.clone())
            .take(20)
            .collect();

        ToolResult {
            tool: "write_refine_research".to_string(),
            version: "1".to_string(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({
                "new_cards": new_views,
                "terms": terms,
                "research_calls_used": ctx.research_calls_used,
                "budget_exhausted": false,
                "reason": reason,
            })),
            trace: None,
        }
    }

    /// `write_refine_finish` — soft finish (plan §4.4).
    pub(super) async fn handle_finish(&self, call: &ToolCall, ctx: &mut RefineContext) -> ToolResult {
        if self.budget.enforce_core_band_finish_gate && !ctx.bands_satisfied {
            let pending: Vec<String> = ctx
                .diagnosis
                .validation
                .metric_results
                .iter()
                .filter(|m| {
                    (m.metric == "hapax_ratio" || m.metric == "zipf_exponent") && !m.passed
                })
                .map(|m| m.metric.clone())
                .collect();
            if !pending.is_empty() {
                return helpers::tool_error(
                    "write_refine_finish",
                    &format!(
                        "核心指标仍未过关：{}。请继续 write_refine_revise 或 write_refine_lexical。",
                        pending.join(", ")
                    ),
                );
            }
        }

        let reason = call
            .args
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("(no reason given)")
            .to_string();
        let bands_satisfied = call
            .args
            .get("bands_satisfied")
            .and_then(|v| v.as_bool())
            .unwrap_or(ctx.bands_satisfied);

        // finish is a soft exit: we just record telemetry and let the loop break.
        ToolResult {
            tool: "write_refine_finish".to_string(),
            version: "1".to_string(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({
                "finish_reason": reason,
                "bands_satisfied": bands_satisfied,
                "validation_warning": !ctx.bands_satisfied,
            })),
            trace: None,
        }
    }
}
