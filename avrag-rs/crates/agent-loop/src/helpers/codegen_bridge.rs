//! Codegen sandbox observation bridge — aligns with `iteration_codegen.rs`.

use contracts::{ToolResult, ToolStatus};
use serde_json::{Value, json};

fn l_eval_rrf_env_on() -> bool {
    std::env::var("GRAPH_L_EVAL_RRF")
        .map(|v| {
            let t = v.trim().to_ascii_lowercase();
            t == "1" || t == "true" || t == "yes" || t == "on"
        })
        .unwrap_or(false)
}

/// Parse sandbox stdout JSON into retrieval items for citation building.
///
/// Codegen observations use `content`; native tools use `text`. Both are normalized to `text`.
pub fn tool_result_from_code_execution_observation(observation: &str) -> Option<ToolResult> {
    let items = parse_retrieval_items_from_code_execution(observation)?;
    if items.is_empty() {
        return None;
    }
    Some(ToolResult {
        tool: "dense_retrieval".to_string(),
        version: "1.0".to_string(),
        status: ToolStatus::Ok,
        data: Some(serde_json::Value::Array(items)),
        trace: None,
    })
}

fn parse_retrieval_items_from_code_execution(observation: &str) -> Option<Vec<serde_json::Value>> {
    let mut items = Vec::new();
    for segment in observation.split("[block ") {
        let Some(stdout_part) = segment.split_once("stdout:") else {
            continue;
        };
        let after_stdout = stdout_part.1;
        let stdout = after_stdout
            .split_once("stderr:")
            .map(|(stdout, _)| stdout)
            .unwrap_or(after_stdout)
            .trim();
        if stdout.is_empty() {
            continue;
        }
        let parsed = serde_json::from_str::<serde_json::Value>(stdout).ok()?;
        match parsed {
            serde_json::Value::Array(arr) => items.extend(normalize_retrieval_items(arr)),
            serde_json::Value::Object(map) => {
                if let Some(arr) = map.get("chunks").and_then(|v| v.as_array()) {
                    items.extend(normalize_retrieval_items(arr.clone()));
                }
            }
            _ => {}
        }
    }
    if items.is_empty() { None } else { Some(items) }
}

fn normalize_retrieval_items(items: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    items
        .into_iter()
        .filter_map(|mut item| {
            let obj = item.as_object_mut()?;
            if !obj.contains_key("text")
                && let Some(content) = obj.get("content").and_then(|v| v.as_str())
            {
                obj.insert(
                    "text".to_string(),
                    serde_json::Value::String(content.to_string()),
                );
            }
            obj.get("chunk_id")
                .and_then(|v| v.as_str())
                .is_some_and(|id| !id.is_empty())
                .then(|| item)
        })
        .collect()
}

/// When sandbox stdout is empty but bridge captured retrieval chunks, serialize them for
/// `<code_execution_result>` so the model and exit policy see the same evidence as `tool_results`.
///
/// Includes compact `graph_context` from lexical force-augment telemetry so the model still
/// sees 1-hop structure when it did not print the bridge return value.
///
/// With `GRAPH_L_EVAL_RRF=1`, performs **three-way** dense ∪ BM25 ∪ graph RRF into `chunks`.
pub fn bridge_tool_results_to_observation_stdout(block_bridge: &[ToolResult]) -> Option<String> {
    let mut dense_items = Vec::new();
    let mut bm25_items = Vec::new();
    let mut graph_items = Vec::new();
    let mut graph_context = Vec::new();
    let mut items = Vec::new();
    let mut doc_scan_only = true;
    let mut doc_scan_chunk_count = 0usize;

    for result in block_bridge {
        if should_skip_bridge_tool_result(result) {
            doc_scan_chunk_count += count_bridge_tool_chunks(result);
            continue;
        }
        doc_scan_only = false;
        if result.status != ToolStatus::Ok {
            continue;
        }
        // Force-augment telemetry: graph_context only.
        if is_graph_augment_telemetry(result) {
            if let Some(data) = &result.data {
                if let Some(arr) = data.get("graph_context").and_then(|v| v.as_array()) {
                    graph_context.extend(arr.iter().cloned());
                }
            }
            continue;
        }
        let Some(data) = &result.data else {
            continue;
        };
        match data {
            Value::Array(arr) => {
                let norm = normalize_retrieval_items(arr.clone());
                if result.tool == "dense_retrieval" || result.tool == "dense" {
                    dense_items.extend(norm.clone());
                } else if result.tool == "graph_retrieval" || result.tool == "graph" {
                    // Explicit graph tool returns supporting chunks as array.
                    let mut marked = norm;
                    for it in &mut marked {
                        if let Some(obj) = it.as_object_mut() {
                            obj.insert("source".into(), json!("graph"));
                        }
                    }
                    graph_items.extend(marked);
                } else {
                    items.extend(norm);
                }
            }
            Value::Object(map) => {
                if result.tool == "lexical_retrieval" || result.tool == "lexical" {
                    // Prefer pure BM25 list for three-way fuse when present.
                    if let Some(arr) = map.get("bm25_chunks").and_then(|v| v.as_array()) {
                        bm25_items.extend(normalize_retrieval_items(arr.clone()));
                    } else if let Some(arr) = map.get("chunks").and_then(|v| v.as_array()) {
                        // Fall back: keep non-graph-tagged rows as lexical.
                        for it in normalize_retrieval_items(arr.clone()) {
                            let src = it.get("source").and_then(|s| s.as_str()).unwrap_or("");
                            if src == "graph" {
                                graph_items.push(it);
                            } else {
                                bm25_items.push(it);
                            }
                        }
                    }
                } else if let Some(arr) = map.get("chunks").and_then(|v| v.as_array()) {
                    items.extend(normalize_retrieval_items(arr.clone()));
                }
                if let Some(arr) = map.get("graph_context").and_then(|v| v.as_array()) {
                    graph_context.extend(arr.iter().cloned());
                }
            }
            _ => {}
        }
    }

    // Pull graph evidence from contexts into graph_items for RRF.
    for g in &graph_context {
        if let Some(evs) = g.get("evidence_chunks").and_then(|e| e.as_array()) {
            for ev in evs {
                let Some(text) = ev.get("text").and_then(|t| t.as_str()) else {
                    continue;
                };
                if text.trim().is_empty() {
                    continue;
                }
                let cid = ev
                    .get("chunk_id")
                    .and_then(|c| c.as_str())
                    .unwrap_or_default();
                if cid.is_empty() {
                    continue;
                }
                graph_items.push(json!({
                    "chunk_id": cid,
                    "doc_id": ev.get("doc_id").and_then(|d| d.as_str()).unwrap_or(""),
                    "text": text,
                    "content": text,
                    "score": ev.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0),
                    "source": "graph",
                }));
            }
        }
    }

    if l_eval_rrf_env_on()
        && (!dense_items.is_empty() || !bm25_items.is_empty() || !graph_items.is_empty())
    {
        let fused = rrf_merge_json_lists(
            [
                ("dense", dense_items),
                ("bm25", bm25_items),
                ("graph", graph_items),
            ],
            60,
        );
        let graph_in_top15 = fused
            .iter()
            .take(15)
            .filter(|c| c.get("source").and_then(|s| s.as_str()) == Some("graph"))
            .count();
        return serde_json::to_string(&json!({
            "chunks": fused,
            "graph_context": graph_context,
            "l_eval_rrf": true,
            "l_eval_channels": ["dense", "bm25", "graph"],
            "graph_chunk_in_top15": graph_in_top15,
            "graph_context_len": graph_context.len(),
        }))
        .ok();
    }

    // Legacy path (no L-eval): concatenate as before.
    items.extend(dense_items);
    items.extend(bm25_items);
    items.extend(graph_items.into_iter().filter(|it| {
        // Prefer not dumping raw graph rows into non-L-eval chunks unless alone.
        it.get("source").and_then(|s| s.as_str()) != Some("graph")
    }));

    if !items.is_empty() {
        if graph_context.is_empty() {
            return serde_json::to_string(&items).ok();
        }
        return serde_json::to_string(&json!({
            "chunks": items,
            "graph_context": graph_context,
        }))
        .ok();
    }
    if !graph_context.is_empty() {
        return serde_json::to_string(&json!({
            "chunks": [],
            "graph_context": graph_context,
        }))
        .ok();
    }
    if doc_scan_only && doc_scan_chunk_count > 0 {
        return Some(format!(
            "doc_scan loaded {doc_scan_chunk_count} segments into the sandbox for code-side scan/count; print a compact result (numbers or a short list)"
        ));
    }
    None
}

/// JSON-level RRF over channel lists (dedupe by chunk_id).
fn rrf_merge_json_lists(
    channels: [(&str, Vec<Value>); 3],
    rrf_k: usize,
) -> Vec<Value> {
    use std::collections::HashMap;
    let mut scores: HashMap<String, f64> = HashMap::new();
    let mut best: HashMap<String, Value> = HashMap::new();
    for (channel, list) in channels {
        for (rank, mut item) in list.into_iter().enumerate() {
            let Some(id) = item
                .get("chunk_id")
                .and_then(|c| c.as_str())
                .map(str::to_owned)
            else {
                continue;
            };
            if id.is_empty() {
                continue;
            }
            let add = 1.0 / (rrf_k as f64 + rank as f64);
            *scores.entry(id.clone()).or_insert(0.0) += add;
            if let Some(obj) = item.as_object_mut() {
                if !obj.contains_key("source") {
                    obj.insert("source".into(), json!(channel));
                }
                // Prefer text field for model; keep content alias.
                if !obj.contains_key("text") {
                    if let Some(c) = obj.get("content").cloned() {
                        obj.insert("text".into(), c);
                    }
                }
            }
            best.entry(id).or_insert(item);
        }
    }
    let mut fused: Vec<(f64, Value)> = scores
        .into_iter()
        .filter_map(|(id, sc)| {
            let mut v = best.remove(&id)?;
            if let Some(obj) = v.as_object_mut() {
                obj.insert("score".into(), json!(sc));
            }
            Some((sc, v))
        })
        .collect();
    fused.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    fused.into_iter().map(|(_, v)| v).collect()
}

fn is_graph_augment_telemetry(result: &ToolResult) -> bool {
    result.tool == "graph_retrieval"
        && result
            .trace
            .as_ref()
            .and_then(|t| t.degrade_reason.as_deref())
            == Some("graph_augment")
}

fn should_skip_bridge_tool_result(result: &ToolResult) -> bool {
    result.tool == "doc_scan"
        || result
            .trace
            .as_ref()
            .and_then(|t| t.degrade_reason.as_deref())
            == Some("scan_data")
}

fn count_bridge_tool_chunks(result: &ToolResult) -> usize {
    if result.status != ToolStatus::Ok {
        return 0;
    }
    if let Some(count) = result.trace.as_ref().and_then(|t| t.raw_hit_count) {
        return count;
    }
    let Some(data) = &result.data else {
        return 0;
    };
    match data {
        serde_json::Value::Array(arr) => arr.len(),
        serde_json::Value::Object(map) => map
            .get("chunks")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0),
        _ => 0,
    }
}

/// Resolve stdout text shown to the model after codegen; bridge chunks fill empty stdout.
///
/// With `GRAPH_L_EVAL_RRF=1`, prefer block-level three-way fused observation over raw prints
/// so dense∪BM25∪graph is always what the model and exit policy see.
pub fn codegen_observation_stdout(exec_stdout: &str, block_bridge: &[ToolResult]) -> String {
    if l_eval_rrf_env_on() {
        if let Some(fused) = bridge_tool_results_to_observation_stdout(block_bridge) {
            return fused;
        }
    }
    if !crate::react_loop::exit_policy::stdout_is_placeholder(exec_stdout.trim())
        && !exec_stdout.trim().is_empty()
    {
        return exec_stdout.to_string();
    }
    bridge_tool_results_to_observation_stdout(block_bridge)
        .unwrap_or_else(|| exec_stdout.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::citations::build_citations_from_tool_results;
    use contracts::ToolResult;

    fn tr(tool: &str, status: ToolStatus, data: Option<serde_json::Value>) -> ToolResult {
        ToolResult {
            tool: tool.to_string(),
            version: "1.0".to_string(),
            status,
            data,
            trace: None,
        }
    }

    #[test]
    fn test_codegen_observation_stdout_uses_bridge_when_exec_stdout_empty() {
        let bridge = vec![tr(
            "dense_retrieval",
            ToolStatus::Ok,
            Some(serde_json::json!([
                {"chunk_id": "6c16ac99-e934-4355-be1c-f0956acb51d1", "doc_id": "5a6de5e8-e913-46c9-a109-43eb65ae4e79", "content": "hello", "score": 0.9}
            ])),
        )];
        let stdout = codegen_observation_stdout("", &bridge);
        assert!(
            stdout.contains("6c16ac99-e934-4355-be1c-f0956acb51d1"),
            "stdout={stdout}"
        );
        assert!(stdout.contains("hello"));
        let observation = format!(
            "<code_execution_result>\n[block 0] stdout: {stdout}\nstderr: \n</code_execution_result>"
        );
        assert!(crate::react_loop::exit_policy::code_execution_has_evidence(
            &observation
        ));
    }

    #[test]
    fn test_observation_includes_graph_context_from_augment_telemetry() {
        use contracts::ToolTrace;
        let bridge = vec![
            tr(
                "lexical_retrieval",
                ToolStatus::Ok,
                Some(serde_json::json!({
                    "chunks": [
                        {"chunk_id": "c1", "doc_id": "d1", "content": "body", "score": 0.9}
                    ]
                })),
            ),
            ToolResult {
                tool: "graph_retrieval".into(),
                version: "1.0".into(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!({
                    "graph_context": [{
                        "subject": "DRC",
                        "object": "DRO",
                        "expansion_hop_limit": 1,
                        "evidence_chunks": [{
                            "chunk_id": "g1",
                            "doc_id": "d1",
                            "text": "graph body",
                            "score": 1.0,
                            "score_gap_to_top1": 0.0,
                            "kept_reason": "top1"
                        }]
                    }]
                })),
                trace: Some(ToolTrace {
                    elapsed_ms: Some(2),
                    raw_hit_count: Some(1),
                    hydrated_hit_count: Some(1),
                    degrade_reason: Some("graph_augment".into()),
                }),
            },
        ];
        let stdout = bridge_tool_results_to_observation_stdout(&bridge).expect("stdout");
        let v: serde_json::Value = serde_json::from_str(&stdout).expect("json");
        assert_eq!(v["chunks"][0]["chunk_id"], "c1");
        assert_eq!(v["graph_context"][0]["subject"], "DRC");
        assert_eq!(
            v["graph_context"][0]["evidence_chunks"][0]["score_gap_to_top1"],
            0.0
        );
    }

    #[test]
    fn test_l_eval_three_way_prefers_fused_with_dense_bm25_graph() {
        // SAFETY: test-only env toggle; serial in practice for this module tests.
        unsafe { std::env::set_var("GRAPH_L_EVAL_RRF", "1") };
        let bridge = vec![
            tr(
                "dense_retrieval",
                ToolStatus::Ok,
                Some(serde_json::json!([
                    {"chunk_id": "d1", "doc_id": "x", "text": "dense", "score": 0.9}
                ])),
            ),
            tr(
                "lexical_retrieval",
                ToolStatus::Ok,
                Some(serde_json::json!({
                    "bm25_chunks": [
                        {"chunk_id": "b1", "doc_id": "x", "text": "bm25", "score": 0.8}
                    ],
                    "chunks": [
                        {"chunk_id": "b1", "doc_id": "x", "text": "bm25", "score": 0.8}
                    ]
                })),
            ),
            tr(
                "graph_retrieval",
                ToolStatus::Ok,
                Some(serde_json::json!([
                    {"chunk_id": "g1", "doc_id": "x", "text": "graph", "score": 0.7, "source": "graph"}
                ])),
            ),
        ];
        let stdout = bridge_tool_results_to_observation_stdout(&bridge).expect("stdout");
        let v: serde_json::Value = serde_json::from_str(&stdout).expect("json");
        assert_eq!(v["l_eval_rrf"], true);
        let chunks = v["chunks"].as_array().expect("chunks");
        assert_eq!(chunks.len(), 3);
        let ids: Vec<&str> = chunks
            .iter()
            .filter_map(|c| c.get("chunk_id").and_then(|x| x.as_str()))
            .collect();
        assert!(ids.contains(&"d1") && ids.contains(&"b1") && ids.contains(&"g1"));
        unsafe { std::env::remove_var("GRAPH_L_EVAL_RRF") };
    }

    #[test]
    fn test_codegen_observation_stdout_keeps_exec_stdout_when_present() {
        let bridge = vec![tr(
            "dense_retrieval",
            ToolStatus::Ok,
            Some(serde_json::json!([{"chunk_id": "c1", "text": "bridge"}])),
        )];
        let stdout = codegen_observation_stdout(r#"{"chunk_id":"from_print"}"#, &bridge);
        assert_eq!(stdout, r#"{"chunk_id":"from_print"}"#);
    }

    #[test]
    fn test_l_eval_prefers_fused_over_model_print() {
        unsafe { std::env::set_var("GRAPH_L_EVAL_RRF", "1") };
        let bridge = vec![
            tr(
                "dense_retrieval",
                ToolStatus::Ok,
                Some(serde_json::json!([
                    {"chunk_id": "d1", "doc_id": "x", "text": "dense", "score": 0.9}
                ])),
            ),
            tr(
                "lexical_retrieval",
                ToolStatus::Ok,
                Some(serde_json::json!({
                    "bm25_chunks": [
                        {"chunk_id": "b1", "doc_id": "x", "text": "bm25", "score": 0.8}
                    ]
                })),
            ),
        ];
        let stdout = codegen_observation_stdout(r#"{"chunk_id":"from_print"}"#, &bridge);
        let v: serde_json::Value = serde_json::from_str(&stdout).expect("json");
        assert_eq!(v["l_eval_rrf"], true);
        assert!(v["chunks"].as_array().map(|a| a.len() >= 2).unwrap_or(false));
        assert!(!stdout.contains("from_print"));
        unsafe { std::env::remove_var("GRAPH_L_EVAL_RRF") };
    }

    #[test]
    fn test_code_execution_observation_builds_dense_retrieval_tool_result() {
        let observation = r#"[block 0] stdout: [{"chunk_id":"c1","doc_id":"d1","content":"hello","score":0.9}]
stderr: 
"#;
        let result = tool_result_from_code_execution_observation(observation).unwrap();
        assert_eq!(result.tool, "dense_retrieval");
        let arr = result.data.as_ref().unwrap().as_array().unwrap();
        assert_eq!(arr[0]["chunk_id"], "c1");
        assert_eq!(arr[0]["text"], "hello");
        let citations = build_citations_from_tool_results(std::slice::from_ref(&result));
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].chunk_id.as_deref(), Some("c1"));
    }

    #[test]
    fn test_bridge_skips_doc_scan_and_returns_compact_hint() {
        let bridge = vec![tr(
            "doc_scan",
            ToolStatus::Ok,
            Some(serde_json::json!([
                {"chunk_id": "c1", "doc_id": "d1", "content": "full body", "score": 0.0},
                {"chunk_id": "c2", "doc_id": "d1", "content": "more body", "score": 0.0},
            ])),
        )];
        let stdout = codegen_observation_stdout("", &bridge);
        assert!(
            stdout.contains("doc_scan loaded 2 segments"),
            "stdout={stdout}"
        );
        assert!(
            stdout.contains("code-side scan") || stdout.contains("compact"),
            "stdout={stdout}"
        );
        assert!(!stdout.contains("full body"));
    }

    #[test]
    fn test_bridge_skips_scan_data_trace_but_keeps_dense_retrieval() {
        let bridge = vec![
            tr(
                "dense_retrieval",
                ToolStatus::Ok,
                Some(serde_json::json!([
                    {"chunk_id": "c1", "doc_id": "d1", "content": "dense hit", "score": 0.9}
                ])),
            ),
            tr(
                "doc_scan",
                ToolStatus::Ok,
                Some(serde_json::json!([
                    {"chunk_id": "c2", "doc_id": "d1", "content": "scan body", "score": 0.0}
                ])),
            ),
        ];
        let stdout = codegen_observation_stdout("", &bridge);
        assert!(stdout.contains("c1"), "stdout={stdout}");
        assert!(stdout.contains("dense hit"));
        assert!(!stdout.contains("scan body"));
    }
}
