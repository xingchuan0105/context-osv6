use avrag_llm::LlmResponse;

#[derive(Debug, Clone)]
pub enum LlmOutput {
    NativeToolCalls(Vec<common::ToolCall>),
    CodeBlocks(Vec<String>),
    Content(String),
}

pub fn parse_llm_output(response: &LlmResponse) -> LlmOutput {
    // 1. Native tool calls
    if let Some(calls) = response.tool_calls.as_ref().filter(|c| !c.is_empty()) {
        return LlmOutput::NativeToolCalls(calls.clone());
    }

    // 2. Extract all <code>...</code> tags
    let code_tags = extract_all_code_tags(&response.content);
    if !code_tags.is_empty() {
        return LlmOutput::CodeBlocks(code_tags);
    }

    // 3. Fallback: markdown code blocks
    let md_blocks = extract_all_markdown_code_blocks(&response.content);
    if !md_blocks.is_empty() {
        return LlmOutput::CodeBlocks(md_blocks);
    }

    // 4. Direct content
    LlmOutput::Content(response.content.clone())
}

fn extract_all_code_tags(content: &str) -> Vec<String> {
    let start_tag = "<code";
    let end_tag = "</code>";
    let mut blocks = Vec::new();
    let mut search_from = 0;

    while let Some(start_idx) = content[search_from..].find(start_tag) {
        let absolute_start = search_from + start_idx;
        let after_start = &content[absolute_start..];
        let Some(tag_end) = after_start.find('>') else { break };
        let code_start = absolute_start + tag_end + 1;
        let remaining = &content[code_start..];
        let Some(code_end) = remaining.find(end_tag) else { break };
        blocks.push(remaining[..code_end].trim().to_string());
        search_from = code_start + code_end + end_tag.len();
    }

    blocks
}

fn extract_all_markdown_code_blocks(content: &str) -> Vec<String> {
    let fence = "```";
    let mut blocks = Vec::new();
    let mut search_from = 0;

    while let Some(start_idx) = content[search_from..].find(fence) {
        let absolute_start = search_from + start_idx;
        let after_fence = &content[absolute_start + fence.len()..];
        let Some(first_newline) = after_fence.find('\n') else { break };
        let code_start = absolute_start + fence.len() + first_newline + 1;
        let remaining = &content[code_start..];
        let Some(end_idx) = remaining.find(fence) else { break };
        blocks.push(remaining[..end_idx].trim().to_string());
        search_from = code_start + end_idx + fence.len();
    }

    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(content: &str) -> LlmResponse {
        LlmResponse {
            content: content.to_string(),
            tool_calls: None,
            usage: avrag_llm::LlmUsage::zeroed(),
            model: String::new(),
        }
    }

    #[test]
    fn extract_multiple_code_tags() {
        let text = "First <code>print(1)</code> then <code>print(2)</code>";
        let blocks = extract_all_code_tags(text);
        assert_eq!(blocks, vec!["print(1)", "print(2)"]);
    }

    #[test]
    fn extract_multiple_markdown_fences() {
        let text = "```python\na = 1\n```\n\n```python\nb = 2\n```";
        let blocks = extract_all_markdown_code_blocks(text);
        assert_eq!(blocks, vec!["a = 1", "b = 2"]);
    }

    #[test]
    fn parse_prefers_code_tags_over_markdown() {
        let text = "<code>x</code>\n\n```\ny\n```";
        match parse_llm_output(&response(text)) {
            LlmOutput::CodeBlocks(blocks) => assert_eq!(blocks, vec!["x"]),
            other => panic!("expected CodeBlocks, got {:?}", other),
        }
    }

    #[test]
    fn parse_multiple_code_blocks() {
        let text = "Step 1: <code>print(2+3)</code>\nStep 2: <code>print(4*5)</code>";
        match parse_llm_output(&response(text)) {
            LlmOutput::CodeBlocks(blocks) => {
                assert_eq!(blocks.len(), 2);
                assert_eq!(blocks[0], "print(2+3)");
                assert_eq!(blocks[1], "print(4*5)");
            }
            other => panic!("expected CodeBlocks, got {:?}", other),
        }
    }

    #[test]
    fn parse_content_when_no_code() {
        let text = "Hello world";
        match parse_llm_output(&response(text)) {
            LlmOutput::Content(c) => assert_eq!(c, "Hello world"),
            other => panic!("expected Content, got {:?}", other),
        }
    }

    #[test]
    fn parse_single_code_tag() {
        let text = "<code>print('hello')</code>";
        match parse_llm_output(&response(text)) {
            LlmOutput::CodeBlocks(blocks) => assert_eq!(blocks, vec!["print('hello')"]),
            other => panic!("expected CodeBlocks, got {:?}", other),
        }
    }
}
