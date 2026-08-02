//! Synthesis answer parsing: JSON contract structs + normalize / upgrade /
//! marker-extraction machinery. Split out of `answer_contract/mod.rs` (C5-S4).

use avrag_llm::ChatMessage;
use contracts::ToolResult;
use serde::{Deserialize, Serialize};

use super::super::config::{AnswerContractKind, ModeConfig};

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
pub fn parse_synthesis_answer(
    raw: &str,
    mode: &ModeConfig,
) -> Result<ParsedSynthesisAnswer, String> {
    let body = normalize_synthesis_json_text(&super::strip_json_fences(raw));
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
pub(crate) fn normalize_synthesis_json_text(body: &str) -> String {
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

pub(crate) fn normalize_schema_version_value(s: &str) -> String {
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

pub(crate) fn parse_unified_or_legacy(body: &str) -> Result<ParsedSynthesisAnswer, String> {
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

pub(crate) fn upgrade_rag_json_to_unified_from_text(
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

/// Chunk-reference ids from `[[cite:…]]` **and** `[[image:…]]` (decision 3 —
/// inline-image references must not be dropped), first-seen order, deduped.
/// Delegates to the single rag-core grammar.
fn extract_cite_chunk_ids(text: &str) -> Vec<String> {
    avrag_rag_core::runtime::markers::extract_chunk_ids(text)
}

/// Public for citation filtering (`[[web:n]]` + legacy bare `[[n]]`).
pub fn extract_web_marker_indices_public(text: &str) -> Vec<u32> {
    extract_web_marker_indices(text)
}

/// Web-marker indices from `[[web:n]]` and legacy bare `[[n]]` (web-first,
/// then bare — see `markers::extract_web_indices`). Delegates to the single
/// rag-core grammar.
fn extract_web_marker_indices(text: &str) -> Vec<u32> {
    avrag_rag_core::runtime::markers::extract_web_indices(text)
}

/// Rewrite legacy bare `[[n]]` web markers to `[[web:n]]` (leave [[cite:]] alone).
pub(crate) fn rewrite_legacy_web_markers(text: &str) -> String {
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
pub(crate) fn scrub_internal_answer_tokens(text: &str) -> String {
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
