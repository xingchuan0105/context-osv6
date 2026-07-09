use avrag_llm::ChatMessage;
use contracts::ToolResult;
use serde::{Deserialize, Serialize};

use super::config::{AnswerContractKind, ModeConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalCitationV1 {
    pub chunk_id: String,
    #[serde(default)]
    pub quote_span: Option<String>,
    #[serde(default)]
    pub confidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalAnswerV1 {
    pub schema_version: String,
    pub answer_text: String,
    #[serde(default)]
    pub citations: Vec<InternalCitationV1>,
    #[serde(default)]
    pub coverage: Option<String>,
    #[serde(default)]
    pub refusal_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalSearchCitationV1 {
    pub index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalSearchAnswerV1 {
    pub schema_version: String,
    pub answer_text: String,
    #[serde(default)]
    pub citations: Vec<InternalSearchCitationV1>,
    #[serde(default)]
    pub coverage: Option<String>,
    #[serde(default)]
    pub refusal_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ParsedSynthesisAnswer {
    Rag(InternalAnswerV1),
    Search(InternalSearchAnswerV1),
}

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
    }
}

pub fn strip_json_fences(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with("```") {
        let inner = trimmed
            .trim_start_matches('`')
            .trim_start_matches("json")
            .trim();
        if let Some(end) = inner.rfind("```") {
            return inner[..end].trim().to_string();
        }
    }
    trimmed.to_string()
}

pub fn parse_synthesis_answer(
    raw: &str,
    mode: &ModeConfig,
) -> Result<ParsedSynthesisAnswer, String> {
    let body = strip_json_fences(raw);
    match mode.synthesis_output.contract {
        AnswerContractKind::InternalSearchAnswerV1 => {
            let parsed: InternalSearchAnswerV1 =
                serde_json::from_str(&body).map_err(|e| format!("json parse error: {e}"))?;
            Ok(ParsedSynthesisAnswer::Search(parsed))
        }
        AnswerContractKind::InternalAnswerV1 => {
            let parsed: InternalAnswerV1 =
                serde_json::from_str(&body).map_err(|e| format!("json parse error: {e}"))?;
            Ok(ParsedSynthesisAnswer::Rag(parsed))
        }
        AnswerContractKind::ProseOnly => Err("prose_only has no synthesis contract".to_string()),
    }
}

pub fn known_chunk_ids(tool_results: &[ToolResult]) -> std::collections::HashSet<String> {
    known_chunk_ids_with_messages(tool_results, &[])
}

pub fn known_chunk_ids_with_messages(
    tool_results: &[ToolResult],
    messages: &[ChatMessage],
) -> std::collections::HashSet<String> {
    let mut ids = std::collections::HashSet::new();
    for result in tool_results {
        if let Some(data) = &result.data {
            collect_chunk_ids_from_value(data, &mut ids);
        }
    }
    for message in messages {
        collect_chunk_ids_from_text(&message.content, &mut ids);
    }
    ids
}

fn collect_chunk_ids_from_text(text: &str, ids: &mut std::collections::HashSet<String>) {
    let mut rest = text;
    while let Some(start) = rest.find("chunk_id") {
        let tail = &rest[start..];
        let after_key = tail.strip_prefix("chunk_id").unwrap_or(tail);
        let after_colon = after_key
            .split_once(':')
            .map(|(_, v)| v)
            .unwrap_or(after_key);
        let trimmed = after_colon.trim().trim_matches('"');
        if !trimmed.is_empty() {
            let id = trimmed
                .split(|c: char| c == '"' || c == ',' || c == '}' || c.is_whitespace())
                .next()
                .unwrap_or(trimmed);
            if !id.is_empty() && id != "null" {
                ids.insert(id.to_string());
            }
        }
        rest = &rest[start + 8..];
    }
}

fn collect_chunk_ids_from_value(
    value: &serde_json::Value,
    ids: &mut std::collections::HashSet<String>,
) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(id) = map.get("chunk_id").and_then(|v| v.as_str()) {
                ids.insert(id.to_string());
            }
            for v in map.values() {
                collect_chunk_ids_from_value(v, ids);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                collect_chunk_ids_from_value(v, ids);
            }
        }
        _ => {}
    }
}

pub fn lift_prose_to_contract(
    raw: &str,
    tool_results: &[ToolResult],
    messages: &[ChatMessage],
    mode: &ModeConfig,
) -> Option<ParsedSynthesisAnswer> {
    let prose = strip_json_fences(raw);
    match mode.synthesis_output.contract {
        AnswerContractKind::InternalAnswerV1 => {
            let cited_ids = crate::rag_prompts::extract_referenced_chunk_ids(&prose);
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
                answer_text: prose,
                citations,
                coverage: Some("full".to_string()),
                refusal_reason: None,
            }))
        }
        AnswerContractKind::InternalSearchAnswerV1 => {
            let indices = extract_search_indices(&prose);
            if indices.is_empty() {
                return None;
            }
            let citations: Vec<InternalSearchCitationV1> = indices
                .into_iter()
                .map(|index| InternalSearchCitationV1 { index })
                .collect();
            Some(ParsedSynthesisAnswer::Search(InternalSearchAnswerV1 {
                schema_version: "internal_search_answer_v1".to_string(),
                answer_text: prose,
                citations,
                coverage: Some("full".to_string()),
                refusal_reason: None,
            }))
        }
        AnswerContractKind::ProseOnly => None,
    }
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
    }
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

/// Collect validation errors from synthesis candidates (for repair prompts).
pub fn collect_synthesis_validation_errors(
    candidates: &[&str],
    tool_results: &[ToolResult],
    messages: &[ChatMessage],
    mode: &ModeConfig,
) -> Vec<String> {
    let mut errors = Vec::new();
    for raw in candidates {
        if let Ok(parsed) = parse_synthesis_answer(raw, mode) {
            errors.extend(validate_synthesis_answer(
                &parsed,
                tool_results,
                messages,
                mode,
            ));
        } else if let Some(lifted) = lift_prose_to_contract(raw, tool_results, messages, mode) {
            errors.extend(validate_synthesis_answer(
                &lifted,
                tool_results,
                messages,
                mode,
            ));
        } else {
            errors.push("response is not valid synthesis JSON".to_string());
        }
    }
    errors.sort();
    errors.dedup();
    errors
}

pub fn render_synthesis_prose(answer: &ParsedSynthesisAnswer) -> String {
    match answer {
        ParsedSynthesisAnswer::Rag(a) => a.answer_text.clone(),
        ParsedSynthesisAnswer::Search(a) => a.answer_text.clone(),
    }
}

const PARTIAL_EVIDENCE_INSUFFICIENT_ZH: &str = "资料不足以完整回答";

const DRAFT_REFUSAL_CUES: &[&str] = &[
    "未找到",
    "未提及",
    "未提到",
    "没有提及",
    "没有找到",
    "没有提到",
    "未在文档中找到",
    "文档中未",
    "资料中未",
    "不在文档",
    "不在资料",
    "未提供",
    "无法确认",
    "无法确定",
    "无法回答",
    "暂无相关",
    "无相关内容",
];

pub fn contract_violation_fallback(mode_id: &str) -> String {
    match mode_id {
        "rag" => "找到了相关资料，但未能生成符合引用格式要求的完整答案，请尝试重新提问。".to_string(),
        "search" => "找到了搜索结果，但未能生成符合格式要求的完整答案，请尝试重新提问。".to_string(),
        _ => "未能生成符合格式要求的完整答案。".to_string(),
    }
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
    match answer {
        ParsedSynthesisAnswer::Rag(ans) => {
            let known = known_chunk_ids_with_messages(tool_results, messages);
            let cleaned = strip_unknown_cite_markers(&ans.answer_text, &known);
            if cleaned.chars().count() >= 4 {
                Some(cleaned)
            } else {
                None
            }
        }
        ParsedSynthesisAnswer::Search(ans) => {
            let valid_indices: Vec<u32> = ans.citations.iter().map(|c| c.index).collect();
            let cleaned = strip_unknown_search_markers(&ans.answer_text, &valid_indices);
            if cleaned.chars().count() >= 4 {
                Some(cleaned)
            } else {
                None
            }
        }
    }
}

/// When synthesis JSON fails validation but the model attempted an answer (no refusal
/// phrasing in draft `answer_text`), salvage usable prose by dropping invalid citations.
pub fn extract_partial_synthesis_fallback(
    candidates: &[&str],
    tool_results: &[ToolResult],
    messages: &[ChatMessage],
    mode: &ModeConfig,
) -> Option<String> {
    if mode.synthesis_output.contract == AnswerContractKind::ProseOnly {
        return None;
    }

    for raw in candidates {
        if let Some(parsed) = try_parse_candidate(raw, tool_results, messages, mode) {
            let answer_text = match &parsed {
                ParsedSynthesisAnswer::Rag(a) => &a.answer_text,
                ParsedSynthesisAnswer::Search(a) => &a.answer_text,
            };
            if draft_contains_refusal(answer_text) {
                return None;
            }
        }
    }

    for raw in candidates.iter().rev() {
        let Some(parsed) = try_parse_candidate(raw, tool_results, messages, mode) else {
            continue;
        };
        if let Some(cleaned) = sanitize_partial_answer(&parsed, tool_results, messages) {
            return Some(cleaned);
        }
    }

    if candidates.iter().any(|raw| {
        try_parse_candidate(raw, tool_results, messages, mode).is_some_and(|parsed| {
            let answer_text = match &parsed {
                ParsedSynthesisAnswer::Rag(a) => &a.answer_text,
                ParsedSynthesisAnswer::Search(a) => &a.answer_text,
            };
            !draft_contains_refusal(answer_text)
        })
    }) {
        return Some(PARTIAL_EVIDENCE_INSUFFICIENT_ZH.to_string());
    }

    None
}

pub fn resolve_synthesis_answer(
    candidates: &[&str],
    tool_results: &[ToolResult],
    messages: &[ChatMessage],
    mode: &ModeConfig,
) -> Option<ParsedSynthesisAnswer> {
    for raw in candidates {
        if let Ok(parsed) = parse_synthesis_answer(raw, mode) {
            let errors = validate_synthesis_answer(&parsed, tool_results, messages, mode);
            if errors.is_empty() {
                return Some(parsed);
            }
            tracing::warn!(?errors, "synthesis JSON failed validation");
        }
        if let Some(lifted) = lift_prose_to_contract(raw, tool_results, messages, mode) {
            let errors = validate_synthesis_answer(&lifted, tool_results, messages, mode);
            if errors.is_empty() {
                return Some(lifted);
            }
            tracing::warn!(?errors, "synthesis prose lift failed validation");
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_rag_json() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let raw = r#"{"schema_version":"internal_answer_v1","answer_text":"Hi [[cite:a]]","citations":[{"chunk_id":"a"}]}"#;
        let parsed = parse_synthesis_answer(raw, &mode).unwrap();
        match parsed {
            ParsedSynthesisAnswer::Rag(a) => assert_eq!(a.citations[0].chunk_id, "a"),
            _ => panic!("expected rag"),
        }
    }

    #[test]
    fn validate_rejects_unknown_chunk() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let answer = ParsedSynthesisAnswer::Rag(InternalAnswerV1 {
            schema_version: "internal_answer_v1".to_string(),
            answer_text: "Text [[cite:missing]]".to_string(),
            citations: vec![InternalCitationV1 {
                chunk_id: "missing".to_string(),
                quote_span: None,
                confidence: None,
            }],
            coverage: Some("full".to_string()),
            refusal_reason: None,
        });
        let errors = validate_synthesis_answer(&answer, &[], &[], &mode);
        assert!(!errors.is_empty());
    }

    #[test]
    fn validates_search_combined_index_markers() {
        let mode = super::super::config::load_mode_config("search").unwrap();
        let answer = ParsedSynthesisAnswer::Search(InternalSearchAnswerV1 {
            schema_version: "internal_search_answer_v1".to_string(),
            answer_text: "Sources [[1, 2]] agree.".to_string(),
            citations: vec![
                InternalSearchCitationV1 { index: 1 },
                InternalSearchCitationV1 { index: 2 },
            ],
            coverage: Some("full".to_string()),
            refusal_reason: None,
        });
        assert!(validate_synthesis_answer(&answer, &[], &[], &mode).is_empty());
    }

    #[test]
    fn rejects_coverage_none_without_refusal_reason() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let answer = ParsedSynthesisAnswer::Rag(InternalAnswerV1 {
            schema_version: "internal_answer_v1".to_string(),
            answer_text: "No evidence.".to_string(),
            citations: vec![],
            coverage: Some("none".to_string()),
            refusal_reason: None,
        });
        let errors = validate_synthesis_answer(&answer, &[], &[], &mode);
        assert!(errors.iter().any(|e| e.contains("refusal_reason")));
    }

    #[test]
    fn lifts_rag_prose_with_cite_markers() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let tool_results = vec![contracts::ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({"chunks": [{"chunk_id": "abc"}]})),
            trace: None,
        }];
        let lifted = lift_prose_to_contract(
            "Antifragility means gain from disorder [[cite:abc]]",
            &tool_results,
            &[],
            &mode,
        )
        .unwrap();
        assert!(validate_synthesis_answer(&lifted, &tool_results, &[], &mode).is_empty());
    }

    #[test]
    fn contract_violation_fallback_rag_is_chinese() {
        let fallback = contract_violation_fallback("rag");
        assert!(!fallback.contains("I found"));
        assert!(fallback.contains('，') || fallback.contains('。') || fallback.chars().any(|c| c > '\u{4e00}'));
    }

    #[test]
    fn extract_partial_fallback_strips_invalid_citations() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let tool_results = vec![contracts::ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({"chunks": [{"chunk_id": "good"}]})),
            trace: None,
        }];
        let raw = r#"{"schema_version":"internal_answer_v1","answer_text":"公司于2019年在大连建厂[[cite:good]][[cite:bad]]，营收550万元。","citations":[{"chunk_id":"good"},{"chunk_id":"bad"}],"coverage":"full","refusal_reason":null}"#;
        let partial = extract_partial_synthesis_fallback(&[raw], &tool_results, &[], &mode)
            .expect("expected partial answer");
        assert!(partial.contains("2019年在大连建厂"));
        assert!(partial.contains("[[cite:good]]"));
        assert!(!partial.contains("[[cite:bad]]"));
    }

    #[test]
    fn extract_partial_fallback_returns_insufficient_zh_when_text_empty_after_strip() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let tool_results = vec![contracts::ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({"chunks": [{"chunk_id": "good"}]})),
            trace: None,
        }];
        let raw = r#"{"schema_version":"internal_answer_v1","answer_text":"[[cite:bad]]","citations":[{"chunk_id":"bad"}],"coverage":"full","refusal_reason":null}"#;
        let partial = extract_partial_synthesis_fallback(&[raw], &tool_results, &[], &mode)
            .expect("expected insufficient fallback");
        assert_eq!(partial, PARTIAL_EVIDENCE_INSUFFICIENT_ZH);
    }

    #[test]
    fn extract_partial_fallback_skips_when_draft_contains_refusal() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let raw = r#"{"schema_version":"internal_answer_v1","answer_text":"文档中未找到保修期限相关信息。","citations":[],"coverage":"none","refusal_reason":"not found"}"#;
        assert!(extract_partial_synthesis_fallback(&[raw], &[], &[], &mode).is_none());
    }

    #[test]
    fn extract_partial_fallback_prefers_latest_candidate() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let tool_results = vec![contracts::ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({"chunks": [{"chunk_id": "a"}]})),
            trace: None,
        }];
        let first = r#"{"schema_version":"internal_answer_v1","answer_text":"旧答案[[cite:missing]]","citations":[{"chunk_id":"missing"}]}"#;
        let second = r#"{"schema_version":"internal_answer_v1","answer_text":"新答案基于证据[[cite:a]]","citations":[{"chunk_id":"a"}]}"#;
        let partial =
            extract_partial_synthesis_fallback(&[first, second], &tool_results, &[], &mode)
                .expect("expected partial answer");
        assert!(partial.contains("新答案"));
        assert!(!partial.contains("旧答案"));
    }
}
