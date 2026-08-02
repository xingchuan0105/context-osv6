//! Synthesis answer contract facade: parse → validate → lift → sanitize →
//! envelope → salvage, plus final-answer format rules. The heavy machinery
//! lives in private submodules (`parse`, `final_answer_rules`); this module
//! keeps the shared glue, the prompt-block helpers and the pub API (C5-S4).

mod parse;
mod final_answer_rules;

use parse::*;
use final_answer_rules::*;

pub use parse::{
    InternalAnswerUnifiedV1, InternalAnswerV1, InternalCitationV1, InternalSearchAnswerV1,
    InternalSearchCitationV1, ParsedSynthesisAnswer, UnifiedCitationV1,
    extract_web_marker_indices_public, known_chunk_ids_with_messages, parse_synthesis_answer,
};
pub use final_answer_rules::{
    FINAL_ANSWER_RULES, FinalAnswerRule, FinalAnswerViolation, check_final_answer,
    contract_violation_fallback, contains_executable_code_form, contains_host_observation_shell,
    contains_template_artifact, executable_code_matched, final_answer_contract_violation,
    host_shell_matched, is_code_only_answer, template_artifact_matched,
};

use avrag_llm::ChatMessage;
use contracts::ToolResult;

use super::config::{AnswerContractKind, ModeConfig};

pub fn synthesis_contract_block(mode: &ModeConfig) -> &'static str {
    match mode.synthesis_output.contract {
        AnswerContractKind::InternalSearchAnswerV1 => {
            "Respond with ONLY a JSON object (no markdown fences):\n\
             {\"schema_version\":\"internal_search_answer_v1\",\"answer_text\":\"...\",\"citations\":[{\"index\":1}],\"coverage\":\"full\",\"refusal_reason\":null}\n\
             Use [[n]] in answer_text matching citations[].index from search observations."
        }
        AnswerContractKind::ProseOnly => "",
        AnswerContractKind::InternalAnswerV1 => {
            "Respond with ONLY a JSON object (no markdown fences):\n\
             {\"schema_version\":\"internal_answer_v1\",\"answer_text\":\"prose with [[cite:CHUNK_ID]]\",\"citations\":[{\"chunk_id\":\"...\"}],\"coverage\":\"full\",\"refusal_reason\":null}\n\
             Every citations[].chunk_id MUST appear as [[cite:CHUNK_ID]] in answer_text."
        }
        AnswerContractKind::InternalAnswerUnifiedV1
        | AnswerContractKind::InternalHybridAnswerV1 => {
            // Thin contract: LLM owns prose; server only peels this envelope + hangs markers.
            "Return ONLY this JSON (no markdown fences, no extra keys):\n\
{\"schema_version\":\"internal_answer_unified_v1\",\"answer_text\":\"<markdown prose>\",\
\"citations\":[{\"kind\":\"doc\",\"id\":\"<chunk_id>\"},{\"kind\":\"web\",\"id\":\"<n>\"}],\
\"coverage\":\"full|partial|none\",\"refusal_reason\":null}\n\
Rules:\n\
- answer_text = user-visible markdown only (never paste this JSON into answer_text).\n\
- Doc: [[cite:CHUNK_ID]] next to the claim; citations kind=doc id=CHUNK_ID from tools.\n\
- Web: [[web:n]] next to the claim; citations kind=web id=n (1-based web_search index).\n\
- Do not invent [来源：…] / source footnotes; UI renders markers."
        }
    }
}

/// C6: delegate to the shared stripper so every consumer sees the same fence
/// semantics (json_fence::strip_json_fence). Kept as a wrapper to preserve
/// this module's long-standing public name.
pub fn strip_json_fences(raw: &str) -> String {
    super::json_fence::strip_json_fence(raw)
}

pub fn lift_prose_to_contract(
    raw: &str,
    tool_results: &[ToolResult],
    messages: &[ChatMessage],
    mode: &ModeConfig,
) -> Option<ParsedSynthesisAnswer> {
    let prose = strip_json_fences(raw);
    match mode.synthesis_output.contract {
        AnswerContractKind::InternalAnswerV1 => lift_rag_prose(&prose, tool_results, messages),
        AnswerContractKind::InternalSearchAnswerV1 => lift_search_prose(&prose),
        AnswerContractKind::InternalAnswerUnifiedV1
        | AnswerContractKind::InternalHybridAnswerV1 => {
            lift_unified_prose(&prose, tool_results, messages)
        }
        AnswerContractKind::ProseOnly => None,
    }
}

fn lift_rag_prose(
    prose: &str,
    tool_results: &[ToolResult],
    messages: &[ChatMessage],
) -> Option<ParsedSynthesisAnswer> {
    let cited_ids = crate::cite_extract::extract_referenced_chunk_ids(prose);
    if cited_ids.is_empty() {
        return None;
    }
    let known = known_chunk_ids_with_messages(tool_results, messages);
    let citations: Vec<InternalCitationV1> = cited_ids
        .iter()
        .filter(|id| known.contains(*id))
        .map(|id| InternalCitationV1 {
            chunk_id: id.clone(),
            quote_span: None,
            confidence: None,
        })
        .collect();
    if citations.is_empty() {
        return None;
    }
    Some(ParsedSynthesisAnswer::Rag(InternalAnswerV1 {
        schema_version: "internal_answer_v1".to_string(),
        answer_text: prose.to_string(),
        citations,
        coverage: Some("full".to_string()),
        refusal_reason: None,
    }))
}

fn lift_search_prose(prose: &str) -> Option<ParsedSynthesisAnswer> {
    let indices = extract_search_indices(prose);
    if indices.is_empty() {
        return None;
    }
    let citations: Vec<InternalSearchCitationV1> = indices
        .into_iter()
        .map(|index| InternalSearchCitationV1 { index })
        .collect();
    Some(ParsedSynthesisAnswer::Search(InternalSearchAnswerV1 {
        schema_version: "internal_search_answer_v1".to_string(),
        answer_text: prose.to_string(),
        citations,
        coverage: Some("full".to_string()),
        refusal_reason: None,
    }))
}

fn answer_references_search_index(answer: &str, index: u32) -> bool {
    extract_search_indices(answer).contains(&index)
}

pub fn extract_search_indices(answer: &str) -> Vec<u32> {
    let mut indices = Vec::new();
    let mut rest = answer;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        if let Some(end) = after.find("]]") {
            let inner = after[..end].trim();
            if inner.contains(',') {
                for part in inner.split(',') {
                    if let Ok(index) = part.trim().parse::<u32>() {
                        if !indices.contains(&index) {
                            indices.push(index);
                        }
                    }
                }
            } else if let Ok(index) = inner.parse::<u32>() {
                if !indices.contains(&index) {
                    indices.push(index);
                }
            }
            rest = &after[end + 2..];
        } else {
            break;
        }
    }
    indices
}

pub fn validate_synthesis_answer(
    answer: &ParsedSynthesisAnswer,
    tool_results: &[ToolResult],
    messages: &[ChatMessage],
    mode: &ModeConfig,
) -> Vec<String> {
    match answer {
        ParsedSynthesisAnswer::Rag(ans) => {
            validate_internal_answer(ans, tool_results, messages, mode)
        }
        ParsedSynthesisAnswer::Search(ans) => validate_search_answer(ans, mode),
        ParsedSynthesisAnswer::Unified(ans) => {
            validate_unified_answer(ans, tool_results, messages, mode)
        }
    }
}

fn validate_unified_answer(
    answer: &InternalAnswerUnifiedV1,
    _tool_results: &[ToolResult],
    _messages: &[ChatMessage],
    _mode: &ModeConfig,
) -> Vec<String> {
    // Thin validation: LLM owns cite quality; server only requires non-empty prose.
    // Citation hanging is done later from [[cite:]] / [[web:n]] in the answer body.
    let mut errors = Vec::new();
    if answer.answer_text.trim().is_empty() {
        errors.push("answer_text is empty".to_string());
    }
    errors
}

fn lift_unified_prose(
    prose: &str,
    tool_results: &[ToolResult],
    messages: &[ChatMessage],
) -> Option<ParsedSynthesisAnswer> {
    let text = scrub_internal_answer_tokens(prose);
    if text.trim().is_empty() {
        return None;
    }
    // Never treat a full synthesis JSON envelope as answer_text (common failure mode
    // when parse fails and we fall through to lift with the raw model string).
    let text = if text.trim_start().starts_with('{') {
        if let Some(inner) = unwrap_synthesis_json_envelope(&text) {
            inner
        } else if let Ok(parsed) =
            parse_unified_or_legacy(&normalize_synthesis_json_text(&strip_json_fences(&text)))
        {
            return Some(parsed);
        } else {
            text
        }
    } else {
        text
    };
    let _ = (tool_results, messages);
    let upgraded = upgrade_rag_json_to_unified_from_text(&text, Some("full".into()), None);
    Some(ParsedSynthesisAnswer::Unified(upgraded))
}

fn validate_internal_answer(
    answer: &InternalAnswerV1,
    tool_results: &[ToolResult],
    messages: &[ChatMessage],
    mode: &ModeConfig,
) -> Vec<String> {
    let mut errors = Vec::new();
    if answer.schema_version != "internal_answer_v1"
        && mode.synthesis_output.contract == AnswerContractKind::InternalAnswerV1
    {
        errors.push(format!(
            "expected schema_version internal_answer_v1, got {}",
            answer.schema_version
        ));
    }
    if answer.answer_text.trim().is_empty() {
        errors.push("answer_text is empty".to_string());
    }

    let known = known_chunk_ids_with_messages(tool_results, messages);
    for cite in &answer.citations {
        if !known.contains(&cite.chunk_id) {
            errors.push(format!(
                "citation chunk_id {} not found in tool results",
                cite.chunk_id
            ));
        }
        let marker = format!("[[cite:{}]]", cite.chunk_id);
        if !answer.answer_text.contains(&marker) {
            errors.push(format!("answer_text missing marker {marker}"));
        }
    }

    if answer.citations.is_empty() && mode.id == "rag" {
        let has_cites_in_text = answer.answer_text.contains("[[cite:");
        if has_cites_in_text {
            errors.push("answer_text has cite markers but citations[] is empty".to_string());
        }
    }

    if answer.coverage.as_deref() == Some("none")
        && answer
            .refusal_reason
            .as_ref()
            .is_none_or(|r| r.trim().is_empty())
    {
        errors.push("refusal_reason is required when coverage=none".to_string());
    }

    errors
}

fn validate_search_answer(answer: &InternalSearchAnswerV1, mode: &ModeConfig) -> Vec<String> {
    let mut errors = Vec::new();
    if answer.schema_version != "internal_search_answer_v1" {
        errors.push(format!(
            "expected schema_version internal_search_answer_v1, got {}",
            answer.schema_version
        ));
    }
    if answer.answer_text.trim().is_empty() {
        errors.push("answer_text is empty".to_string());
    }
    for cite in &answer.citations {
        if !answer_references_search_index(&answer.answer_text, cite.index) {
            errors.push(format!(
                "answer_text missing marker for index {}",
                cite.index
            ));
        }
    }
    if answer.citations.is_empty() && mode.id == "search" {
        let has_markers = answer.answer_text.contains("[[");
        if has_markers {
            errors.push("answer_text has index markers but citations[] is empty".to_string());
        }
    }

    if answer.coverage.as_deref() == Some("none")
        && answer
            .refusal_reason
            .as_ref()
            .is_none_or(|r| r.trim().is_empty())
    {
        errors.push("refusal_reason is required when coverage=none".to_string());
    }

    errors
}


pub fn render_synthesis_prose(answer: &ParsedSynthesisAnswer) -> String {
    let raw = match answer {
        ParsedSynthesisAnswer::Rag(a) => a.answer_text.clone(),
        ParsedSynthesisAnswer::Search(a) => a.answer_text.clone(),
        ParsedSynthesisAnswer::Unified(a) => a.answer_text.clone(),
    };
    ensure_user_visible_answer_text(&raw)
}

/// Peel synthesis JSON envelopes until only user prose remains.
pub fn ensure_user_visible_answer_text(raw: &str) -> String {
    let mut text = scrub_internal_answer_tokens(raw);
    // Peel nested / accidental envelope dumps (up to a few times).
    for _ in 0..4 {
        let trimmed = text.trim();
        if !trimmed.starts_with('{') {
            break;
        }
        match unwrap_synthesis_json_envelope(trimmed) {
            Some(inner) if inner.trim() != trimmed => {
                text = scrub_internal_answer_tokens(&inner);
            }
            _ => break,
        }
    }
    let text = rewrite_legacy_web_markers(&text);
    strip_model_source_wrappers(&text)
}

/// Remove model-invented source attribution shells like `[来源：…]` / `**[来源：…]**`.
/// Keeps inline `[[cite:]]` / `[[web:n]]` markers (rescued from inside wrappers).
pub fn strip_model_source_wrappers(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(found) = rest.find("[来源") {
        let mut start = found;
        if start >= 2 && rest.as_bytes()[start - 2] == b'*' && rest.as_bytes()[start - 1] == b'*' {
            start -= 2;
        }
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        let Some(close) = after.find(']') else {
            if let Some(nl) = after.find('\n') {
                rest = &after[nl..];
            } else {
                rest = "";
            }
            continue;
        };
        let block = &after[..close + 1];
        // Rescue citation markers from inside the wrapper before dropping it.
        let rescued = extract_inline_markers_from(block);
        if !rescued.is_empty() {
            if !out.ends_with(|c: char| c.is_whitespace()) {
                out.push(' ');
            }
            out.push_str(&rescued);
            out.push(' ');
        }
        let mut end = close + 1;
        if after.get(end..end + 2) == Some("**") {
            end += 2;
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    for empty in ["[来源： ]", "[来源: ]", "[来源：]", "[来源:]"] {
        out = out.replace(empty, "");
    }
    while out.contains("  ") {
        out = out.replace("  ", " ");
    }
    out
}

fn extract_inline_markers_from(block: &str) -> String {
    let mut parts = Vec::new();
    let mut rest = block;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else {
            break;
        };
        let inner = after[..end].trim();
        if inner.starts_with("cite:")
            || inner.starts_with("image:")
            || inner.starts_with("web:")
            || inner.parse::<u32>().is_ok()
            || inner.contains(',')
        {
            parts.push(format!("[[{inner}]]"));
        }
        rest = &after[end + 2..];
    }
    parts.join(" ")
}

fn partial_evidence_insufficient_zh() -> &'static str {
    super::prompt_assets::partial_evidence_insufficient()
}


fn draft_contains_refusal(answer_text: &str) -> bool {
    DRAFT_REFUSAL_CUES
        .iter()
        .any(|cue| answer_text.contains(cue))
}

fn try_parse_candidate(
    raw: &str,
    tool_results: &[ToolResult],
    messages: &[ChatMessage],
    mode: &ModeConfig,
) -> Option<ParsedSynthesisAnswer> {
    parse_synthesis_answer(raw, mode)
        .ok()
        .or_else(|| lift_prose_to_contract(raw, tool_results, messages, mode))
}

fn strip_unknown_cite_markers(text: &str, known: &std::collections::HashSet<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("]]") else {
            out.push_str(&rest[start..]);
            break;
        };
        let token = after_start[..end].trim();
        let marker = format!("[[{token}]]");
        if let Some(chunk_id) = token.strip_prefix("cite:").map(str::trim) {
            if known.contains(chunk_id) {
                out.push_str(&marker);
            }
        } else if let Some(chunk_id) = token.strip_prefix("image:").map(str::trim) {
            if known.contains(chunk_id) {
                out.push_str(&marker);
            }
        } else {
            out.push_str(&marker);
        }
        rest = &after_start[end + 2..];
    }
    out.push_str(rest);
    collapse_whitespace(&out)
}

fn strip_unknown_search_markers(text: &str, valid_indices: &[u32]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("]]") else {
            out.push_str(&rest[start..]);
            break;
        };
        let inner = after_start[..end].trim();
        let marker = format!("[[{inner}]]");
        let keep = if inner.contains(',') {
            inner.split(',').all(|part| {
                part.trim()
                    .parse::<u32>()
                    .ok()
                    .is_some_and(|index| valid_indices.contains(&index))
            })
        } else {
            inner
                .parse::<u32>()
                .ok()
                .is_some_and(|index| valid_indices.contains(&index))
        };
        if keep {
            out.push_str(&marker);
        }
        rest = &after_start[end + 2..];
    }
    out.push_str(rest);
    collapse_whitespace(&out)
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn sanitize_partial_answer(
    answer: &ParsedSynthesisAnswer,
    tool_results: &[ToolResult],
    messages: &[ChatMessage],
) -> Option<String> {
    sanitize_parsed_answer(answer, tool_results, messages).map(|p| render_synthesis_prose(&p))
}

/// Drop unknown citation ids / markers; keep usable prose (hybrid-safe).
pub fn sanitize_parsed_answer(
    answer: &ParsedSynthesisAnswer,
    tool_results: &[ToolResult],
    messages: &[ChatMessage],
) -> Option<ParsedSynthesisAnswer> {
    match answer {
        ParsedSynthesisAnswer::Rag(ans) => {
            let known = known_chunk_ids_with_messages(tool_results, messages);
            let cleaned =
                scrub_internal_answer_tokens(&strip_unknown_cite_markers(&ans.answer_text, &known));
            if cleaned.chars().count() < 4 {
                return None;
            }
            let citations: Vec<InternalCitationV1> = ans
                .citations
                .iter()
                .filter(|c| known.contains(&c.chunk_id))
                .cloned()
                .collect();
            Some(ParsedSynthesisAnswer::Rag(InternalAnswerV1 {
                schema_version: "internal_answer_v1".to_string(),
                answer_text: cleaned,
                citations,
                coverage: ans.coverage.clone(),
                refusal_reason: ans.refusal_reason.clone(),
            }))
        }
        ParsedSynthesisAnswer::Search(ans) => {
            let valid_indices: Vec<u32> = ans.citations.iter().map(|c| c.index).collect();
            let cleaned = scrub_internal_answer_tokens(&strip_unknown_search_markers(
                &ans.answer_text,
                &valid_indices,
            ));
            if cleaned.chars().count() < 4 {
                return None;
            }
            Some(ParsedSynthesisAnswer::Search(InternalSearchAnswerV1 {
                schema_version: "internal_search_answer_v1".to_string(),
                answer_text: cleaned,
                citations: ans.citations.clone(),
                coverage: ans.coverage.clone(),
                refusal_reason: ans.refusal_reason.clone(),
            }))
        }
        ParsedSynthesisAnswer::Unified(ans) => {
            let known = known_chunk_ids_with_messages(tool_results, messages);
            let mut cleaned = scrub_internal_answer_tokens(&ans.answer_text);
            cleaned = strip_unknown_cite_markers(&cleaned, &known);
            // Keep [[web:n]] always for now (web index validation is softer).
            cleaned = rewrite_legacy_web_markers(&cleaned);
            if cleaned.chars().count() < 4 {
                return None;
            }
            let citations: Vec<UnifiedCitationV1> = ans
                .citations
                .iter()
                .filter(|c| match c.kind.as_str() {
                    "doc" => known.contains(&c.id),
                    "web" => c.id.parse::<u32>().is_ok(),
                    _ => false,
                })
                .cloned()
                .collect();
            Some(ParsedSynthesisAnswer::Unified(InternalAnswerUnifiedV1 {
                schema_version: "internal_answer_unified_v1".into(),
                answer_text: cleaned,
                citations,
                coverage: ans.coverage.clone(),
                refusal_reason: ans.refusal_reason.clone(),
            }))
        }
    }
}

/// If a string is a synthesis JSON envelope, return `answer_text` only.
pub fn unwrap_synthesis_json_envelope(raw: &str) -> Option<String> {
    let body = normalize_synthesis_json_text(&strip_json_fences(raw));
    let trimmed = body.trim();
    if !trimmed.starts_with('{') {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let schema = value
        .get("schema_version")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let schema_norm = normalize_schema_version_value(schema);
    let looks_like_synthesis = schema_norm.contains("internal_answer")
        || schema_norm.contains("internal_search")
        || value.get("answer_text").is_some()
            && (value.get("citations").is_some() || value.get("coverage").is_some());
    if !looks_like_synthesis {
        return None;
    }
    let text = value.get("answer_text").and_then(|v| v.as_str())?;
    let t = scrub_internal_answer_tokens(text);
    if t.is_empty() {
        return None;
    }
    Some(rewrite_legacy_web_markers(&t))
}

/// When synthesis JSON fails validation but the model attempted an answer, salvage
/// usable prose by dropping invalid citations. Do **not** abort salvage just because
/// analytical text contains phrases like「未提及」.
pub fn extract_partial_synthesis_fallback(
    candidates: &[&str],
    tool_results: &[ToolResult],
    messages: &[ChatMessage],
    mode: &ModeConfig,
) -> Option<String> {
    if mode.synthesis_output.contract == AnswerContractKind::ProseOnly {
        return None;
    }

    for raw in candidates.iter().rev() {
        let Some(parsed) = try_parse_candidate(raw, tool_results, messages, mode) else {
            // Unparseable as contract: still try envelope unwrap for user-facing prose.
            if let Some(text) = unwrap_synthesis_json_envelope(raw) {
                if text.chars().count() >= 4 && !draft_contains_refusal(&text) {
                    return Some(text);
                }
            }
            continue;
        };
        let answer_text = match &parsed {
            ParsedSynthesisAnswer::Rag(a) => a.answer_text.as_str(),
            ParsedSynthesisAnswer::Search(a) => a.answer_text.as_str(),
            ParsedSynthesisAnswer::Unified(a) => a.answer_text.as_str(),
        };
        // Explicit contract refusal (coverage=none / refusal_reason) still skips salvage.
        if draft_contains_refusal(answer_text)
            || matches!(
                &parsed,
                ParsedSynthesisAnswer::Rag(a) if a.coverage.as_deref() == Some("none")
            )
            || matches!(
                &parsed,
                ParsedSynthesisAnswer::Search(a) if a.coverage.as_deref() == Some("none")
            )
            || matches!(
                &parsed,
                ParsedSynthesisAnswer::Unified(a) if a.coverage.as_deref() == Some("none")
            )
        {
            continue;
        }
        if let Some(cleaned) = sanitize_partial_answer(&parsed, tool_results, messages) {
            return Some(cleaned);
        }
    }

    if candidates.iter().any(|raw| {
        try_parse_candidate(raw, tool_results, messages, mode).is_some_and(|parsed| {
            let answer_text = match &parsed {
                ParsedSynthesisAnswer::Rag(a) => a.answer_text.as_str(),
                ParsedSynthesisAnswer::Search(a) => a.answer_text.as_str(),
                ParsedSynthesisAnswer::Unified(a) => a.answer_text.as_str(),
            };
            !draft_contains_refusal(answer_text)
        })
    }) {
        return Some(partial_evidence_insufficient_zh().to_string());
    }

    None
}

pub fn resolve_synthesis_answer(
    candidates: &[&str],
    tool_results: &[ToolResult],
    messages: &[ChatMessage],
    mode: &ModeConfig,
) -> Option<ParsedSynthesisAnswer> {
    let unified = matches!(
        mode.synthesis_output.contract,
        AnswerContractKind::InternalAnswerUnifiedV1 | AnswerContractKind::InternalHybridAnswerV1
    );

    for raw in candidates {
        if let Ok(parsed) = parse_synthesis_answer(raw, mode) {
            if unified {
                // Thin path: trust LLM answer_text; do not fail the turn on cite hygiene.
                let errors = validate_synthesis_answer(&parsed, tool_results, messages, mode);
                if errors.is_empty() {
                    return Some(parsed);
                }
                // Empty answer_text only — try next candidate / unwrap.
                tracing::warn!(?errors, "unified synthesis rejected (empty prose)");
                continue;
            }
            let errors = validate_synthesis_answer(&parsed, tool_results, messages, mode);
            if errors.is_empty() {
                return Some(parsed);
            }
            tracing::warn!(?errors, "synthesis JSON failed validation");
            if let Some(sanitized) = sanitize_parsed_answer(&parsed, tool_results, messages) {
                return Some(sanitized);
            }
        }
        // Minimal peel: if model returned an envelope, use answer_text only.
        if unified {
            if let Some(text) = unwrap_synthesis_json_envelope(raw) {
                if !text.trim().is_empty() {
                    return Some(ParsedSynthesisAnswer::Unified(InternalAnswerUnifiedV1 {
                        schema_version: "internal_answer_unified_v1".into(),
                        answer_text: text,
                        citations: vec![],
                        coverage: Some("full".into()),
                        refusal_reason: None,
                    }));
                }
            }
            continue;
        }
        if let Some(lifted) = lift_prose_to_contract(raw, tool_results, messages, mode) {
            let errors = validate_synthesis_answer(&lifted, tool_results, messages, mode);
            if errors.is_empty() {
                return Some(lifted);
            }
            if let Some(sanitized) = sanitize_parsed_answer(&lifted, tool_results, messages) {
                return Some(sanitized);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests;
