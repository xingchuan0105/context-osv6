use contracts::ToolResult;
use serde_json::Value;

/// Map each RAG tool to the JSON arg field that carries its document scope.
///
/// Retrieval tools (`dense_retrieval`/`lexical_retrieval`/`graph_retrieval`) use an
/// array field named `doc_scope`. Doc-centric tools (`doc_summary`/`doc_metadata`/
/// `doc_profile`/`doc_scan`) use an array field named `doc_ids`. All of these arg
/// structs use `#[serde(deny_unknown_fields)]`, so the scope must be injected under
/// the exact field the downstream tool expects. `index_lookup` carries a single
/// `doc_id` string and is handled by the codegen bridge instead.
fn scope_field_for_tool(tool: &str) -> Option<&'static str> {
    match tool {
        "dense_retrieval" | "lexical_retrieval" | "graph_retrieval" => Some("doc_scope"),
        "doc_summary" | "doc_metadata" | "doc_profile" | "doc_scan" => Some("doc_ids"),
        _ => None,
    }
}

/// Intersect caller-supplied doc ids against the session scope.
/// - If `scope` is empty: no enforcement (org-wide permitted by upstream).
/// - If `scope` is non-empty: result is caller ∩ scope; if caller is empty, use scope;
///   if caller has items but none match scope, return scope (fall back to session scope
///   rather than allowing an out-of-scope id or an empty all-matching scope).
pub(crate) fn intersect_doc_scope(caller: &[String], scope: &[String]) -> Vec<String> {
    if scope.is_empty() {
        return caller.to_vec();
    }
    if caller.is_empty() {
        return scope.to_vec();
    }
    let scope_set: std::collections::HashSet<&String> = scope.iter().collect();
    let intersection: Vec<String> = caller
        .iter()
        .filter(|c| scope_set.contains(*c))
        .cloned()
        .collect();
    if intersection.is_empty() {
        scope.to_vec()
    } else {
        intersection
    }
}

/// Read an array-of-strings field from a JSON value. Returns an empty vec when the
/// field is absent or not a string array.
fn read_string_array(args: &Value, field: &str) -> Vec<String> {
    args.get(field)
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

/// Force the document scope onto a RAG tool call via intersection.
///
/// Unlike the previous "fill only when empty" merge, this **always** overrides the
/// scope-carrying arg field so the LLM can never widen scope beyond what the caller
/// (session) permitted. `caller` is read from the existing args (under the tool's
/// canonical scope field), then intersected against `scope`, and the result is
/// written back to the same field.
pub(crate) fn force_doc_scope(call: &mut contracts::ToolCall, scope: &[String]) {
    let Some(field) = scope_field_for_tool(&call.tool) else {
        return;
    };
    // Read the caller-supplied scope before mutably borrowing args.
    let caller = read_string_array(&call.args, field);
    let resolved = intersect_doc_scope(&caller, scope);
    if let Some(args) = call.args.as_object_mut() {
        args.insert(field.to_string(), serde_json::json!(resolved));
    }
}

pub(crate) async fn dispatch_rag_tool(
    runtime: &avrag_rag_core::RagRuntime,
    auth: &avrag_auth::AuthContext,
    call: &contracts::ToolCall,
    doc_scope: &[String],
) -> ToolResult {
    let mut call = call.clone();
    // Enforce scope on every RAG tool. Non-RAG tools are not routed here, so apply
    // unconditionally rather than guarding on specific tool names.
    force_doc_scope(&mut call, doc_scope);
    avrag_rag_core::runtime::tools::dispatch(runtime, auth, &call).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::ToolCall;

    fn call(tool: &str, scope_value: serde_json::Value) -> ToolCall {
        let mut args = serde_json::Map::new();
        if !scope_value.is_null() {
            if let Some(field) = scope_field_for_tool(tool) {
                args.insert(field.to_string(), scope_value);
            }
        }
        ToolCall {
            tool: tool.to_string(),
            version: "1.0".to_string(),
            args: Value::Object(args),
        }
    }

    fn doc_scope_of(call: &ToolCall, tool: &str) -> Vec<String> {
        let field = scope_field_for_tool(tool).expect("rag tool");
        read_string_array(&call.args, field)
    }

    #[test]
    fn intersect_preserves_caller_when_scope_empty() {
        let caller = vec!["a".to_string(), "b".to_string()];
        assert_eq!(intersect_doc_scope(&caller, &[]), caller);
    }

    #[test]
    fn intersect_uses_full_scope_when_caller_empty() {
        let scope = vec!["x".to_string(), "y".to_string()];
        assert_eq!(intersect_doc_scope(&[], &scope), scope);
    }

    #[test]
    fn intersect_narrows_out_of_scope_caller_to_scope() {
        // caller asks for an out-of-scope id -> falls back to the whole session scope.
        let scope = vec!["s1".to_string(), "s2".to_string()];
        let caller = vec!["out-of-scope".to_string()];
        assert_eq!(intersect_doc_scope(&caller, &scope), scope);
    }

    #[test]
    fn intersect_intersects_partial_overlap() {
        let scope = vec!["s1".to_string(), "s2".to_string()];
        let caller = vec!["s2".to_string(), "s3".to_string()];
        assert_eq!(intersect_doc_scope(&caller, &scope), vec!["s2".to_string()]);
    }

    #[test]
    fn force_doc_scope_llm_out_of_scope_narrowed_down() {
        // The LLM supplies a doc id outside the session scope; force_doc_scope must
        // clamp it down to the session scope rather than honoring the LLM's pick.
        let mut c = call("dense_retrieval", serde_json::json!(["llm-evil-id"]));
        let scope = vec!["session-1".to_string(), "session-2".to_string()];
        force_doc_scope(&mut c, &scope);
        assert_eq!(doc_scope_of(&c, "dense_retrieval"), scope);
    }

    #[test]
    fn force_doc_scope_llm_empty_uses_full_scope() {
        let mut c = call("doc_summary", serde_json::json!([]));
        let scope = vec!["d1".to_string(), "d2".to_string()];
        force_doc_scope(&mut c, &scope);
        assert_eq!(doc_scope_of(&c, "doc_summary"), scope);
    }

    #[test]
    fn force_doc_scope_scope_empty_preserves_caller_org_wide() {
        let mut c = call("lexical_retrieval", serde_json::json!(["any-doc"]));
        force_doc_scope(&mut c, &[]);
        assert_eq!(
            doc_scope_of(&c, "lexical_retrieval"),
            vec!["any-doc".to_string()]
        );
    }

    #[test]
    fn force_doc_scope_preserves_in_scope_caller() {
        // LLM-supplied id that *is* in scope survives the intersection.
        let mut c = call("doc_profile", serde_json::json!(["d2", "d3"]));
        let scope = vec!["d1".to_string(), "d2".to_string()];
        force_doc_scope(&mut c, &scope);
        assert_eq!(doc_scope_of(&c, "doc_profile"), vec!["d2".to_string()]);
    }

    #[test]
    fn all_rag_tools_get_force_set() {
        // Every RAG tool must receive the enforced scope regardless of what the
        // LLM emitted (here: empty caller -> full scope).
        let tools = [
            "dense_retrieval",
            "lexical_retrieval",
            "graph_retrieval",
            "doc_summary",
            "doc_metadata",
            "doc_profile",
            "doc_scan",
        ];
        let scope = vec!["enforced-1".to_string()];
        for tool in tools {
            let mut c = call(tool, serde_json::json!([]));
            force_doc_scope(&mut c, &scope);
            assert_eq!(
                doc_scope_of(&c, tool),
                scope,
                "tool {tool} did not get force-set"
            );
        }
    }

    #[test]
    fn force_doc_scope_ignores_non_rag_tool() {
        // A non-RAG tool has no scope field and must be left untouched.
        let mut c = ToolCall {
            tool: "web_search".to_string(),
            version: "1.0".to_string(),
            args: serde_json::json!({"query": "foo"}),
        };
        force_doc_scope(&mut c, &["whatever".to_string()]);
        assert_eq!(c.args, serde_json::json!({"query": "foo"}));
    }
}
