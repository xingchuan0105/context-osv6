/// Simple bilingual message helper.
/// Defaults to Chinese (zh) unless language is explicitly "en" or "en-US".
pub fn msg(language: Option<&str>, zh: &'static str, en: &'static str) -> &'static str {
    match language {
        Some("en") | Some("en-US") | Some("en-us") => en,
        _ => zh,
    }
}

/// RAG clarification messages.
pub mod clarify {
    use super::msg;

    pub fn need_doc_scope(language: Option<&str>) -> &'static str {
        msg(
            language,
            "请先选择要检索的文档范围，再让我执行知识库检索。",
            "Please select the documents to search first, then I will perform the knowledge base retrieval.",
        )
    }

    pub fn need_query_or_doc_scope(language: Option<&str>) -> &'static str {
        msg(
            language,
            "请补充要检索的具体问题或目标文档范围。",
            "Please provide a specific question or select target documents to search.",
        )
    }
}

/// Activity / streaming status messages.
pub mod activity {}

/// Fallback / static answer messages.
pub mod fallback {
    use super::msg;

    pub fn no_valid_retrieval_results(language: Option<&str>) -> &'static str {
        msg(
            language,
            "未找到足够的证据来回答您的问题。请尝试更换关键词或上传更多相关文档。",
            "No sufficient evidence was found to answer your question. Try different keywords or upload more relevant documents.",
        )
    }
}

/// Agent display names.
pub fn agent_name(agent_type: &str, language: Option<&str>) -> &'static str {
    match agent_type {
        "search" => msg(language, "网络搜索助手", "Web Search Agent"),
        "general" => msg(language, "通用聊天助手", "General Chat Agent"),
        _ => msg(language, "知识库助手", "Knowledge Base Agent"),
    }
}
