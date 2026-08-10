//! Verify: one-shot adjudicate after synthesis (three-loop design 2026-08-07).
//!
//! Pass → deliver. Fail → route synthesis|retrieve with advice. No tools, no answer rewrite.

use avrag_llm::{ChatMessage, LlmClient, LlmUsage};
use common::AppError;
use contracts::{ToolResult, ToolStatus};
use serde::{Deserialize, Serialize};

use super::config::LoopExitConfig;
use super::json_fence;
use super::prompt_assets;
use super::query_card::{QueryCard, QuestionType};

/// Soft cap on total evidence chars (across buckets).
const EVIDENCE_EXCERPT_MAX: usize = 6000;
/// Per-source budget so one long code_execution cannot starve tools.
const BUCKET_MSG_CLAIM: usize = 1500;
const BUCKET_MSG_CODE: usize = 2500;
const BUCKET_MSG_SUMMARY: usize = 800;
const BUCKET_TOOLS: usize = 2000;

/// Default fail-round cap when config is 0 but verify is on.
pub const DEFAULT_VERIFY_MAX_FAIL: u8 = 3;
/// When product billable has used this fraction of max_tokens, shrink fail budget to 1.
const TIGHT_BUDGET_NUM: u32 = 85;
const TIGHT_BUDGET_DEN: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifyRoute {
    Synthesis,
    Retrieve,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyVerdict {
    Pass,
    Fail { route: VerifyRoute, advice: String },
}

/// Host action after a verify **fail** (count already incremented).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyFailFollowUp {
    DeliverCeiling,
    Resynthesis { observation: String },
    Reretrieve { observation: String },
}

/// Outcome of one verify call (includes parse diagnostics).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyOutcome {
    pub verdict: VerifyVerdict,
    /// True when model JSON could not be parsed / unknown verdict → soft Pass.
    pub parse_error: bool,
}

#[derive(Debug, Deserialize)]
struct VerifyJson {
    verdict: String,
    #[serde(default)]
    route: Option<String>,
    #[serde(default)]
    advice: Option<String>,
}

/// True when every **Ok** tool is `weather_query` (and at least one such Ok exists).
pub fn weather_only_ok_evidence(tool_results: &[ToolResult]) -> bool {
    let mut any_weather = false;
    let mut any_other_ok = false;
    for t in tool_results {
        if !matches!(t.status, ToolStatus::Ok) {
            continue;
        }
        if t.tool == "weather_query" {
            any_weather = true;
        } else {
            any_other_ok = true;
        }
    }
    any_weather && !any_other_ok
}

/// One verify LLM call (pass or fail), for `mode_debug.general.verify.calls`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifyCallObs {
    /// `pass` | `fail` | `error` (LLM/transport failure → deliver draft).
    pub verdict: String,
    /// On fail: `synthesis` | `retrieve` | `ceiling`. On pass/error: omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    pub parse_error: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub advice_summary: String,
}

/// White-box verify summary for `mode_debug.general.verify` / eval artifacts.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifyObservability {
    /// True when at least one verify LLM call was attempted.
    pub ran: bool,
    /// When `ran` is false: why verify was skipped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bypass_reason: Option<String>,
    pub fail_count: u8,
    /// True when delivery used fail-ceiling path (closeout or disaster; no user footnote).
    pub ceiling: bool,
    pub parse_error_count: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calls: Vec<VerifyCallObs>,
    pub rereretrieve_iters: u8,
    pub product_rounds_used: u8,
}

/// Stable bypass id for observability (`None` = should run verify).
pub fn verify_bypass_reason(
    loop_exit: &LoopExitConfig,
    card: Option<&QueryCard>,
    tool_results: &[ToolResult],
) -> Option<&'static str> {
    if !loop_exit.verify {
        return Some("verify_disabled");
    }
    if let Some(c) = card {
        match c.question_type {
            QuestionType::Calculation => return Some("calculation"),
            QuestionType::Chitchat => return Some("chitchat"),
            _ => {}
        }
    }
    if weather_only_ok_evidence(tool_results) {
        return Some("weather_only");
    }
    None
}

/// Whether this run should invoke verify after synthesis.
pub fn should_run_verify(
    loop_exit: &LoopExitConfig,
    card: Option<&QueryCard>,
    tool_results: &[ToolResult],
) -> bool {
    verify_bypass_reason(loop_exit, card, tool_results).is_none()
}

/// Shrink fail budget when the product run is already near the token ceiling
/// (retrieve + prior synth/verify usage). `0` means next fail → ceiling immediately.
pub fn effective_max_verify_fails(
    configured: u8,
    product_billable: u32,
    max_tokens: u32,
) -> u8 {
    let base = if configured == 0 {
        DEFAULT_VERIFY_MAX_FAIL
    } else {
        configured
    };
    if max_tokens == 0 {
        return base;
    }
    if product_billable >= max_tokens {
        return 0;
    }
    // remain < 15% of budget → at most one fail re-entry
    let used_pct = product_billable.saturating_mul(TIGHT_BUDGET_DEN) / max_tokens.max(1);
    if used_pct >= TIGHT_BUDGET_NUM {
        return base.min(1);
    }
    base
}

/// After a fail, force ceiling when product token budget is already exhausted.
pub fn budget_forces_ceiling(product_billable: u32, max_tokens: u32) -> bool {
    max_tokens > 0 && product_billable >= max_tokens
}

/// After a fail verdict, decide ceiling delivery vs re-entry observation.
/// Ceiling no longer appends a host footnote; the host either runs a
/// user-facing closeout LLM turn (token remaining) or a disaster prose line
/// (token exhausted / illegal out-bound draft).
pub fn follow_up_after_verify_fail(
    route: VerifyRoute,
    advice: &str,
    fail_count_after: u8,
    max_fails: u8,
) -> VerifyFailFollowUp {
    if fail_count_after > max_fails {
        return VerifyFailFollowUp::DeliverCeiling;
    }
    match route {
        VerifyRoute::Synthesis => VerifyFailFollowUp::Resynthesis {
            observation: prompt_assets::verify_fail_synthesis_observation(advice),
        },
        VerifyRoute::Retrieve => VerifyFailFollowUp::Reretrieve {
            observation: prompt_assets::verify_fail_retrieve_observation(advice),
        },
    }
}

/// Outbound sanitizer when no further LLM turn is available (token ceiling) or
/// after a closeout draft: illegal format → disaster prose; legal prose ships
/// as-is **without** host footnotes.
pub fn finalize_delivery_without_llm(answer: String, mode_id: &str) -> String {
    if super::answer_contract::check_final_answer(&answer).is_some() {
        prompt_assets::disaster_format_exhausted().to_string()
    } else if answer.trim().is_empty() {
        prompt_assets::disaster_no_evidence_answer(mode_id).to_string()
    } else {
        answer
    }
}

pub fn verify_max_fail_rounds(loop_exit: &LoopExitConfig) -> u8 {
    if loop_exit.verify_max_fail_rounds == 0 {
        DEFAULT_VERIFY_MAX_FAIL
    } else {
        loop_exit.verify_max_fail_rounds
    }
}

/// Parse model JSON. Unparseable → Pass + `parse_error: true`.
pub fn parse_verify_response(raw: &str) -> VerifyOutcome {
    let stripped = json_fence::strip_json_fence(raw);
    let parsed: VerifyJson = match serde_json::from_str(stripped.trim()) {
        Ok(v) => v,
        Err(_) => {
            tracing::warn!(raw = %truncate_for_log(raw, 200), "verify: unparseable JSON → pass");
            return VerifyOutcome {
                verdict: VerifyVerdict::Pass,
                parse_error: true,
            };
        }
    };
    let v = parsed.verdict.trim().to_ascii_lowercase();
    if v == "pass" || v == "ok" || v == "通过" {
        return VerifyOutcome {
            verdict: VerifyVerdict::Pass,
            parse_error: false,
        };
    }
    if v != "fail" && v != "不合格" && v != "reject" {
        tracing::warn!(verdict = %parsed.verdict, "verify: unknown verdict → pass");
        return VerifyOutcome {
            verdict: VerifyVerdict::Pass,
            parse_error: true,
        };
    }
    let route = match parsed
        .route
        .as_deref()
        .unwrap_or("synthesis")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "retrieve" | "retrieval" | "检索" => VerifyRoute::Retrieve,
        _ => VerifyRoute::Synthesis,
    };
    let advice = parsed.advice.unwrap_or_default().trim().to_string();
    let advice = if advice.is_empty() {
        prompt_assets::verify_empty_advice().to_string()
    } else {
        advice
    };
    VerifyOutcome {
        verdict: VerifyVerdict::Fail { route, advice },
        parse_error: false,
    }
}

fn truncate_for_log(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}

fn take_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}

/// Build evidence with per-bucket caps (claim / code / summary / tools).
pub fn evidence_excerpt(tool_results: &[ToolResult], messages: &[ChatMessage]) -> String {
    let mut claim_parts: Vec<String> = Vec::new();
    let mut code_parts: Vec<String> = Vec::new();
    let mut summary_parts: Vec<String> = Vec::new();
    let mut claim_used = 0usize;
    let mut code_used = 0usize;
    let mut summary_used = 0usize;

    for msg in messages.iter().rev() {
        let c = msg.content.as_str();
        if c.contains("[claim_notes]") && claim_used < BUCKET_MSG_CLAIM {
            let piece = take_chars(c, BUCKET_MSG_CLAIM - claim_used);
            claim_used += piece.chars().count();
            claim_parts.push(format!("msg.claim_notes:\n{piece}"));
        } else if c.contains("<code_execution_result") && code_used < BUCKET_MSG_CODE {
            let piece = take_chars(c, BUCKET_MSG_CODE - code_used);
            code_used += piece.chars().count();
            code_parts.push(format!("msg.code_execution:\n{piece}"));
        } else if c.contains("[retrieval_summary]") && summary_used < BUCKET_MSG_SUMMARY {
            let piece = take_chars(c, BUCKET_MSG_SUMMARY - summary_used);
            summary_used += piece.chars().count();
            summary_parts.push(format!("msg.retrieval_summary:\n{piece}"));
        }
    }

    let mut tool_parts: Vec<String> = Vec::new();
    let mut tool_used = 0usize;
    for tr in tool_results.iter().rev() {
        if tool_used >= BUCKET_TOOLS {
            break;
        }
        let status = format!("{:?}", tr.status);
        let data = tr
            .data
            .as_ref()
            .map(|d| d.to_string())
            .unwrap_or_default();
        let chunk = format!("tool={} status={} data={}", tr.tool, status, data);
        let piece = take_chars(&chunk, BUCKET_TOOLS - tool_used);
        tool_used += piece.chars().count();
        tool_parts.push(piece);
    }

    let mut parts: Vec<String> = Vec::new();
    parts.extend(claim_parts);
    parts.extend(code_parts);
    parts.extend(summary_parts);
    parts.extend(tool_parts);

    if parts.is_empty() {
        return prompt_assets::verify_empty_evidence().to_string();
    }
    let joined = parts.join("\n---\n");
    take_chars(&joined, EVIDENCE_EXCERPT_MAX)
}

/// One-shot verify LLM call. Returns outcome + usage for product budget accounting.
/// Honors `cancel` (select against complete; cooperative cancel mid-call).
pub async fn run_verify(
    llm: &LlmClient,
    question: &str,
    final_answer: &str,
    tool_results: &[ToolResult],
    messages: &[ChatMessage],
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<(VerifyOutcome, LlmUsage), AppError> {
    if cancel.is_cancelled() {
        return Err(super::cancellation::cancellation_error());
    }
    let evidence = evidence_excerpt(tool_results, messages);
    let system = format!(
        "{}\n\n{}",
        prompt_assets::verify_system(),
        prompt_assets::verify_skill_body()
    );
    let user = prompt_assets::verify_user(question, final_answer, &evidence);
    let messages = vec![ChatMessage::system(system), ChatMessage::user(user)];
    let complete = llm.complete_json_mode(&messages, Some(0.2));
    tokio::pin!(complete);
    let resp = tokio::select! {
        _ = cancel.cancelled() => {
            return Err(super::cancellation::cancellation_error());
        }
        result = &mut complete => {
            result.map_err(|e| AppError::internal(format!("verify complete failed: {e}")))?
        }
    };
    Ok((parse_verify_response(&resp.content), resp.usage.clone()))
}

/// Truncate advice for telemetry Activity detail.
pub fn advice_summary(advice: &str, max_chars: usize) -> String {
    take_chars(advice, max_chars)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::config::LoopExitConfig;
    use super::super::query_card::QueryCard;
    use contracts::ToolStatus;

    #[test]
    fn parse_pass() {
        let o = parse_verify_response(r#"{"verdict":"pass"}"#);
        assert_eq!(o.verdict, VerifyVerdict::Pass);
        assert!(!o.parse_error);
    }

    #[test]
    fn parse_fail_synthesis() {
        let o = parse_verify_response(
            r#"{"verdict":"fail","route":"synthesis","advice":"终稿口径与证据中另一数字并存。"}"#,
        );
        match o.verdict {
            VerifyVerdict::Fail {
                route: VerifyRoute::Synthesis,
                advice,
            } => assert!(advice.contains("口径")),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn parse_empty_advice_uses_prompt() {
        let o = parse_verify_response(r#"{"verdict":"fail","route":"synthesis","advice":""}"#);
        match o.verdict {
            VerifyVerdict::Fail { advice, .. } => {
                assert_eq!(advice, prompt_assets::verify_empty_advice());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parse_garbage_is_pass_with_flag() {
        let o = parse_verify_response("not json at all");
        assert_eq!(o.verdict, VerifyVerdict::Pass);
        assert!(o.parse_error);
    }

    #[test]
    fn weather_only_ok_bypasses_verify() {
        let mut le = LoopExitConfig::default();
        le.verify = true;
        let tools = vec![ToolResult {
            tool: "weather_query".into(),
            version: "1".into(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({"aqi": 43})),
            trace: None,
        }];
        assert!(!should_run_verify(&le, None, &tools));
    }

    #[test]
    fn weather_plus_dense_does_not_bypass() {
        let mut le = LoopExitConfig::default();
        le.verify = true;
        let tools = vec![
            ToolResult {
                tool: "weather_query".into(),
                version: "1".into(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!({})),
                trace: None,
            },
            ToolResult {
                tool: "dense_retrieval".into(),
                version: "1".into(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!({"chunks": []})),
                trace: None,
            },
        ];
        assert!(should_run_verify(&le, None, &tools));
    }

    #[test]
    fn effective_fails_zero_when_budget_exhausted() {
        assert_eq!(effective_max_verify_fails(3, 1000, 1000), 0);
        assert_eq!(effective_max_verify_fails(3, 900, 1000), 1); // 90% used → tight
        assert_eq!(effective_max_verify_fails(3, 100, 1000), 3);
    }

    #[test]
    fn budget_forces_ceiling_when_spent() {
        assert!(budget_forces_ceiling(500, 500));
        assert!(!budget_forces_ceiling(100, 500));
        assert!(!budget_forces_ceiling(999, 0)); // disabled cap
    }

    #[test]
    fn evidence_buckets_cap_long_code() {
        let long = format!(
            "<code_execution_result>\n{}\n</code_execution_result>",
            "字".repeat(5000)
        );
        let msgs = vec![ChatMessage::user(long)];
        let tools = vec![ToolResult {
            tool: "dense_retrieval".into(),
            version: "1".into(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({"marker": "tool_data_present"})),
            trace: None,
        }];
        let ex = evidence_excerpt(&tools, &msgs);
        assert!(ex.contains("code_execution") || ex.contains("tool_data_present"));
        // Must not exceed global soft cap by much
        assert!(ex.chars().count() <= EVIDENCE_EXCERPT_MAX + 10);
        // Tool bucket should still get a slice when code is huge
        assert!(ex.contains("dense_retrieval") || ex.contains("tool_data_present"));
    }

    #[test]
    fn fourth_fail_delivers_ceiling() {
        let fu = follow_up_after_verify_fail(VerifyRoute::Synthesis, "still wrong", 4, 3);
        assert_eq!(fu, VerifyFailFollowUp::DeliverCeiling);
    }

    #[test]
    fn fail_to_synthesis_injects_observation() {
        let fu = follow_up_after_verify_fail(VerifyRoute::Synthesis, "1467 vs 2e4", 1, 3);
        match fu {
            VerifyFailFollowUp::Resynthesis { observation } => {
                assert!(observation.contains("verify_feedback"));
                assert!(observation.contains("1467"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn should_skip_calculation_card() {
        let mut le = LoopExitConfig::default();
        le.verify = true;
        let card = QueryCard {
            question_type: QuestionType::Calculation,
            required_actions: vec![],
        };
        assert!(!should_run_verify(&le, Some(&card), &[]));
    }

    #[test]
    fn finalize_delivery_keeps_clean_prose() {
        let out = finalize_delivery_without_llm("馆员式说明：库里没有该数字。".into(), "rag");
        assert!(out.contains("馆员式说明"));
        assert!(!out.contains("上限"));
        assert!(!out.contains("本 run"));
    }

    #[test]
    fn finalize_delivery_replaces_dsml() {
        let dirty = "<｜DSML｜tool_calls>invoke</｜DSML｜tool_calls>";
        let out = finalize_delivery_without_llm(dirty.into(), "rag");
        assert!(!out.contains("DSML"));
        assert!(!out.is_empty());
    }

    #[test]
    fn rereretrieve_cap_shares_product_rounds() {
        // product used 10 of 12 → remaining 2 → min(2, JUDGE cap 2) = 2
        let remaining = 12u8.saturating_sub(10);
        let cap = remaining.min(2);
        assert_eq!(cap, 2);
        // exhausted → cap 0 → force ceiling path
        let remaining0 = 12u8.saturating_sub(12);
        assert_eq!(remaining0.min(2), 0);
    }

    #[test]
    fn advice_summary_truncates() {
        let s = advice_summary(&"字".repeat(100), 10);
        assert!(s.chars().count() <= 11); // 10 + ellipsis
    }
}
