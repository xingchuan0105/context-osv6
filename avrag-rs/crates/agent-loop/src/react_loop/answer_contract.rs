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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedCitationV1 {
    /// `doc` | `web`
    pub kind: String,
    /// chunk_id for doc, web observation index as string for web.
    pub id: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalAnswerUnifiedV1 {
    pub schema_version: String,
    pub answer_text: String,
    #[serde(default)]
    pub citations: Vec<UnifiedCitationV1>,
    #[serde(default)]
    pub coverage: Option<String>,
    #[serde(default)]
    pub refusal_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ParsedSynthesisAnswer {
    Rag(InternalAnswerV1),
    Search(InternalSearchAnswerV1),
    Unified(InternalAnswerUnifiedV1),
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

pub fn parse_synthesis_answer(
    raw: &str,
    mode: &ModeConfig,
) -> Result<ParsedSynthesisAnswer, String> {
    let body = normalize_synthesis_json_text(&strip_json_fences(raw));
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
        AnswerContractKind::InternalAnswerUnifiedV1
        | AnswerContractKind::InternalHybridAnswerV1 => parse_unified_or_legacy(&body),
        AnswerContractKind::ProseOnly => Err("prose_only has no synthesis contract".to_string()),
    }
}

/// Normalize mangled model JSON keys/values (e.g. schemaversion → schema_version).
fn normalize_synthesis_json_text(body: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(body) else {
        return body.to_string();
    };
    normalize_json_value_keys(&mut value);
    serde_json::to_string(&value).unwrap_or_else(|_| body.to_string())
}

fn normalize_json_value_keys(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            let keys: Vec<String> = map.keys().cloned().collect();
            let mut remapped = serde_json::Map::new();
            for k in keys {
                if let Some(mut v) = map.remove(&k) {
                    normalize_json_value_keys(&mut v);
                    let nk = normalize_synthesis_key(&k);
                    if nk == "schema_version" {
                        if let Some(s) = v.as_str() {
                            v = serde_json::Value::String(normalize_schema_version_value(s));
                        }
                    }
                    if nk == "kind" {
                        if let Some(s) = v.as_str() {
                            let lower = s.to_ascii_lowercase();
                            if lower == "doc" || lower == "document" || lower == "rag" {
                                v = serde_json::Value::String("doc".into());
                            } else if lower == "web" || lower == "search" {
                                v = serde_json::Value::String("web".into());
                            }
                        }
                    }
                    remapped.insert(nk, v);
                }
            }
            *map = remapped;
            // Promote legacy citation shapes inside citations arrays.
            if let Some(serde_json::Value::Array(cites)) = map.get_mut("citations") {
                for c in cites.iter_mut() {
                    if let serde_json::Value::Object(cm) = c {
                        if !cm.contains_key("kind") {
                            if cm.contains_key("chunk_id") {
                                let id = cm
                                    .get("chunk_id")
                                    .and_then(|x| x.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                cm.insert("kind".into(), serde_json::json!("doc"));
                                cm.insert("id".into(), serde_json::json!(id));
                            } else if cm.contains_key("index") {
                                let id = cm
                                    .get("index")
                                    .map(|x| match x {
                                        serde_json::Value::Number(n) => n.to_string(),
                                        serde_json::Value::String(s) => s.clone(),
                                        _ => String::new(),
                                    })
                                    .unwrap_or_default();
                                cm.insert("kind".into(), serde_json::json!("web"));
                                cm.insert("id".into(), serde_json::json!(id));
                            }
                        }
                    }
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                normalize_json_value_keys(v);
            }
        }
        _ => {}
    }
}

fn normalize_synthesis_key(key: &str) -> String {
    match key {
        "schemaversion" | "schemaVersion" | "schema_version" => "schema_version".into(),
        "answertext" | "answerText" | "answer_text" => "answer_text".into(),
        "chunkid" | "chunkId" | "chunk_id" => "chunk_id".into(),
        "refusalreason" | "refusalReason" | "refusal_reason" => "refusal_reason".into(),
        other => other.to_string(),
    }
}

fn normalize_schema_version_value(s: &str) -> String {
    let compact: String = s
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    match compact.as_str() {
        "internalanswerunifiedv1" | "internalhybridanswerv1" => "internal_answer_unified_v1".into(),
        "internalanswerv1" => "internal_answer_v1".into(),
        "internalsearchanswerv1" => "internal_search_answer_v1".into(),
        _ => s.to_string(),
    }
}

fn parse_unified_or_legacy(body: &str) -> Result<ParsedSynthesisAnswer, String> {
    if let Ok(parsed) = serde_json::from_str::<InternalAnswerUnifiedV1>(body) {
        if parsed.schema_version == "internal_answer_unified_v1"
            || parsed.schema_version == "internal_hybrid_answer_v1"
            || parsed.schema_version == "internal_answer_v1"
            || parsed.schema_version.starts_with("internal_answer")
        {
            // Accept if it has unified citations shape or answer_text.
            if !parsed.answer_text.is_empty() {
                let mut p = parsed;
                if p.schema_version != "internal_answer_unified_v1" {
                    // Convert legacy rag-shaped unified parse (citations may be empty/doc).
                    if p.citations.is_empty() {
                        p = upgrade_rag_json_to_unified_from_text(
                            &p.answer_text,
                            p.coverage,
                            p.refusal_reason,
                        );
                    } else {
                        p.schema_version = "internal_answer_unified_v1".into();
                    }
                }
                return Ok(ParsedSynthesisAnswer::Unified(p));
            }
        }
    }
    if let Ok(parsed) = serde_json::from_str::<InternalAnswerV1>(body) {
        if parsed.schema_version != "internal_search_answer_v1" {
            return Ok(ParsedSynthesisAnswer::Unified(rag_to_unified(parsed)));
        }
    }
    if let Ok(parsed) = serde_json::from_str::<InternalSearchAnswerV1>(body) {
        return Ok(ParsedSynthesisAnswer::Unified(search_to_unified(parsed)));
    }
    Err(
        "json parse error: expected internal_answer_unified_v1 (or legacy rag/search schemas)"
            .to_string(),
    )
}

fn rag_to_unified(parsed: InternalAnswerV1) -> InternalAnswerUnifiedV1 {
    let mut citations: Vec<UnifiedCitationV1> = parsed
        .citations
        .into_iter()
        .map(|c| UnifiedCitationV1 {
            kind: "doc".into(),
            id: c.chunk_id,
            url: None,
            title: None,
        })
        .collect();
    // Also harvest [[web:n]] from text if model mixed styles.
    for n in extract_web_marker_indices(&parsed.answer_text) {
        let id = n.to_string();
        if !citations.iter().any(|c| c.kind == "web" && c.id == id) {
            citations.push(UnifiedCitationV1 {
                kind: "web".into(),
                id,
                url: None,
                title: None,
            });
        }
    }
    InternalAnswerUnifiedV1 {
        schema_version: "internal_answer_unified_v1".into(),
        answer_text: rewrite_legacy_web_markers(&parsed.answer_text),
        citations,
        coverage: parsed.coverage,
        refusal_reason: parsed.refusal_reason,
    }
}

fn search_to_unified(parsed: InternalSearchAnswerV1) -> InternalAnswerUnifiedV1 {
    let citations = parsed
        .citations
        .into_iter()
        .map(|c| UnifiedCitationV1 {
            kind: "web".into(),
            id: c.index.to_string(),
            url: None,
            title: None,
        })
        .collect();
    InternalAnswerUnifiedV1 {
        schema_version: "internal_answer_unified_v1".into(),
        answer_text: rewrite_legacy_web_markers(&parsed.answer_text),
        citations,
        coverage: parsed.coverage,
        refusal_reason: parsed.refusal_reason,
    }
}

fn upgrade_rag_json_to_unified_from_text(
    answer_text: &str,
    coverage: Option<String>,
    refusal_reason: Option<String>,
) -> InternalAnswerUnifiedV1 {
    let mut citations = Vec::new();
    for id in extract_cite_chunk_ids(answer_text) {
        citations.push(UnifiedCitationV1 {
            kind: "doc".into(),
            id,
            url: None,
            title: None,
        });
    }
    for n in extract_web_marker_indices(answer_text) {
        citations.push(UnifiedCitationV1 {
            kind: "web".into(),
            id: n.to_string(),
            url: None,
            title: None,
        });
    }
    InternalAnswerUnifiedV1 {
        schema_version: "internal_answer_unified_v1".into(),
        answer_text: rewrite_legacy_web_markers(answer_text),
        citations,
        coverage,
        refusal_reason,
    }
}

fn extract_cite_chunk_ids(text: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("[[cite:") {
        let after = &rest[start + 7..];
        if let Some(end) = after.find("]]") {
            let id = after[..end].trim().to_string();
            if !id.is_empty() && !ids.contains(&id) {
                ids.push(id);
            }
            rest = &after[end + 2..];
        } else {
            break;
        }
    }
    ids
}

/// Public for citation filtering (`[[web:n]]` + legacy bare `[[n]]`).
pub fn extract_web_marker_indices_public(text: &str) -> Vec<u32> {
    extract_web_marker_indices(text)
}

fn extract_web_marker_indices(text: &str) -> Vec<u32> {
    let mut indices = Vec::new();
    let mut rest = text;
    // [[web:n]]
    while let Some(start) = rest.find("[[web:") {
        let after = &rest[start + 6..];
        if let Some(end) = after.find("]]") {
            if let Ok(n) = after[..end].trim().parse::<u32>() {
                if !indices.contains(&n) {
                    indices.push(n);
                }
            }
            rest = &after[end + 2..];
        } else {
            break;
        }
    }
    // legacy bare [[n]]
    rest = text;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        if after.starts_with("cite:") || after.starts_with("image:") || after.starts_with("web:") {
            if let Some(end) = after.find("]]") {
                rest = &after[end + 2..];
                continue;
            }
            break;
        }
        if let Some(end) = after.find("]]") {
            let inner = after[..end].trim();
            if let Ok(n) = inner.parse::<u32>() {
                if !indices.contains(&n) {
                    indices.push(n);
                }
            }
            rest = &after[end + 2..];
        } else {
            break;
        }
    }
    indices
}

/// Rewrite legacy bare `[[n]]` web markers to `[[web:n]]` (leave [[cite:]] alone).
fn rewrite_legacy_web_markers(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else {
            out.push_str(&rest[start..]);
            break;
        };
        let inner = after[..end].trim();
        if inner.starts_with("cite:") || inner.starts_with("image:") || inner.starts_with("web:") {
            out.push_str(&format!("[[{inner}]]"));
        } else if let Ok(n) = inner.parse::<u32>() {
            out.push_str(&format!("[[web:{n}]]"));
        } else {
            out.push_str(&format!("[[{inner}]]"));
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

/// Strip internal salvage tokens models sometimes append into answer_text.
fn scrub_internal_answer_tokens(text: &str) -> String {
    let mut t = text.to_string();
    for junk in [
        "EVIDENCEINSUFFICIENTFALLBACK",
        "EVIDENCE_INSUFFICIENT_FALLBACK",
        "PARTIAL_EVIDENCE_INSUFFICIENT",
    ] {
        t = t.replace(junk, "");
    }
    t.trim().to_string()
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

/// Strong refusal cues only. Avoid mid-sentence phrases like「未提及…」in analytical prose
/// (that false-positive aborted hybrid salvage and leaked raw synthesis JSON).
const DRAFT_REFUSAL_CUES: &[&str] = &[
    "未在文档中找到",
    "文档中未找到",
    "资料中未找到",
    "资料不足以",
    "无法回答",
    "暂无相关",
    "无相关内容",
    "没有找到相关",
];

pub fn contract_violation_fallback(mode_id: &str) -> String {
    super::prompt_assets::contract_violation_fallback(mode_id).to_string()
}

/// prose_only-contract detector: true when `text` carries code spans
/// (`<code>…</code>` or markdown fences) but no prose outside them — the
/// retrieve-phase "output one code block" framing leaked into the final
/// answer. Detector only (host structural check); the repair observation
/// lives in `prompts/loop/synthesis-prose-repair.nudge.md`.
///
/// Stricter than `parse::parse_llm_output`'s CodeBlocks classification on
/// purpose: a prose answer that *quotes* one fenced query is a valid answer
/// and must not trigger a repair round.
pub fn is_code_only_answer(text: &str) -> bool {
    let mut saw_code = false;
    let mut outside = String::new();
    let mut rest = text;
    // `<code …>…</code>` spans (inline or block) — same tag shape parse.rs
    // treats as executable.
    while let Some(start) = rest.find("<code") {
        let Some(tag_end) = rest[start..].find('>').map(|o| start + o) else {
            break;
        };
        let Some(close) = rest[tag_end..].find("</code>").map(|o| tag_end + o) else {
            break;
        };
        outside.push_str(&rest[..start]);
        saw_code = true;
        rest = &rest[close + "</code>".len()..];
    }
    outside.push_str(rest);
    // Markdown fences of ANY language: a fence-only answer is not prose no
    // matter the tag (unlike parse.rs, which only executes python fences).
    let mut prose = String::new();
    let mut in_fence = false;
    for line in outside.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            saw_code = true;
            continue;
        }
        if !in_fence {
            prose.push_str(line);
            prose.push('\n');
        }
    }
    saw_code && prose.trim().is_empty()
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
mod tests {
    use super::*;

    #[test]
    fn code_only_detector_flags_block_answers() {
        // The observed terminal-answer failure shapes.
        assert!(is_code_only_answer(
            "<code language=\"python\">print(1)</code>"
        ));
        assert!(is_code_only_answer("```python\nprint(1)\n```"));
        assert!(is_code_only_answer("```sql\nSELECT 1\n```"));
        // Truncated stream: unclosed fence is still a code-only answer.
        assert!(is_code_only_answer("```python\nprint(1)"));
    }

    #[test]
    fn code_only_detector_accepts_prose() {
        assert!(!is_code_only_answer("答案是 LPDT-03。"));
        // Prose quoting a fenced query is a valid answer, not a violation.
        assert!(!is_code_only_answer(
            "查询结果如下：\n```sql\nSELECT 1\n```\n如上所示共 3 行。"
        ));
        // Inline `<code>` inside prose leaves prose behind.
        assert!(!is_code_only_answer("使用 <code>foo()</code> 即可。"));
        // Empty / whitespace answers are a different classification.
        assert!(!is_code_only_answer(""));
        assert!(!is_code_only_answer("  \n  "));
    }

    /// Legacy `internal_answer_v1` envelope machinery tests: `modes/rag.yaml` is
    /// ProseOnly now (PR-A 2026-07-20 — worker final = handoff JSON). Force the
    /// historical contract so the envelope code paths stay under test.
    fn legacy_rag_mode() -> ModeConfig {
        let mut mode = super::super::config::load_mode_config("rag").unwrap();
        mode.synthesis_output.contract = AnswerContractKind::InternalAnswerV1;
        mode
    }

    #[test]
    fn parses_valid_rag_json() {
        let mode = legacy_rag_mode();
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
        let mode = legacy_rag_mode();
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
        assert!(
            fallback.contains('，')
                || fallback.contains('。')
                || fallback.chars().any(|c| c > '\u{4e00}')
        );
    }

    #[test]
    fn extract_partial_fallback_strips_invalid_citations() {
        let mode = legacy_rag_mode();
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
    fn unwrap_synthesis_json_envelope_extracts_answer_text() {
        let raw = r#"{
  "schema_version": "internal_answer_v1",
  "answer_text": "这篇报告与最佳实践的差距在于未提及 IaC。",
  "citations": [{"chunk_id": "e8018cfe"}],
  "coverage": "full",
  "refusal_reason": null
}"#;
        let text = unwrap_synthesis_json_envelope(raw).expect("unwrap");
        assert!(text.contains("差距"));
        assert!(!text.contains("schema_version"));
    }

    #[test]
    fn unwrap_mangled_keys_without_underscores() {
        let raw = r#"{"schemaversion":"internalanswerv1","answertext":"正文[[cite:a]]与网页[[1]]EVIDENCEINSUFFICIENTFALLBACK","citations":[{"chunkid":"a"}],"coverage":"partial","refusal_reason":null}"#;
        let text = unwrap_synthesis_json_envelope(raw).expect("unwrap mangled");
        assert!(text.contains("正文"));
        assert!(!text.contains("EVIDENCEINSUFFICIENTFALLBACK"));
        assert!(text.contains("[[web:1]]") || text.contains("[[1]]"));
        assert!(!text.contains("schemaversion"));
    }

    #[test]
    fn strip_model_source_wrappers_removes_laiyuan_shells() {
        let raw = "[来源： ]** --- ## 二、框架\n根据报告[[web:4]]：\n**[来源：[[web:4]] [[web:2]]]**\n正文";
        let cleaned = strip_model_source_wrappers(raw);
        assert!(!cleaned.contains("[来源"));
        assert!(cleaned.contains("[[web:4]]") || cleaned.contains("框架"));
        assert!(cleaned.contains("正文") || cleaned.contains("框架"));
    }

    #[test]
    fn ensure_user_visible_peels_full_unified_envelope() {
        let raw = r##"{ "schema_version": "internal_answer_unified_v1", "answer_text": "差距分析：正文[[web:1]]", "citations": [ {"kind": "web", "id": "1"} ], "coverage": "full", "refusal_reason": null }"##;
        let text = ensure_user_visible_answer_text(raw);
        assert!(text.contains("差距分析"));
        assert!(text.contains("[[web:1]]"));
        assert!(!text.contains("schema_version"));
        assert!(!text.trim_start().starts_with('{'));
    }

    #[test]
    fn lift_unified_does_not_keep_envelope_as_answer_text() {
        let mut mode = super::super::config::load_mode_config("rag").unwrap();
        mode.synthesis_output.contract = AnswerContractKind::InternalAnswerUnifiedV1;
        let raw = r#"{"schema_version":"internal_answer_unified_v1","answer_text":"仅正文[[web:2]]","citations":[{"kind":"web","id":"2"}],"coverage":"full","refusal_reason":null}"#;
        let lifted = lift_prose_to_contract(raw, &[], &[], &mode).expect("lift");
        let prose = render_synthesis_prose(&lifted);
        assert_eq!(prose.contains("schema_version"), false);
        assert!(prose.contains("仅正文"));
        assert!(prose.contains("[[web:2]]"));
    }

    #[test]
    fn parse_unified_contract_with_doc_and_web() {
        let mut mode = super::super::config::load_mode_config("rag").unwrap();
        mode.synthesis_output.contract = AnswerContractKind::InternalAnswerUnifiedV1;
        let raw = r#"{"schema_version":"internal_answer_unified_v1","answer_text":"文档点[[cite:c1]]与网页[[web:2]]","citations":[{"kind":"doc","id":"c1"},{"kind":"web","id":"2"}],"coverage":"full","refusal_reason":null}"#;
        let parsed = parse_synthesis_answer(raw, &mode).expect("parse unified");
        match parsed {
            ParsedSynthesisAnswer::Unified(u) => {
                assert_eq!(u.citations.len(), 2);
                assert!(u.answer_text.contains("[[web:2]]"));
            }
            _ => panic!("expected unified"),
        }
    }

    #[test]
    fn resolve_sanitizes_unknown_cites_instead_of_failing() {
        let mode = legacy_rag_mode();
        let tool_results = vec![contracts::ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({"chunks": [{"chunk_id": "good"}]})),
            trace: None,
        }];
        let raw = r#"{"schema_version":"internal_answer_v1","answer_text":"正文[[cite:good]]与未知[[cite:bad]]","citations":[{"chunk_id":"good"},{"chunk_id":"bad"}],"coverage":"full","refusal_reason":null}"#;
        let resolved =
            resolve_synthesis_answer(&[raw], &tool_results, &[], &mode).expect("should sanitize");
        let prose = render_synthesis_prose(&resolved);
        assert!(prose.contains("正文"));
        assert!(prose.contains("[[cite:good]]"));
        assert!(!prose.contains("[[cite:bad]]"));
        assert!(!prose.contains("schema_version"));
    }

    #[test]
    fn analytical_weiti_phrase_does_not_abort_partial_salvage() {
        let mode = legacy_rag_mode();
        let tool_results = vec![contracts::ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({"chunks": []})),
            trace: None,
        }];
        // 「未提及」used to false-positive as refusal and return None (leaking JSON upstream).
        let raw = r#"{"schema_version":"internal_answer_v1","answer_text":"报告采用了容器化，但未提及基础设施即代码（IaC）。","citations":[{"chunk_id":"missing"}],"coverage":"full","refusal_reason":null}"#;
        let partial = extract_partial_synthesis_fallback(&[raw], &tool_results, &[], &mode)
            .expect("should salvage analytical prose");
        assert!(partial.contains("未提及"));
        assert!(!partial.contains("schema_version"));
    }

    #[test]
    fn extract_partial_fallback_returns_insufficient_zh_when_text_empty_after_strip() {
        let mode = legacy_rag_mode();
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
        assert_eq!(partial, partial_evidence_insufficient_zh());
    }

    #[test]
    fn extract_partial_fallback_skips_when_draft_contains_refusal() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let raw = r#"{"schema_version":"internal_answer_v1","answer_text":"文档中未找到保修期限相关信息。","citations":[],"coverage":"none","refusal_reason":"not found"}"#;
        assert!(extract_partial_synthesis_fallback(&[raw], &[], &[], &mode).is_none());
    }

    #[test]
    fn extract_partial_fallback_prefers_latest_candidate() {
        let mode = legacy_rag_mode();
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
