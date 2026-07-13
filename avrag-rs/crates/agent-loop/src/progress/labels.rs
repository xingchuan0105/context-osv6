//! Product labels for progress (no internal tool ids).

/// Bridge SDK method → product label (Chinese).
pub fn product_action_for_bridge_method(method: &str) -> Option<&'static str> {
    match method {
        "dense_search" => Some("语义检索"),
        "lexical_search" => Some("关键词检索"),
        "graph_search" => Some("关系检索"),
        "doc_summary" => Some("阅读文档摘要"),
        "doc_profile" => Some("查看文档结构"),
        "doc_chunks" => Some("通读文档片段"),
        "chunk_fetch" => Some("展开原文片段"),
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
