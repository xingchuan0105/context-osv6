//! doc_grep / doc_read_lines (2026-07-29, spec: docs/plans/2026-07-29-markitdown-grep-toolcall-spec.md §4)
//!
//! Coding-agent 语义的行级检索，替代 doc_scan 的"全量装载+自写解析"：
//! - `doc_grep`：关键词/正则逐行命中，返回**精确命中数**（total_hits，Rust
//!   数的——计数题不再需要 LLM 解析）、行号、上下文；`truncated` 即完备性
//!   声明（模型明确知道自己拿的是不是全部命中）。
//! - `doc_read_lines`：同一虚拟行视图上的区间原文读取。
//!
//! 虚拟行视图：文档的全部 text chunks 按 (page, chunk_id) 排序后拼接行序列。
//! 行号在同一语料版本内稳定（chunk 流不变则视图不变）。

use std::collections::BTreeMap;

use contracts::auth_runtime::AuthContext;
use contracts::{DocGrepArgs, DocReadLinesArgs, ToolResult, ToolStatus, ToolTrace};
use serde_json::json;
use uuid::Uuid;

use crate::RagRuntime;

const MAX_SCAN_CHUNKS: usize = 16384;
const MAX_LINE_CHARS: usize = 300;
const MAX_HITS_CAP: u32 = 200;
const DEFAULT_MAX_HITS: u32 = 50;
const MAX_CONTEXT: u32 = 3;
const MAX_READ_LINES: u32 = 400;

/// One document's virtual line view: chunk texts concatenated in
/// (page, chunk_id) order, **每行保留所属 chunk_id**——行命中必须能回映射到
/// chunk，否则证据平面（SELECTED 水合/引用/召回测量）拿不到 chunk 身份。
struct DocLines {
    /// (chunk_id, line_text) per line.
    lines: Vec<(Uuid, String)>,
    /// chunk_id → (full_text, page)，按 (page, chunk_id) 序，供 hits 回查全文。
    chunks: Vec<(Uuid, String, Option<i64>)>,
}

fn build_doc_line_views(
    chunks: Vec<avrag_retrieval_data_plane::ScoredChunk>,
) -> BTreeMap<Uuid, DocLines> {
    let mut by_doc: BTreeMap<Uuid, Vec<avrag_retrieval_data_plane::ScoredChunk>> = BTreeMap::new();
    for c in chunks {
        by_doc.entry(c.doc_id).or_default().push(c);
    }
    by_doc
        .into_iter()
        .map(|(doc_id, mut cs)| {
            cs.sort_by(|a, b| a.page.cmp(&b.page).then(a.chunk_id.cmp(&b.chunk_id)));
            let lines = cs
                .iter()
                .flat_map(|c| c.content.lines().map(move |l| (c.chunk_id, l.to_owned())))
                .collect::<Vec<_>>();
            let chunks = cs
                .iter()
                .map(|c| (c.chunk_id, c.content.clone(), c.page))
                .collect();
            (doc_id, DocLines { lines, chunks })
        })
        .collect()
}

/// 返回命中行所属 chunk 的去重列表（首见序）——证据平面食粮：全文、chunk_id、
/// doc_id、page。hydration/recall/citation 全部经既有 chunks 通道复用。
fn chunks_json_for_hits(
    hit_chunk_ids: &[Uuid],
    views: &BTreeMap<Uuid, DocLines>,
) -> Vec<serde_json::Value> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for cid in hit_chunk_ids {
        if !seen.insert(*cid) {
            continue;
        }
        for (doc_id, view) in views {
            if let Some((_, text, page)) = view.chunks.iter().find(|(id, _, _)| id == cid) {
                out.push(json!({
                    "chunk_id": cid.to_string(),
                    "doc_id": doc_id.to_string(),
                    "text": text,
                    "score": 0.0,
                    "page": page,
                    "source": "doc_grep",
                }));
                break;
            }
        }
    }
    out
}

fn clip(text: &str) -> String {
    if text.chars().count() <= MAX_LINE_CHARS {
        text.to_string()
    } else {
        format!("{}…", text.chars().take(MAX_LINE_CHARS).collect::<String>())
    }
}

fn context_window(
    lines: &[(Uuid, String)],
    idx: usize,
    context: u32,
) -> (Vec<String>, Vec<String>) {
    let lo = idx.saturating_sub(context as usize);
    let hi = (idx + context as usize + 1).min(lines.len());
    let before = lines[lo..idx].iter().map(|(_, l)| clip(l)).collect();
    let after = lines[idx + 1..hi].iter().map(|(_, l)| clip(l)).collect();
    (before, after)
}

enum Matcher {
    Substring(String),
    Regex(regex::Regex),
}

impl Matcher {
    fn is_hit(&self, line: &str) -> bool {
        match self {
            Self::Substring(p) => line.contains(p.as_str()),
            Self::Regex(re) => re.is_match(line),
        }
    }
}

fn resolve_doc_uuids(raw: &[String], tool: &str) -> Result<Vec<Uuid>, ToolResult> {
    if raw.is_empty() {
        return Err(super::error_result(
            tool,
            "doc_ids must not be empty".to_string(),
        ));
    }
    let uuids: Vec<Uuid> = raw
        .iter()
        .filter_map(|id| Uuid::parse_str(id).ok())
        .collect();
    if uuids.is_empty() {
        return Err(super::error_result(
            tool,
            "no valid doc_ids provided".to_string(),
        ));
    }
    Ok(uuids)
}

async fn load_line_views(
    runtime: &RagRuntime,
    auth: &AuthContext,
    doc_uuids: &[Uuid],
    tool: &str,
) -> Result<BTreeMap<Uuid, DocLines>, ToolResult> {
    match runtime.list_text_chunks(auth, doc_uuids).await {
        Ok(chunks) => {
            if chunks.len() > MAX_SCAN_CHUNKS {
                return Err(super::error_result(
                    tool,
                    format!(
                        "chunk count {} exceeds limit {}; narrow doc_scope",
                        chunks.len(),
                        MAX_SCAN_CHUNKS
                    ),
                ));
            }
            Ok(build_doc_line_views(chunks))
        }
        Err(e) => Err(super::error_result(tool, e.to_string())),
    }
}

pub async fn run_grep(
    runtime: &RagRuntime,
    auth: &AuthContext,
    args: &serde_json::Value,
) -> ToolResult {
    let mut normalized = args.clone();
    contracts::normalize_doc_id_alias(&mut normalized);
    let args: DocGrepArgs = match serde_json::from_value(normalized) {
        Ok(a) => a,
        Err(e) => return super::error_result("doc_grep", format!("invalid args: {e}")),
    };
    if args.pattern.is_empty() {
        return super::error_result("doc_grep", "pattern must not be empty".to_string());
    }
    let matcher = match if args.regex {
        regex::Regex::new(&args.pattern)
            .map(Matcher::Regex)
            .map_err(|e| e.to_string())
    } else {
        Ok(Matcher::Substring(args.pattern.clone()))
    } {
        Ok(m) => m,
        Err(e) => return super::error_result("doc_grep", format!("invalid regex: {e}")),
    };
    let doc_uuids = match resolve_doc_uuids(&args.doc_ids, "doc_grep") {
        Ok(u) => u,
        Err(r) => return r,
    };
    let context = args.context.min(MAX_CONTEXT);
    let max_hits = args
        .max_hits
        .unwrap_or(DEFAULT_MAX_HITS)
        .clamp(1, MAX_HITS_CAP) as usize;

    let started = std::time::Instant::now();
    let views = match load_line_views(runtime, auth, &doc_uuids, "doc_grep").await {
        Ok(v) => v,
        Err(r) => return r,
    };

    let mut total_hits = 0usize;
    let mut hits = Vec::new();
    let mut hit_chunk_ids: Vec<Uuid> = Vec::new();
    let mut truncated = false;
    // 永远扫完全部行：total_hits 必须精确（计数语义），截断只影响返回条数。
    for (doc_id, view) in &views {
        for (idx, (cid, line)) in view.lines.iter().enumerate() {
            if !matcher.is_hit(line) {
                continue;
            }
            total_hits += 1;
            if hits.len() >= max_hits {
                truncated = true;
                continue;
            }
            let (before, after) = context_window(&view.lines, idx, context);
            hit_chunk_ids.push(*cid);
            hits.push(json!({
                "doc_id": doc_id.to_string(),
                "line": idx + 1,
                "text": clip(line),
                "chunk_id": cid.to_string(),
                "before": before,
                "after": after,
            }));
        }
    }
    let chunks = chunks_json_for_hits(&hit_chunk_ids, &views);

    ToolResult {
        tool: "doc_grep".to_string(),
        version: "1.0".to_string(),
        status: ToolStatus::Ok,
        data: Some(json!({
            "total_hits": total_hits,
            "returned": hits.len(),
            "truncated": truncated,
            "hits": hits,
            "chunks": chunks,
            "request_pattern": args.pattern,
            "request_regex": args.regex,
        })),
        trace: Some(ToolTrace {
            elapsed_ms: Some(started.elapsed().as_millis() as u64),
            raw_hit_count: Some(total_hits),
            hydrated_hit_count: Some(total_hits),
            degrade_reason: Some("grep_data".to_string()),
        }),
    }
}

pub async fn run_read_lines(
    runtime: &RagRuntime,
    auth: &AuthContext,
    args: &serde_json::Value,
) -> ToolResult {
    let args: DocReadLinesArgs = match serde_json::from_value(args.clone()) {
        Ok(a) => a,
        Err(e) => return super::error_result("doc_read_lines", format!("invalid args: {e}")),
    };
    let doc_uuid = match Uuid::parse_str(&args.doc_id) {
        Ok(u) => u,
        Err(_) => return super::error_result("doc_read_lines", "invalid doc_id".to_string()),
    };
    if args.start == 0 || args.end < args.start {
        return super::error_result(
            "doc_read_lines",
            "start must be >= 1 and end >= start".to_string(),
        );
    }
    let end = args.end.min(args.start + MAX_READ_LINES - 1);

    let started = std::time::Instant::now();
    let views = match load_line_views(runtime, auth, &[doc_uuid], "doc_read_lines").await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let empty = DocLines {
        lines: Vec::new(),
        chunks: Vec::new(),
    };
    let view = views.get(&doc_uuid).unwrap_or(&empty);
    let total_lines = view.lines.len();
    let from = (args.start as usize).saturating_sub(1).min(total_lines);
    let to = (end as usize).min(total_lines);
    let mut range_chunk_ids = Vec::new();
    let window: Vec<_> = view.lines[from..to]
        .iter()
        .enumerate()
        .map(|(i, (cid, l))| {
            range_chunk_ids.push(*cid);
            json!({ "line": from + i + 1, "text": clip(l), "chunk_id": cid.to_string() })
        })
        .collect();
    let chunks = chunks_json_for_hits(&range_chunk_ids, &views);

    ToolResult {
        tool: "doc_read_lines".to_string(),
        version: "1.0".to_string(),
        status: ToolStatus::Ok,
        data: Some(json!({
            "doc_id": args.doc_id,
            "total_lines": total_lines,
            "start": from + 1,
            "end": to,
            "lines": window,
            "chunks": chunks,
        })),
        trace: Some(ToolTrace {
            elapsed_ms: Some(started.elapsed().as_millis() as u64),
            raw_hit_count: Some(window.len()),
            hydrated_hit_count: Some(window.len()),
            degrade_reason: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(
        doc: &str,
        id: u128,
        page: i64,
        text: &str,
    ) -> avrag_retrieval_data_plane::ScoredChunk {
        avrag_retrieval_data_plane::ScoredChunk {
            chunk_id: Uuid::from_u128(id),
            doc_id: Uuid::parse_str(doc).unwrap(),
            content: text.to_string(),
            score: 0.0,
            source: "test".to_string(),
            page: Some(page),
            chunk_type: "paragraph".to_string(),
            asset_id: None,
            caption: None,
            image_path: None,
            parser_backend: None,
            source_locator: None,
            parse_run_id: None,
            cursor: None,
        }
    }

    const DOC: &str = "693eb189-0b1e-462e-9d72-127339ecacea";

    #[test]
    fn line_view_orders_by_page_then_chunk_id() {
        let doc = DOC;
        let views = build_doc_line_views(vec![
            chunk(doc, 3, 2, "l4\nl5"),
            chunk(doc, 1, 1, "l2\nl3"),
            chunk(doc, 2, 1, "l1"),
        ]);
        // Same page: chunk_id orders; page precedes chunk_id.
        // (uuid v-from-u128 1 < 2 numerically? Uuid cmp is by bytes — yes.)
        let view = &views[&Uuid::parse_str(doc).unwrap()];
        // page1 chunks: id=1 ("l2\nl3") then id=2 ("l1"); page2: "l4\nl5".
        // Order by (page, chunk_id): (1,1),(1,2),(2,3); u128→Uuid keeps byte order.
        assert_eq!(view.lines.len(), 5);
        assert_eq!(view.lines[0].0, Uuid::from_u128(1));
        assert_eq!(view.lines[0].1, "l2");
    }

    #[test]
    fn substring_hit_and_context() {
        let id = Uuid::from_u128(7);
        let lines: Vec<(Uuid, String)> = ["a", "b hit", "c", "d hit2", "e"]
            .into_iter()
            .map(|s| (id, String::from(s)))
            .collect();
        let m = Matcher::Substring("hit".to_string());
        let hits: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, (_, l))| m.is_hit(l))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(hits, vec![1, 3]);
        let (before, after) = context_window(&lines, 1, 1);
        assert_eq!(before, vec!["a".to_string()]);
        assert_eq!(after, vec!["c".to_string()]);
    }

    #[test]
    fn chunks_json_dedups_and_preserves_first_hit_order() {
        let doc = Uuid::parse_str(DOC).unwrap();
        let views = build_doc_line_views(vec![
            chunk(DOC, 1, 1, "hit one\nplain"),
            chunk(DOC, 2, 1, "hit two\nhit three"),
        ]);
        let ids = [Uuid::from_u128(2), Uuid::from_u128(1), Uuid::from_u128(2)];
        let chunks = chunks_json_for_hits(&ids, &views);
        assert_eq!(chunks.len(), 2, "dedup by chunk_id");
        assert_eq!(chunks[0]["chunk_id"], Uuid::from_u128(2).to_string());
        assert_eq!(chunks[1]["chunk_id"], Uuid::from_u128(1).to_string());
        assert_eq!(chunks[0]["text"], "hit two\nhit three", "full chunk text");
        assert_eq!(chunks[0]["doc_id"], doc.to_string());
    }

    #[test]
    fn regex_matches_space_padded_pipe_cells() {
        // markitdown/PDF 方言：列宽 ljust 填充 → `| 概念阶段     |`。
        let m = Matcher::Regex(regex::Regex::new(r"\|\s*概念阶段\s*\|").unwrap());
        assert!(m.is_hit("| 概念阶段 |"));
        assert!(m.is_hit("| 概念阶段     |"));
        assert!(!m.is_hit("| 概念阶段工作 |"));
        assert!(!Matcher::Substring("| 概念阶段 |".to_string()).is_hit("| 概念阶段     |"));
    }

    #[test]
    fn clip_bounds_long_lines() {
        let long = "x".repeat(500);
        assert!(clip(&long).chars().count() <= MAX_LINE_CHARS + 1);
    }
}
