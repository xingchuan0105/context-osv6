//! Mock RAG codegen bodies and memory-tool helpers for Product E2E.

use super::mock_rag_state::read_mock_rag_state;
use axum::response::IntoResponse;
use serde_json::json;

pub(super) fn latest_user_message_content(messages: &[serde_json::Value]) -> Option<&str> {
    messages
        .iter()
        .rev()
        .find(|message| message.get("role").and_then(|role| role.as_str()) == Some("user"))
        .and_then(|message| message.get("content").and_then(|content| content.as_str()))
}

pub(super) fn dense_search_query_from_messages(messages: &[serde_json::Value]) -> Option<String> {
    let content = latest_user_message_content(messages)?;
    let query = content
        .trim()
        .trim_start_matches("[prior_user_query]")
        .trim();
    if query.is_empty() {
        None
    } else {
        Some(query.to_string())
    }
}

pub(super) fn resolve_dense_search_query(messages: &[serde_json::Value]) -> String {
    dense_search_query_from_messages(messages)
        .or_else(|| read_mock_rag_state(|state| state.codegen_query.clone()))
        .unwrap_or_else(|| "antifragility".to_string())
}

pub(super) fn format_mock_rag_codegen_response_for_query(query: &str) -> String {
    let query_json =
        serde_json::to_string(query).unwrap_or_else(|_| "\"antifragility\"".to_string());
    format!(
        r#"<code language="python">
chunks = await client.dense_search(query={query_json}, top_k=10)
import json
print(json.dumps(chunks))
</code>"#
    )
}

/// Build mock codegen body that exercises the sandbox retrieval bridge.
///
/// The query defaults to `"antifragility"`, which matches the standard smoke fixture
/// `antifragile.txt`. Override via [`set_mock_rag_codegen_query`] when using other fixtures.
pub fn format_mock_rag_codegen_response(_chunk_id: &str) -> String {
    format_mock_rag_codegen_response_for_query(&read_mock_rag_state(|state| {
        state
            .codegen_query
            .clone()
            .unwrap_or_else(|| "antifragility".to_string())
    }))
}

/// Round0 multiround codegen: fetch document profile (sections + metadata).
pub fn format_mock_rag_doc_profile_codegen(doc_id: &str) -> String {
    let doc_id_json = serde_json::to_string(doc_id).unwrap_or_else(|_| "\"doc\"".to_string());
    format!(
        r#"<code language="python">
profile = await client.doc_profile(doc_ids=[{doc_id_json}])
import json
print(json.dumps(profile))
</code>"#
    )
}

/// Round1 multiround codegen: fetch chunk body by id (`chunk_fetch` → `index_lookup`).
pub fn format_mock_rag_chunk_fetch_codegen(chunk_id: &str) -> String {
    let chunk_json = serde_json::to_string(chunk_id).unwrap_or_else(|_| "\"chunk\"".to_string());
    format!(
        r#"<code language="python">
chunks = await client.chunk_fetch(chunk_id={chunk_json})
import json
print(json.dumps(chunks))
</code>"#
    )
}

pub(super) fn count_code_execution_results(messages: &[serde_json::Value]) -> usize {
    messages
        .iter()
        .filter(|message| {
            message
                .get("content")
                .and_then(|content| content.as_str())
                .is_some_and(|content| content.contains("<code_execution_result>"))
        })
        .count()
}

pub(super) fn mock_memory_tool_call(tool: &str) -> Option<serde_json::Value> {
    let (id, arguments) = match tool {
        "conversation_history_load" => (
            "call_mem_history_0",
            serde_json::to_string(
                &json!({"query": "antifragility", "scope": "workspace", "limit": 20}),
            )
            .unwrap_or_else(|_| "{}".to_string()),
        ),
        "user_profile_load" => ("call_mem_profile_0", "{}".to_string()),
        _ => return None,
    };
    Some(json!({
        "id": id,
        "type": "function",
        "function": {
            "name": tool,
            "arguments": arguments,
        }
    }))
}

pub(super) fn messages_have_code_execution_result(messages: &[serde_json::Value]) -> bool {
    messages.iter().any(|message| {
        message
            .get("content")
            .and_then(|content| content.as_str())
            .is_some_and(|content| content.contains("<code_execution_result>"))
    })
}

pub(super) fn mock_rag_retrieve_codegen_content(messages: &[serde_json::Value]) -> String {
    if read_mock_rag_state(|state| state.skip_codegen) {
        return String::new();
    }
    if read_mock_rag_state(|state| state.multiround_profile) {
        let rounds = count_code_execution_results(messages);
        return match rounds {
            0 => {
                let doc_id = read_mock_rag_state(|state| {
                    state
                        .codegen_doc_id
                        .clone()
                        .unwrap_or_else(|| "00000000-0000-4000-8000-000000000001".to_string())
                });
                format_mock_rag_doc_profile_codegen(&doc_id)
            }
            1 => {
                let chunk_id = read_mock_rag_state(|state| {
                    state
                        .codegen_chunk_id
                        .clone()
                        .unwrap_or_else(|| "00000000-0000-4000-8000-000000000001".to_string())
                });
                format_mock_rag_chunk_fetch_codegen(&chunk_id)
            }
            _ => String::new(),
        };
    }
    if messages_have_code_execution_result(messages) {
        String::new()
    } else {
        format_mock_rag_codegen_response_for_query(&resolve_dense_search_query(messages))
    }
}

pub(super) fn try_memory_tool_response(
    tool_names: &[String],
    has_tool_results: bool,
) -> Option<axum::response::Response> {
    if has_tool_results {
        return None;
    }
    let requested = read_mock_rag_state(|state| state.emit_memory_tool.clone())?;
    if !tool_names.iter().any(|name| name == &requested) {
        return None;
    }
    let tool_call = mock_memory_tool_call(&requested)?;
    Some(
        axum::Json(json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "",
                    "tool_calls": [tool_call],
                }
            }],
            "usage": {"prompt_tokens": 100, "completion_tokens": 1, "total_tokens": 101},
            "model": "mock-llm"
        }))
        .into_response(),
    )
}
