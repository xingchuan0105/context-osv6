//! Shared per-turn evidence store (V2 design §3.3).
//!
//! Append-only, monotonic `E{n}` ids, host-written (never by the LLM), so
//! evidence numbering is deterministic and citation ids are real by
//! construction. Agents receive lightweight listings (id + identity + preview)
//! in prompts; full text travels only inside the store / on explicit fetch.

use std::collections::HashMap;

use contracts::{ToolResult, ToolStatus};
use serde::{Deserialize, Serialize};

use super::types::Channel;

const MAX_PREVIEW_CHARS: usize = 300;
const MAX_FULL_TEXT_CHARS: usize = 4000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    DocChunk,
    WebPage,
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

/// Stub listing shown to agents in place of raw text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceListing {
    pub eid: String,
    pub channel: Channel,
    /// Human label: 《file》pN for doc chunks, `title (url)` for web pages.
    pub label: String,
    pub preview: String,
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

    /// Normalize raw worker tool results into store entries. Returns how many
    /// entries were inserted (0 ⇒ channel pack status `empty`).
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
                Channel::Rag => self.insert_rag(data),
                Channel::Search => self.insert_search(data),
            }
        }
        self.entries.len() - before
    }

    fn push_entry(&mut self, mut entry: EvidenceEntry) {
        entry.eid = format!("E{}", self.entries.len() + 1);
        self.entries.push(entry);
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
                if chunk_id.is_empty() {
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
                    full_text: cap_chars(text, MAX_FULL_TEXT_CHARS),
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
            if url.is_empty() {
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
                full_text: cap_chars(text, MAX_FULL_TEXT_CHARS),
                score: None,
            });
        }
    }

    /// Stubs for prompts: `[E3] 《file》p5 | preview…`
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
                };
                EvidenceListing {
                    eid: e.eid.clone(),
                    channel: e.channel,
                    label,
                    preview: e.preview.clone(),
                }
            })
            .collect()
    }
}

fn preview_of(text: &str) -> String {
    cap_chars(text.trim(), MAX_PREVIEW_CHARS)
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
}
