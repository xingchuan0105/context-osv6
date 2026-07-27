//! Shared per-turn evidence store (V2 design §3.3).
//!
//! Append-only, monotonic `E{n}` ids, host-written (never by the LLM), so
//! evidence numbering is deterministic and citation ids are real by
//! construction. After workers finish, the chat exit receives the **full
//! bodies** of every citable store entry — the set already decided by the RAG
//! pipeline (dynamic rough-recall / rerank / final-feed) and worker tools.
//! Chat must not re-select or invent evidence (2026-07-20 product rule).
//!
//! **No second TOPK / body truncation here (2026-07-20):** volume is controlled
//! only by the retrieval pipeline (`dynamic_rough_recall` → rerank →
//! `dynamic_final_feed` ≈ clamp 10–30, plus worker `top_k`). This store only
//! **dedupes** by locator and assigns `E{n}`. Forcibly re-cutting by score or
//! char-capping `full_text` would split semantics after the pipeline already
//! chose whole chunks (~512 tokens at ingest).
//!
//! **Targeted entries (R2, 2026-07-18):** `doc_profile` / `doc_summary` output
//! is captured as [`EvidenceKind::DocProfile`] — orientation only, never citable.

use std::collections::HashMap;

use contracts::{ToolResult, ToolStatus};
use serde::{Deserialize, Serialize};

use super::types::Channel;

/// Short preview for logs / UI chips only — not used as the chat evidence body.
const MAX_PREVIEW_CHARS: usize = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    DocChunk,
    WebPage,
    /// Targeted doc orientation (doc_profile / doc_summary output): kept
    /// outside the TOPK gate, rendered in the chat brief's targeting section,
    /// never citable.
    DocProfile,
}

/// One evidence unit in the shared store.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceEntry {
    pub eid: String,
    pub channel: Channel,
    pub kind: EvidenceKind,
    /// Native locator: chunk_id for doc chunks (used to build product markers).
    pub chunk_id: Option<String>,
    pub doc_id: Option<String>,
    /// Real file name resolved from docscope metadata (what O1 dropped).
    pub doc_name: Option<String>,
    pub page: Option<usize>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub preview: String,
    pub full_text: String,
    pub score: Option<f64>,
}

/// One evidence unit as shown to the chat exit (and for answer-rule selection).
///
/// This is the pipeline-decided set: `full_text` is the intact chunk body Chat
/// must ground on (not a re-selectable preview-only stub).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceListing {
    pub eid: String,
    pub channel: Channel,
    pub kind: EvidenceKind,
    /// Human label: 《file》pN for doc chunks, `title (url)` for web pages.
    pub label: String,
    /// Short preview (logging / UI); chat prompt uses `full_text`.
    pub preview: String,
    /// Intact evidence body from the pipeline (no store-side char truncation).
    #[serde(default)]
    pub full_text: String,
    /// Native chunk_id when this is a doc chunk (for eval tool_results bridge).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// A source document in scope (identity for genre judgment + chip display).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceDoc {
    pub doc_id: String,
    pub file_name: String,
    pub genre: Option<String>,
}

#[derive(Debug, Default)]
pub struct EvidenceStore {
    entries: Vec<EvidenceEntry>,
    doc_names: HashMap<String, String>,
    source_docs: Vec<SourceDoc>,
}

impl EvidenceStore {
    /// Build from docscope metadata (filenames + genres), when available.
    pub fn from_docscope(meta: Option<&common::DocScopeMetadata>) -> Self {
        let mut doc_names = HashMap::new();
        let mut source_docs = Vec::new();
        if let Some(meta) = meta {
            for doc in &meta.documents {
                let name = if doc.filename.trim().is_empty() {
                    doc.docname.clone()
                } else {
                    doc.filename.clone()
                };
                if !name.is_empty() {
                    doc_names.insert(doc.doc_id.clone(), name.clone());
                }
                let genre = doc.genre.as_str();
                source_docs.push(SourceDoc {
                    doc_id: doc.doc_id.clone(),
                    file_name: name,
                    genre: (genre != "unknown").then(|| genre.to_string()),
                });
            }
        }
        Self {
            entries: Vec::new(),
            doc_names,
            source_docs,
        }
    }

    pub fn entries(&self) -> &[EvidenceEntry] {
        &self.entries
    }

    pub fn source_docs(&self) -> &[SourceDoc] {
        &self.source_docs
    }

    pub fn get(&self, eid: &str) -> Option<&EvidenceEntry> {
        self.entries.iter().find(|e| e.eid == eid)
    }

    pub fn count_channel(&self, channel: Channel) -> usize {
        self.entries.iter().filter(|e| e.channel == channel).count()
    }

    /// Normalize raw worker tool results into store entries. Returns the net
    /// number of **new** entries (0 ⇒ channel pack status `empty`).
    ///
    /// Only **dedupes** by locator (chunk_id / url). Does **not** re-apply TOPK
    /// or truncate body text — the RAG pipeline already sized the result set.
    pub fn insert_from_tool_results(&mut self, channel: Channel, results: &[ToolResult]) -> usize {
        let before = self.entries.len();
        for tr in results {
            if tr.status != ToolStatus::Ok {
                continue;
            }
            let Some(data) = tr.data.as_ref() else {
                continue;
            };
            match channel {
                Channel::Rag => match tr.tool.as_str() {
                    // Targeted doc-orientation tools return per-doc objects
                    // without chunk_id — capture them as DocProfile entries.
                    "doc_profile" | "doc_summary" => self.insert_targeted(data),
                    _ => self.insert_rag(data),
                },
                Channel::Search => self.insert_search(data),
            }
        }
        self.entries.len() - before
    }

    fn locator_exists(&self, channel: Channel, locator: &str) -> bool {
        self.entries.iter().any(|e| {
            e.channel == channel
                && match e.kind {
                    EvidenceKind::DocChunk => e.chunk_id.as_deref() == Some(locator),
                    EvidenceKind::WebPage => e.url.as_deref() == Some(locator),
                    // Targeted entries dedupe in `insert_targeted` (per doc + text).
                    EvidenceKind::DocProfile => false,
                }
        })
    }

    fn push_entry(&mut self, mut entry: EvidenceEntry) {
        entry.eid = format!("E{}", self.entries.len() + 1);
        self.entries.push(entry);
    }

    /// Capture doc_profile / doc_summary output (top-level per-doc array, no
    /// chunk_id) as targeted orientation entries. One entry per (doc, text):
    /// repeat calls with identical output dedupe.
    fn insert_targeted(&mut self, data: &serde_json::Value) {
        let Some(list) = data.as_array() else {
            return;
        };
        for item in list {
            let doc_id = item
                .get("doc_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if doc_id.is_empty() {
                continue;
            }
            let full_text = targeted_text(item);
            if full_text.is_empty()
                || self.entries.iter().any(|e| {
                    e.kind == EvidenceKind::DocProfile
                        && e.doc_id.as_deref() == Some(doc_id)
                        && e.full_text == full_text
                })
            {
                continue;
            }
            let doc_name = self
                .doc_names
                .get(doc_id)
                .cloned()
                .or_else(|| {
                    item.get("name")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                });
            self.push_entry(EvidenceEntry {
                eid: String::new(),
                channel: Channel::Rag,
                kind: EvidenceKind::DocProfile,
                chunk_id: None,
                doc_id: Some(doc_id.to_string()),
                doc_name,
                page: None,
                url: None,
                title: None,
                preview: preview_of(&full_text),
                full_text,
                score: None,
            });
        }
    }

    /// Targeted doc-orientation entries (doc_profile / doc_summary) — for the
    /// chat brief's targeting section; never citable.
    pub fn targeted_entries(&self) -> Vec<EvidenceEntry> {
        self.entries
            .iter()
            .filter(|e| e.kind == EvidenceKind::DocProfile)
            .cloned()
            .collect()
    }

    fn insert_rag(&mut self, data: &serde_json::Value) {
        // dense_retrieval: top-level array of chunk objects; tolerate
        // {chunks|results|items} wrappers (see workers.rs O1 probes).
        let arrays = [
            data.as_array().map(|_| data),
            data.get("chunks"),
            data.get("results"),
            data.get("items"),
        ];
        for arr in arrays.into_iter().flatten() {
            let Some(list) = arr.as_array() else {
                continue;
            };
            for c in list {
                let chunk_id = c
                    .get("chunk_id")
                    .or_else(|| c.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if chunk_id.is_empty() || self.locator_exists(Channel::Rag, chunk_id) {
                    continue;
                }
                let text = c
                    .get("text")
                    .or_else(|| c.get("content"))
                    .or_else(|| c.get("snippet"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let doc_id = c
                    .get("doc_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let doc_name = doc_id
                    .as_ref()
                    .and_then(|id| self.doc_names.get(id))
                    .cloned();
                let page = c
                    .get("page")
                    .and_then(|v| v.as_u64())
                    .map(|p| p as usize);
                self.push_entry(EvidenceEntry {
                    eid: String::new(),
                    channel: Channel::Rag,
                    kind: EvidenceKind::DocChunk,
                    chunk_id: Some(chunk_id.to_string()),
                    doc_id,
                    doc_name,
                    page,
                    url: None,
                    title: c
                        .get("title")
                        .or_else(|| c.get("filename"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    preview: preview_of(text),
                    full_text: text.to_string(),
                    score: c.get("score").and_then(|v| v.as_f64()),
                });
            }
        }
    }

    fn insert_search(&mut self, data: &serde_json::Value) {
        let Some(list) = data
            .get("results")
            .or_else(|| data.get("items"))
            .and_then(|v| v.as_array())
        else {
            return;
        };
        for r in list {
            let url = r
                .get("url")
                .or_else(|| r.get("link"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if url.is_empty() || self.locator_exists(Channel::Search, url) {
                continue;
            }
            let text = r
                .get("snippet")
                .or_else(|| r.get("content"))
                .or_else(|| r.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            self.push_entry(EvidenceEntry {
                eid: String::new(),
                channel: Channel::Search,
                kind: EvidenceKind::WebPage,
                chunk_id: None,
                doc_id: None,
                doc_name: None,
                page: None,
                url: Some(url.to_string()),
                title: r
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                preview: preview_of(text),
                full_text: text.to_string(),
                score: None,
            });
        }
    }

    /// Listings for chat exit / answer-rule selection (includes full bodies).
    pub fn listings(&self) -> Vec<EvidenceListing> {
        self.entries
            .iter()
            .map(|e| {
                let label = match e.kind {
                    EvidenceKind::DocChunk => {
                        let name = e
                            .doc_name
                            .as_deref()
                            .or(e.doc_id.as_deref())
                            .unwrap_or("document");
                        match e.page {
                            Some(p) => format!("《{name}》p{p}"),
                            None => format!("《{name}》"),
                        }
                    }
                    EvidenceKind::WebPage => {
                        let title = e.title.as_deref().unwrap_or("untitled");
                        let url = e.url.as_deref().unwrap_or("");
                        format!("{title} ({url})")
                    }
                    EvidenceKind::DocProfile => {
                        let name = e
                            .doc_name
                            .as_deref()
                            .or(e.doc_id.as_deref())
                            .unwrap_or("document");
                        format!("《{name}》(文档定向)")
                    }
                };
                let full_text = if e.full_text.trim().is_empty() {
                    e.preview.clone()
                } else {
                    e.full_text.clone()
                };
                EvidenceListing {
                    eid: e.eid.clone(),
                    channel: e.channel,
                    kind: e.kind,
                    label,
                    preview: e.preview.clone(),
                    full_text,
                    chunk_id: e.chunk_id.clone(),
                    doc_id: e.doc_id.clone(),
                    score: e.score,
                    url: e.url.clone(),
                }
            })
            .collect()
    }

    /// Citable doc chunks as a single `dense_retrieval` tool result for eval /
    /// clients that still score retrieval from `ChatResponse.tool_results`.
    pub fn as_retrieval_tool_results(&self) -> Vec<ToolResult> {
        let mut out = Vec::new();
        let rag: Vec<serde_json::Value> = self
            .entries
            .iter()
            .filter(|e| e.kind == EvidenceKind::DocChunk)
            .filter_map(|e| {
                let chunk_id = e.chunk_id.as_deref().filter(|s| !s.is_empty())?;
                let text = if e.full_text.trim().is_empty() {
                    e.preview.as_str()
                } else {
                    e.full_text.as_str()
                };
                Some(serde_json::json!({
                    "chunk_id": chunk_id,
                    "text": text,
                    "content": text,
                    "doc_id": e.doc_id,
                    "score": e.score,
                    "page": e.page,
                    "eid": e.eid,
                }))
            })
            .collect();
        if !rag.is_empty() {
            out.push(ToolResult {
                tool: "dense_retrieval".into(),
                version: "1.0".into(),
                status: ToolStatus::Ok,
                data: Some(serde_json::Value::Array(rag)),
                trace: None,
            });
        }
        let web: Vec<serde_json::Value> = self
            .entries
            .iter()
            .filter(|e| e.kind == EvidenceKind::WebPage)
            .filter_map(|e| {
                let url = e.url.as_deref().filter(|s| !s.is_empty())?;
                let text = if e.full_text.trim().is_empty() {
                    e.preview.as_str()
                } else {
                    e.full_text.as_str()
                };
                Some(serde_json::json!({
                    "url": url,
                    "title": e.title,
                    "snippet": text,
                    "content": text,
                    "eid": e.eid,
                }))
            })
            .collect();
        if !web.is_empty() {
            // Not in RETRIEVAL_TOOLS for golden recall@k (doc-centric); still
            // useful for clients/debugging. Eval recall uses dense_retrieval.
            out.push(ToolResult {
                tool: "web_search".into(),
                version: "1.0".into(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!({ "results": web })),
                trace: None,
            });
        }
        // C9: DocProfile orientation entries also bridge into tool_results so
        // the evidence trail is visible downstream (metadata answers looked
        // "evidence-less" — q132). Kept OUT of dense_retrieval (not retrieval
        // chunks) and named for its real tool; payload mirrors the doc_profile
        // tool's per-doc array shape (doc_id / name / summary). Citation and
        // selection consumers ignore non-chunk tools (eval RETRIEVAL_TOOLS is
        // deliberately untouched).
        let profiles: Vec<serde_json::Value> = self
            .entries
            .iter()
            .filter(|e| e.kind == EvidenceKind::DocProfile)
            .map(|e| {
                serde_json::json!({
                    "doc_id": e.doc_id,
                    "name": e.doc_name,
                    "summary": e.full_text,
                    "eid": e.eid,
                })
            })
            .collect();
        if !profiles.is_empty() {
            out.push(ToolResult {
                tool: "doc_profile".into(),
                version: "1.0".into(),
                status: ToolStatus::Ok,
                data: Some(serde_json::Value::Array(profiles)),
                trace: None,
            });
        }
        out
    }
}

fn preview_of(text: &str) -> String {
    cap_chars(text.trim(), MAX_PREVIEW_CHARS)
}

/// Render one doc_profile / doc_summary element as targeted orientation text:
/// profile → `genre: X\nsections: t1 (p1), t2 (p5)…`; summary → the summary.
fn targeted_text(item: &serde_json::Value) -> String {
    if let Some(summary) = item.get("summary").and_then(|v| v.as_str()) {
        return summary.trim().to_string();
    }
    let genre = item
        .get("genre")
        .and_then(|v| v.as_str())
        .filter(|g| *g != "unknown")
        .unwrap_or("");
    let sections = item
        .get("sections")
        .and_then(|v| v.as_array())
        .map(|ss| {
            ss.iter()
                .filter_map(|s| {
                    let title = s.get("title").and_then(|v| v.as_str())?;
                    Some(match s.get("page").and_then(|v| v.as_u64()) {
                        Some(p) => format!("{title} (p{p})"),
                        None => title.to_string(),
                    })
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    match (genre.is_empty(), sections.is_empty()) {
        (true, true) => String::new(),
        (false, true) => format!("genre: {genre}"),
        (true, false) => format!("sections: {sections}"),
        (false, false) => format!("genre: {genre}\nsections: {sections}"),
    }
}

fn cap_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut s: String = text.chars().take(max).collect();
    s.push('…');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn docscope() -> common::DocScopeMetadata {
        serde_json::from_value(serde_json::json!({
            "documents": [{
                "doc_id": "d1",
                "filename": "数字化转型IT立项报告.docx",
                "docname": "数字化转型IT立项报告",
                "language": "zh",
                "domain": "technology",
                "genre": "report",
                "era": "contemporary"
            }],
            "profile": { "languages": ["zh"], "domains": [], "genres": [], "eras": [] }
        }))
        .expect("docscope")
    }

    #[test]
    fn rag_entries_carry_doc_identity() {
        let mut store = EvidenceStore::from_docscope(Some(&docscope()));
        let tr = ToolResult {
            tool: "dense_retrieval".into(),
            version: "1".into(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!([
                {"chunk_id": "c1", "doc_id": "d1", "text": "现状诊断内容", "score": 0.9, "page": 5}
            ])),
            trace: None,
        };
        assert_eq!(store.insert_from_tool_results(Channel::Rag, &[tr]), 1);
        let e = &store.entries()[0];
        assert_eq!(e.eid, "E1");
        assert_eq!(e.doc_name.as_deref(), Some("数字化转型IT立项报告.docx"));
        assert_eq!(e.page, Some(5));
        let listings = store.listings();
        assert!(listings[0].label.contains("数字化转型IT立项报告"));
        assert!(listings[0].label.contains("p5"));
    }

    #[test]
    fn search_entries_monotonic_eids() {
        let mut store = EvidenceStore::default();
        let tr = ToolResult {
            tool: "web_search".into(),
            version: "1".into(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({
                "results": [
                    {"url": "https://a.example", "title": "A", "snippet": "sa"},
                    {"url": "https://b.example", "title": "B", "snippet": "sb"}
                ]
            })),
            trace: None,
        };
        assert_eq!(store.insert_from_tool_results(Channel::Search, &[tr]), 2);
        assert_eq!(store.entries()[0].eid, "E1");
        assert_eq!(store.entries()[1].eid, "E2");
        assert_eq!(store.count_channel(Channel::Search), 2);
        assert_eq!(store.count_channel(Channel::Rag), 0);
    }

    #[test]
    fn source_docs_genre_unknown_filtered() {
        let store = EvidenceStore::from_docscope(Some(&docscope()));
        assert_eq!(store.source_docs()[0].genre.as_deref(), Some("report"));
    }

    fn rag_results(n: usize, score_of: impl Fn(usize) -> f64) -> Vec<ToolResult> {
        let chunks: Vec<serde_json::Value> = (0..n)
            .map(|i| {
                serde_json::json!({
                    "chunk_id": format!("c{i}"), "doc_id": "d1",
                    "text": format!("chunk {i}"), "score": score_of(i),
                })
            })
            .collect();
        vec![ToolResult {
            tool: "dense_retrieval".into(),
            version: "1".into(),
            status: ToolStatus::Ok,
            data: Some(serde_json::Value::Array(chunks)),
            trace: None,
        }]
    }

    #[test]
    fn store_keeps_all_pipeline_results_without_second_topk() {
        // Volume control belongs in dense/rerank (dynamic budgets), not here.
        let mut store = EvidenceStore::default();
        store.insert_from_tool_results(Channel::Rag, &rag_results(30, |i| i as f64));
        assert_eq!(store.count_channel(Channel::Rag), 30);
        assert!(store.entries().iter().any(|e| e.chunk_id.as_deref() == Some("c0")));
        assert!(store.entries().iter().any(|e| e.chunk_id.as_deref() == Some("c29")));
    }

    #[test]
    fn full_text_not_char_truncated() {
        let long = "字".repeat(5000);
        let mut store = EvidenceStore::default();
        let tr = ToolResult {
            tool: "dense_retrieval".into(),
            version: "1".into(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!([{
                "chunk_id": "c-long", "doc_id": "d1", "text": long, "score": 1.0
            }])),
            trace: None,
        };
        store.insert_from_tool_results(Channel::Rag, &[tr]);
        let e = store.entries().iter().find(|e| e.chunk_id.as_deref() == Some("c-long")).unwrap();
        assert_eq!(e.full_text.chars().count(), 5000);
        assert_eq!(e.preview.chars().count(), MAX_PREVIEW_CHARS + 1); // + ellipsis
    }

    #[test]
    fn duplicate_locators_not_reinserted() {
        let mut store = EvidenceStore::default();
        assert_eq!(store.insert_from_tool_results(Channel::Rag, &rag_results(3, |_| 0.5)), 3);
        // Same chunks again (e.g. re-dispatch): nothing new added.
        assert_eq!(store.insert_from_tool_results(Channel::Rag, &rag_results(3, |_| 0.5)), 0);
        assert_eq!(store.count_channel(Channel::Rag), 3);
    }

    #[test]
    fn search_channel_dedupes_without_hard_cap() {
        let mut store = EvidenceStore::default();
        let results: Vec<serde_json::Value> = (0..20)
            .map(|i| serde_json::json!({"url": format!("https://{i}.example"), "title": "T", "snippet": "s"}))
            .collect();
        let tr = ToolResult {
            tool: "web_search".into(),
            version: "1".into(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({"results": results})),
            trace: None,
        };
        store.insert_from_tool_results(Channel::Search, std::slice::from_ref(&tr));
        assert_eq!(store.count_channel(Channel::Search), 20);
        // Second call with same URLs is fully deduped.
        assert_eq!(store.insert_from_tool_results(Channel::Search, &[tr]), 0);
    }

    #[test]
    fn pipeline_sized_result_set_is_accepted_intact() {
        // Flood control is the retrieval tool's job (dynamic final feed / top_k),
        // not a second score cut in the shared store.
        let mut store = EvidenceStore::default();
        store.insert_from_tool_results(Channel::Rag, &rag_results(148, |_| 0.0));
        assert_eq!(store.count_channel(Channel::Rag), 148);
    }

    fn targeted_result(tool: &str, data: serde_json::Value) -> ToolResult {
        ToolResult {
            tool: tool.into(),
            version: "1".into(),
            status: ToolStatus::Ok,
            data: Some(data),
            trace: None,
        }
    }

    #[test]
    fn doc_profile_and_summary_become_targeted_entries() {
        let mut store = EvidenceStore::from_docscope(Some(&docscope()));
        let profile = targeted_result(
            "doc_profile",
            serde_json::json!([{
                "doc_id": "d1", "genre": "report",
                "sections": [
                    {"title": "现状诊断", "page": 3, "chunk_id": "c9"},
                    {"title": "基础设施选型", "page": 12, "chunk_id": "c40"}
                ]
            }]),
        );
        assert_eq!(store.insert_from_tool_results(Channel::Rag, &[profile]), 1);
        let summary = targeted_result(
            "doc_summary",
            serde_json::json!([{ "doc_id": "d1", "level": "doc", "summary": "本报告论证数字化转型立项必要性。" }]),
        );
        assert_eq!(store.insert_from_tool_results(Channel::Rag, &[summary]), 1);

        let targeted = store.targeted_entries();
        assert_eq!(targeted.len(), 2);
        assert_eq!(targeted[0].kind, EvidenceKind::DocProfile);
        assert_eq!(targeted[0].doc_name.as_deref(), Some("数字化转型IT立项报告.docx"));
        assert!(targeted[0].full_text.contains("genre: report"), "{}", targeted[0].full_text);
        assert!(targeted[0].full_text.contains("基础设施选型 (p12)"), "{}", targeted[0].full_text);
        assert_eq!(targeted[1].full_text, "本报告论证数字化转型立项必要性。");
        // Targeted entries appear in listings (orchestrator visibility) with kind.
        let listings = store.listings();
        assert!(listings.iter().all(|l| l.kind == EvidenceKind::DocProfile));
    }

    #[test]
    fn targeted_entries_dedupe_and_coexist_with_doc_chunks() {
        let mut store = EvidenceStore::default();
        let profile = || {
            targeted_result(
                "doc_profile",
                serde_json::json!([{ "doc_id": "d1", "genre": "report", "sections": [{"title": "t", "page": 1}] }]),
            )
        };
        assert_eq!(store.insert_from_tool_results(Channel::Rag, &[profile()]), 1);
        // Identical repeat call dedupes.
        assert_eq!(store.insert_from_tool_results(Channel::Rag, &[profile()]), 0);
        store.insert_from_tool_results(Channel::Rag, &rag_results(40, |_| 0.0));
        assert_eq!(store.targeted_entries().len(), 1);
        assert_eq!(
            store.entries().iter().filter(|e| e.kind == EvidenceKind::DocChunk).count(),
            40
        );
    }

    #[test]
    fn doc_profile_entries_bridge_into_tool_results() {
        // C9: DocProfile orientation evidence must appear in
        // as_retrieval_tool_results as a `doc_profile` result (real tool name,
        // per-doc array shape) — metadata answers no longer look evidence-less
        // downstream — while DocChunk bridging is unchanged.
        let mut store = EvidenceStore::from_docscope(Some(&docscope()));
        let profile = targeted_result(
            "doc_profile",
            serde_json::json!([{ "doc_id": "d1", "genre": "report", "sections": [{"title": "t", "page": 1}] }]),
        );
        assert_eq!(store.insert_from_tool_results(Channel::Rag, &[profile]), 1);
        store.insert_from_tool_results(Channel::Rag, &rag_results(2, |_| 0.0));

        let bridged = store.as_retrieval_tool_results();
        let tools: Vec<&str> = bridged.iter().map(|t| t.tool.as_str()).collect();
        assert!(tools.contains(&"dense_retrieval"), "{tools:?}");
        assert!(tools.contains(&"doc_profile"), "{tools:?}");

        let profile_result = bridged
            .iter()
            .find(|t| t.tool == "doc_profile")
            .expect("doc_profile result");
        assert_eq!(profile_result.status, ToolStatus::Ok);
        let arr = profile_result
            .data
            .as_ref()
            .and_then(|d| d.as_array())
            .expect("per-doc array");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["doc_id"], "d1");
        assert_eq!(arr[0]["name"], "数字化转型IT立项报告.docx");
        assert!(arr[0]["summary"].as_str().unwrap().contains("genre: report"));

        // DocChunk bridging unchanged: dense_retrieval still carries chunks.
        let dense = bridged
            .iter()
            .find(|t| t.tool == "dense_retrieval")
            .expect("dense_retrieval result");
        assert_eq!(dense.data.as_ref().and_then(|d| d.as_array()).unwrap().len(), 2);
    }
}
