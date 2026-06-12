/// Single authoritative protocol for LLM skill-body requests (ADR-0007 / A2).
///
/// Accepts only `{"skill_request":["cluster_id",...]}` as the full assistant content
/// (after trim). Embedded-in-prose extraction is intentionally unsupported.
pub fn parse_skill_request(content: &str) -> Vec<String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return Vec::new();
    };
    value
        .get("skill_request")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Filter parsed ids against the mode skill catalog.
pub fn validate_skill_request(mode: &super::config::ModeConfig, content: &str) -> Vec<String> {
    let mut ids = parse_skill_request(content);
    ids.retain(|id| mode.skill_catalog.cluster_by_id(id).is_some());
    ids
}

/// True when the trimmed content is a JSON object containing `skill_request`.
pub fn is_skill_request_message(content: &str) -> bool {
    !parse_skill_request(content).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_json_single_id() {
        assert_eq!(
            parse_skill_request(r#"{"skill_request": ["codegen"]}"#),
            vec!["codegen"]
        );
    }

    #[test]
    fn json_with_extra_fields() {
        assert_eq!(
            parse_skill_request(
                r#"{"thought":"need memory","skill_request":["memory","codegen"]}"#
            ),
            vec!["memory", "codegen"]
        );
    }

    #[test]
    fn multiple_ids() {
        assert_eq!(
            parse_skill_request(r#"{"skill_request":["search","memory"]}"#),
            vec!["search", "memory"]
        );
    }

    #[test]
    fn unknown_ids_parsed_as_is() {
        assert_eq!(
            parse_skill_request(r#"{"skill_request":["unknown_cluster"]}"#),
            vec!["unknown_cluster"]
        );
    }

    #[test]
    fn no_request_returns_empty() {
        assert!(parse_skill_request("just answering").is_empty());
        assert!(parse_skill_request("").is_empty());
    }

    #[test]
    fn malformed_returns_empty() {
        assert!(parse_skill_request(r#"{"skill_request": "codegen"}"#).is_empty());
        assert!(parse_skill_request(r#"{"skill_request": [1, 2]}"#).is_empty());
        assert!(parse_skill_request(r#"not json at all"#).is_empty());
    }

    #[test]
    fn embedded_json_in_prose_is_unsupported() {
        assert!(parse_skill_request(
            "I need memory context.\n{\"skill_request\":[\"memory\"]}"
        )
        .is_empty());
    }

    #[test]
    fn is_skill_request_message_detects_json_only() {
        assert!(is_skill_request_message(r#"{"skill_request":["memory"]}"#));
        assert!(!is_skill_request_message("plain answer"));
    }

    #[test]
    fn validate_filters_unknown_clusters() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let ids = validate_skill_request(&mode, r#"{"skill_request":["codegen","bogus"]}"#);
        assert_eq!(ids, vec!["codegen"]);
    }
}
