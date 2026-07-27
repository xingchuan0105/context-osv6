//! Shared JSON fence stripper (C6): single implementation for peeling a
//! leading markdown fence off model output before JSON parsing.
//!
//! Semantics (union of the previous scattered copies in `answer_contract.rs`
//! and app-chat `workers.rs`): when the trimmed text starts with a ``` fence,
//! drop the entire fence TAG LINE (whatever the tag: `json`, `python`, empty),
//! then take everything up to the LAST closing ``` if one exists, otherwise
//! take the rest of the text (unterminated fence tolerated). Text that does
//! not start with a fence is returned unchanged (trimmed). Leading prose
//! before the fence is NOT supported by design — parsers here require the
//! whole message to be the payload.

/// Strip a leading markdown fence (and its tag line) from `raw`.
///
/// ```
/// use agent_loop::r#loop::json_fence::strip_json_fence;
/// assert_eq!(strip_json_fence("```json\n{\"a\":1}\n```"), "{\"a\":1}");
/// assert_eq!(strip_json_fence("{\"a\":1}"), "{\"a\":1}");
/// ```
pub fn strip_json_fence(raw: &str) -> String {
    let trimmed = raw.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }
    // Drop the fence tag line entirely (`json`, `python`, empty, anything) —
    // the tag never belongs to the payload.
    let Some(first_nl) = trimmed.find('\n') else {
        // Fence tag with no body at all.
        return String::new();
    };
    let rest = &trimmed[first_nl + 1..];
    if let Some(end) = rest.rfind("```") {
        rest[..end].trim().to_string()
    } else {
        // Unterminated fence: take the rest (workers.rs legacy behavior).
        rest.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_json_tagged_fence() {
        assert_eq!(strip_json_fence("```json\n{\"a\": 1}\n```"), "{\"a\": 1}");
    }

    #[test]
    fn strips_bare_fence() {
        assert_eq!(strip_json_fence("```\n{\"a\": 1}\n```"), "{\"a\": 1}");
    }

    #[test]
    fn strips_any_tag_line_not_just_json() {
        assert_eq!(strip_json_fence("```python\nx = 1\n```"), "x = 1");
    }

    #[test]
    fn tolerates_unterminated_fence() {
        assert_eq!(strip_json_fence("```json\n{\"a\": 1}"), "{\"a\": 1}");
    }

    #[test]
    fn drops_trailing_prose_after_closing_fence() {
        assert_eq!(
            strip_json_fence("```json\n{\"a\": 1}\n```\n以上是结果"),
            "{\"a\": 1}"
        );
    }

    #[test]
    fn passes_through_unfenced_text() {
        assert_eq!(strip_json_fence("{\"a\": 1}"), "{\"a\": 1}");
        assert_eq!(strip_json_fence("  plain answer  "), "plain answer");
        assert_eq!(strip_json_fence(""), "");
    }

    #[test]
    fn fence_without_body_is_empty() {
        assert_eq!(strip_json_fence("```json"), "");
    }
}
