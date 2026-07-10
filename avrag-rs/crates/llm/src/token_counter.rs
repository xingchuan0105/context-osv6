use tiktoken_rs::cl100k_base_singleton;

/// Count tokens in text using the cl100k_base tokenizer (GPT-4 / Claude / Qwen compatible).
///
/// Uses the process-wide tokenizer singleton. Calling `cl100k_base()` per text
/// re-parses the full BPE vocab (~80ms) and made multi-chunk embedding burn CPU
/// for minutes (rate-limit estimates call this once per input per batch).
pub fn count_tokens(text: &str) -> usize {
    cl100k_base_singleton().encode_ordinary(text).len()
}

/// Estimate tokens for a slice of chat messages.
/// Includes ~4 tokens per message for role/format overhead.
pub fn count_chat_messages(messages: &[crate::ChatMessage]) -> usize {
    let mut total = 0usize;
    for msg in messages {
        total += 4 + count_tokens(&msg.content);
    }
    total
}

/// Count tokens for a system prompt + user query pair (common Chat pattern).
pub fn count_system_and_query(system: &str, query: &str) -> usize {
    count_tokens(system) + 4 + count_tokens(query)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_tokens_english() {
        let text = "Hello world, this is a test sentence.";
        let n = count_tokens(text);
        assert!(n >= 7 && n <= 12, "expected ~9 tokens, got {n}");
    }

    #[test]
    fn count_tokens_chinese() {
        let text = "你好，这是一个测试句子。";
        let n = count_tokens(text);
        assert!(
            n >= 8 && n <= 18,
            "expected ~12 tokens for Chinese, got {n}"
        );
    }

    #[test]
    fn count_chat_messages_includes_overhead() {
        let messages = vec![
            crate::ChatMessage::system("You are a helpful assistant."),
            crate::ChatMessage::user("Hello!"),
        ];
        let n = count_chat_messages(&messages);
        assert!(n > 10, "expected >10 tokens with overhead, got {n}");
    }
}
