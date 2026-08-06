//! Product labels for progress (no internal tool ids).

/// Bridge SDK method → product label (Chinese).
pub fn product_action_for_bridge_method(method: &str) -> Option<&'static str> {
    match method {
        "dense" | "dense_search" => Some("语义检索"),
        "lexical" | "lexical_search" => Some("关键词检索"),
        "grep" => Some("行级检索"),
        "web" => Some("网页搜索"),
        "fetch" => Some("读取网页"),
        "doc_summary" => Some("查看文档档案"),
        "history" | "user_profile" => Some("回忆相关上下文"),
        "graph_search" => Some("关系检索"),
        "doc_scan" | "doc_chunks" | "chunk_fetch" | "read_lines" => Some("文档读取"),
        _ => None,
    }
}

/// Native ReAct tool id → product label (never show raw id in UI title).
pub fn product_action_for_native_tool(tool: &str) -> Option<&'static str> {
    match tool {
        "web_search" => Some("网页搜索"),
        "web_fetch" => Some("读取网页"),
        "conversation_history_load" | "user_profile_load" => Some("回忆相关上下文"),
        _ => None,
    }
}
