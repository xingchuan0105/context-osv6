//! Cross-mode agent work-progress disclosure (WorkFact → Activity).
//!
//! Product rules: observed-only facts; never surface codegen / internal tool ids.
//! See `docs/engineering/AGENT_PROGRESS_DISCLOSURE_DESIGN_2026-07-13.md`.

mod labels;
mod snapshot;

use std::collections::BTreeMap;

use contracts::chat::ChatActivitySourcePreview;

use crate::events::{AgentEvent, AgentEventSink};

pub use labels::{product_action_for_bridge_method, product_action_for_native_tool};
pub use snapshot::assistant_progress_turn_metadata;

/// Stable progress phase (cross-mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressPhase {
    Accept,
    Act,
    Reason,
    Compose,
    Done,
}

impl ProgressPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Act => "act",
            Self::Reason => "reason",
            Self::Compose => "compose",
            Self::Done => "done",
        }
    }
}

/// Action kind for product labeling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressKind {
    Understand,
    RetrieveSemantic,
    RetrieveKeyword,
    RetrieveGraph,
    RetrieveDoc,
    SearchWeb,
    FetchUrl,
    Memory,
    WriteResearch,
    WriteOutline,
    WriteDraft,
    WriteRefine,
    WriteValidate,
    ComposeAnswer,
    ReasonPreview,
}

impl ProgressKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Understand => "understand",
            Self::RetrieveSemantic => "retrieve_semantic",
            Self::RetrieveKeyword => "retrieve_keyword",
            Self::RetrieveGraph => "retrieve_graph",
            Self::RetrieveDoc => "retrieve_doc",
            Self::SearchWeb => "search_web",
            Self::FetchUrl => "fetch_url",
            Self::Memory => "memory",
            Self::WriteResearch => "write_research",
            Self::WriteOutline => "write_outline",
            Self::WriteDraft => "write_draft",
            Self::WriteRefine => "write_refine",
            Self::WriteValidate => "write_validate",
            Self::ComposeAnswer => "compose_answer",
            Self::ReasonPreview => "reason_preview",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressStatus {
    Started,
    Succeeded,
    Failed,
}

/// Observed work fact → user-facing Activity.
#[derive(Debug, Clone)]
pub struct WorkFact {
    pub phase: ProgressPhase,
    pub kind: ProgressKind,
    pub title: String,
    pub detail: Option<String>,
    pub hits: Option<usize>,
    pub previews: Vec<ChatActivitySourcePreview>,
    pub status: ProgressStatus,
}

impl WorkFact {
    /// i18n key for frontend (`progress.*`). Query/hits stay in detail/counts.
    fn message_key(kind: ProgressKind, variant: &str) -> String {
        if variant.is_empty() {
            format!("progress.{}", kind.as_str())
        } else {
            format!("progress.{}.{}", kind.as_str(), variant)
        }
    }

    pub fn understand(query_preview: &str) -> Self {
        let preview = truncate_chars(query_preview.trim(), 40);
        Self {
            phase: ProgressPhase::Accept,
            kind: ProgressKind::Understand,
            title: Self::message_key(ProgressKind::Understand, ""),
            // Raw query fragment for frontend to wrap per-locale (no Chinese quotes).
            detail: if preview.is_empty() {
                None
            } else {
                Some(preview)
            },
            hits: None,
            previews: Vec::new(),
            status: ProgressStatus::Started,
        }
    }

    pub fn compose_answer() -> Self {
        Self {
            phase: ProgressPhase::Compose,
            kind: ProgressKind::ComposeAnswer,
            title: Self::message_key(ProgressKind::ComposeAnswer, ""),
            detail: None,
            hits: None,
            previews: Vec::new(),
            status: ProgressStatus::Started,
        }
    }

    pub fn retrieval_started(kind: ProgressKind, _product_action: &str, query: &str) -> Self {
        let q = truncate_chars(query.trim(), 48);
        Self {
            phase: ProgressPhase::Act,
            kind,
            title: Self::message_key(kind, "running"),
            detail: if q.is_empty() { None } else { Some(q) },
            hits: None,
            previews: Vec::new(),
            status: ProgressStatus::Started,
        }
    }

    pub fn retrieval_finished(
        kind: ProgressKind,
        _product_action: &str,
        query: &str,
        hits: usize,
        doc_labels: &[String],
    ) -> Self {
        let q = truncate_chars(query.trim(), 48);
        let variant = if hits == 0 { "empty" } else { "done" };
        let previews = doc_labels
            .iter()
            .take(3)
            .enumerate()
            .map(|(i, label)| ChatActivitySourcePreview {
                id: format!("doc-{i}"),
                label: truncate_chars(label, 28),
                href: None,
            })
            .collect();
        Self {
            phase: ProgressPhase::Act,
            kind,
            title: Self::message_key(kind, variant),
            // Raw query only; hits go in counts for frontend i18n.
            detail: if q.is_empty() { None } else { Some(q) },
            hits: if hits > 0 { Some(hits) } else { None },
            previews,
            status: ProgressStatus::Succeeded,
        }
    }

    pub fn write_stage(kind: ProgressKind, _title: impl Into<String>, detail: Option<String>) -> Self {
        Self {
            phase: ProgressPhase::Act,
            kind,
            title: Self::message_key(kind, ""),
            detail,
            hits: None,
            previews: Vec::new(),
            status: ProgressStatus::Started,
        }
    }

    pub fn memory_context() -> Self {
        Self {
            phase: ProgressPhase::Act,
            kind: ProgressKind::Memory,
            title: Self::message_key(ProgressKind::Memory, ""),
            detail: None,
            hits: None,
            previews: Vec::new(),
            status: ProgressStatus::Started,
        }
    }

    fn into_activity(self) -> AgentEvent {
        let mut counts = BTreeMap::new();
        if let Some(hits) = self.hits.filter(|h| *h > 0) {
            counts.insert("hits".to_string(), hits);
        }
        // Encode kind in stage for frontend: "act:retrieve_semantic"
        let stage = format!("{}:{}", self.phase.as_str(), self.kind.as_str());
        AgentEvent::Activity {
            stage,
            // Stable i18n key (progress.*) — frontend maps by locale.
            message: self.title,
            detail: self.detail,
            counts,
            sources_preview: self.previews,
        }
    }
}

pub async fn emit_work_fact(sink: &dyn AgentEventSink, fact: WorkFact) {
    let _ = sink.emit(fact.into_activity()).await;
}

pub fn truncate_chars(s: &str, max: usize) -> String {
    let mut it = s.chars();
    let head: String = it.by_ref().take(max).collect();
    if it.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

/// Map bridge SDK method → kind + product label.
pub fn bridge_method_progress(
    method: &str,
) -> Option<(ProgressKind, &'static str)> {
    match method {
        "dense_search" => Some((ProgressKind::RetrieveSemantic, "语义检索")),
        "lexical_search" => Some((ProgressKind::RetrieveKeyword, "关键词检索")),
        "graph_search" => Some((ProgressKind::RetrieveGraph, "关系检索")),
        "doc_summary" => Some((ProgressKind::RetrieveDoc, "阅读文档摘要")),
        "doc_profile" => Some((ProgressKind::RetrieveDoc, "查看文档结构")),
        "doc_chunks" => Some((ProgressKind::RetrieveDoc, "通读文档片段")),
        "chunk_fetch" => Some((ProgressKind::RetrieveDoc, "展开原文片段")),
        _ => None,
    }
}

pub fn native_tool_progress(tool: &str) -> Option<(ProgressKind, &'static str)> {
    match tool {
        "web_search" => Some((ProgressKind::SearchWeb, "网页搜索")),
        "web_fetch" => Some((ProgressKind::FetchUrl, "读取网页")),
        "conversation_history_load" | "user_profile_load" => {
            Some((ProgressKind::Memory, "回忆相关上下文"))
        }
        _ => None,
    }
}

/// Extract human query string from native tool args JSON.
pub fn query_from_tool_args(tool: &str, args: &serde_json::Value) -> String {
    match tool {
        "web_search" => args
            .get("query")
            .and_then(|v| v.as_str())
            .or_else(|| args.get("q").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string(),
        "web_fetch" => args
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        _ => args
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    }
}

/// Count chunks / results in a tool result payload.
pub fn hits_from_tool_data(data: Option<&serde_json::Value>) -> usize {
    let Some(data) = data else {
        return 0;
    };
    if let Some(chunks) = data.get("chunks").and_then(|c| c.as_array()) {
        return chunks.len();
    }
    if let Some(results) = data.get("results").and_then(|c| c.as_array()) {
        return results.len();
    }
    if let Some(n) = data.get("count").and_then(|c| c.as_u64()) {
        return n as usize;
    }
    0
}

/// Doc short labels from retrieval chunks (RAG). No URLs/domains for Search.
pub fn doc_labels_from_tool_data(data: Option<&serde_json::Value>) -> Vec<String> {
    let Some(data) = data else {
        return Vec::new();
    };
    let Some(chunks) = data.get("chunks").and_then(|c| c.as_array()) else {
        return Vec::new();
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for chunk in chunks {
        let label = chunk
            .get("doc_name")
            .and_then(|v| v.as_str())
            .or_else(|| chunk.get("title").and_then(|v| v.as_str()))
            .or_else(|| chunk.get("file_name").and_then(|v| v.as_str()))
            .unwrap_or("");
        if label.is_empty() {
            continue;
        }
        if seen.insert(label.to_string()) {
            out.push(label.to_string());
        }
        if out.len() >= 3 {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_hits_emits_i18n_key_and_raw_query() {
        let fact = WorkFact::retrieval_finished(
            ProgressKind::RetrieveSemantic,
            "semantic",
            "数字化转型 观点",
            0,
            &[],
        );
        let AgentEvent::Activity {
            message,
            detail,
            counts,
            stage,
            ..
        } = fact.into_activity()
        else {
            panic!("expected activity");
        };
        assert_eq!(message, "progress.retrieve_semantic.empty");
        assert_eq!(stage, "act:retrieve_semantic");
        assert_eq!(detail.as_deref(), Some("数字化转型 观点"));
        assert!(counts.is_empty());
    }

    #[test]
    fn positive_hits_include_count_and_done_key() {
        let fact = WorkFact::retrieval_finished(
            ProgressKind::RetrieveSemantic,
            "semantic",
            "q",
            12,
            &["立项报告.docx".into()],
        );
        let AgentEvent::Activity {
            message,
            counts,
            sources_preview,
            ..
        } = fact.into_activity()
        else {
            panic!("expected activity");
        };
        assert_eq!(message, "progress.retrieve_semantic.done");
        assert_eq!(counts.get("hits"), Some(&12));
        assert_eq!(sources_preview.len(), 1);
    }
}
