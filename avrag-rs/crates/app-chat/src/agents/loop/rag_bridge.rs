use contracts::ToolResult;

pub(crate) fn merge_request_doc_scope(call: &mut contracts::ToolCall, doc_scope: &[String]) {
    if doc_scope.is_empty() {
        return;
    }
    let Some(args) = call.args.as_object_mut() else {
        return;
    };
    let scope_empty = args
        .get("doc_scope")
        .and_then(|value| value.as_array())
        .is_none_or(|items| items.is_empty());
    if scope_empty {
        args.insert("doc_scope".to_string(), serde_json::json!(doc_scope));
    }
}

pub(crate) async fn dispatch_rag_tool(
    runtime: &avrag_rag_core::RagRuntime,
    auth: &avrag_auth::AuthContext,
    call: &contracts::ToolCall,
    doc_scope: &[String],
) -> ToolResult {
    let mut call = call.clone();
    if call.tool == "dense_retrieval" || call.tool == "lexical_retrieval" {
        merge_request_doc_scope(&mut call, doc_scope);
    }
    avrag_rag_core::runtime::tools::dispatch(runtime, auth, &call).await
}
