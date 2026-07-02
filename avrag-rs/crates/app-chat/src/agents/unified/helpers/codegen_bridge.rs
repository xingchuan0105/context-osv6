//! Codegen sandbox observation bridge — aligns with `iteration_codegen.rs`.

use contracts::{ToolResult, ToolStatus};

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
pub fn bridge_tool_results_to_observation_stdout(block_bridge: &[ToolResult]) -> Option<String> {
    let mut items = Vec::new();
    for result in block_bridge {
        if result.status != ToolStatus::Ok {
            continue;
        }
        let Some(data) = &result.data else {
            continue;
        };
        match data {
            serde_json::Value::Array(arr) => items.extend(normalize_retrieval_items(arr.clone())),
            serde_json::Value::Object(map) => {
                if let Some(arr) = map.get("chunks").and_then(|v| v.as_array()) {
                    items.extend(normalize_retrieval_items(arr.clone()));
                }
            }
            _ => {}
        }
    }
    if items.is_empty() {
        return None;
    }
    serde_json::to_string(&items).ok()
}

/// Resolve stdout text shown to the model after codegen; bridge chunks fill empty stdout.
pub fn codegen_observation_stdout(exec_stdout: &str, block_bridge: &[ToolResult]) -> String {
    if !crate::agents::r#loop::exit_policy::stdout_is_placeholder(exec_stdout.trim())
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
    use crate::agents::unified::helpers::citations::build_citations_from_tool_results;
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
                {"chunk_id": "c1", "doc_id": "d1", "content": "hello", "score": 0.9}
            ])),
        )];
        let stdout = codegen_observation_stdout("", &bridge);
        assert!(stdout.contains("c1"), "stdout={stdout}");
        assert!(stdout.contains("hello"));
        let observation = format!(
            "<code_execution_result>\n[block 0] stdout: {stdout}\nstderr: \n</code_execution_result>"
        );
        assert!(crate::agents::r#loop::exit_policy::code_execution_has_evidence(&observation));
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
}
