/// Single authoritative protocol for LLM skill-body requests (ADR-0007 / A2).
///
/// Accepts only `{"skill_request":["token",...]}` as the full assistant content
/// (after trim). Tokens are either a cluster id (`knowledge-base`, `memory`) or a
/// progressive reference under a cluster (`knowledge-base/how-to-read-tables`).
/// Embedded-in-prose extraction is intentionally unsupported.
/// C6: a ```json / ``` fenced payload is unwrapped via the shared stripper
/// first — a fenced skill request previously fell through unrecognized and
/// leaked into the final answer.
///
/// Token forms:
/// - `cluster` — load cluster body (if not already disclosed)
/// - `cluster/ref-slug` or `cluster:ref-slug` — load body (if needed) + one
///   `reference/<slug>.md` spoke under that cluster

/// One skill_request token after split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillRequestToken {
    pub cluster_id: String,
    /// Reference slug without `.md` (e.g. `how-to-read-tables`).
    pub reference: Option<String>,
}

impl SkillRequestToken {
    /// Canonical storage form for `last_skill_request` / disclosure keys.
    pub fn as_token(&self) -> String {
        match &self.reference {
            Some(r) => format!("{}/{}", self.cluster_id, r),
            None => self.cluster_id.clone(),
        }
    }
}

/// Legacy cluster id from pre-rename skill packs (still accepted in skill_request).
fn alias_cluster_id(cluster: &str) -> &str {
    match cluster {
        "codegen" => "knowledge-base",
        other => other,
    }
}

/// Split `knowledge-base/how-to-read-tables` or `codegen:how-to-read-tables` or bare id.
pub fn split_skill_request_token(raw: &str) -> SkillRequestToken {
    let raw = raw.trim();
    if let Some((cluster, rest)) = raw.split_once('/') {
        let slug = rest.trim().trim_end_matches(".md");
        if !cluster.is_empty() && !slug.is_empty() {
            return SkillRequestToken {
                cluster_id: alias_cluster_id(cluster).to_string(),
                reference: Some(slug.to_string()),
            };
        }
    }
    if let Some((cluster, rest)) = raw.split_once(':') {
        let slug = rest.trim().trim_end_matches(".md");
        if !cluster.is_empty() && !slug.is_empty() {
            return SkillRequestToken {
                cluster_id: alias_cluster_id(cluster).to_string(),
                reference: Some(slug.to_string()),
            };
        }
    }
    SkillRequestToken {
        cluster_id: alias_cluster_id(raw).to_string(),
        reference: None,
    }
}

pub fn parse_skill_request(content: &str) -> Vec<String> {
    let trimmed = super::json_fence::strip_json_fence(content);
    if trimmed.is_empty() {
        return Vec::new();
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&trimmed) else {
        return Vec::new();
    };
    value
        .get("skill_request")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| split_skill_request_token(s).as_token()))
                .collect()
        })
        .unwrap_or_default()
}

/// Filter parsed tokens against the mode skill catalog (and optional reference
/// spokes on the cluster skill).
pub fn validate_skill_request(mode: &super::config::ModeConfig, content: &str) -> Vec<String> {
    let ids = parse_skill_request(content);
    let registry = agent_tools::progressive::PromptRegistry::standard_cached();
    ids.into_iter()
        .filter(|raw| {
            let tok = split_skill_request_token(raw);
            if mode.skill_catalog.cluster_by_id(&tok.cluster_id).is_none() {
                return false;
            }
            match &tok.reference {
                None => true,
                Some(slug) => {
                    let Some(skill) = registry.skill(&tok.cluster_id) else {
                        return false;
                    };
                    let key = if slug.ends_with(".md") {
                        slug.clone()
                    } else {
                        format!("{slug}.md")
                    };
                    skill.references().contains_key(&key)
                }
            }
        })
        .map(|raw| split_skill_request_token(&raw).as_token())
        .collect()
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
            parse_skill_request(r#"{"skill_request": ["knowledge-base"]}"#),
            vec!["knowledge-base"]
        );
    }

    #[test]
    fn json_with_extra_fields() {
        assert_eq!(
            parse_skill_request(
                r#"{"thought":"need memory","skill_request":["memory","knowledge-base"]}"#
            ),
            vec!["memory", "knowledge-base"]
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
    fn tokens_are_normalized_trim_and_alias() {
        // 输出为规范化后的 token:trim、去 .md、codegen→knowledge-base。
        assert_eq!(
            parse_skill_request(r#"{"skill_request": [" codegen "]}"#),
            vec!["knowledge-base"]
        );
        assert_eq!(
            parse_skill_request(r#"{"skill_request": ["codegen:how-to-read-tables.md"]}"#),
            vec!["knowledge-base/how-to-read-tables"]
        );
    }

    #[test]
    fn malformed_returns_empty() {
        assert!(parse_skill_request(r#"{"skill_request": "knowledge-base"}"#).is_empty());
        assert!(parse_skill_request(r#"{"skill_request": [1, 2]}"#).is_empty());
        assert!(parse_skill_request(r#"not json at all"#).is_empty());
    }

    #[test]
    fn embedded_json_in_prose_is_unsupported() {
        assert!(
            parse_skill_request("I need memory context.\n{\"skill_request\":[\"memory\"]}")
                .is_empty()
        );
    }

    #[test]
    fn is_skill_request_message_detects_json_only() {
        assert!(is_skill_request_message(r#"{"skill_request":["memory"]}"#));
        assert!(!is_skill_request_message("plain answer"));
    }

    #[test]
    fn fenced_skill_request_is_recognized() {
        assert_eq!(
            parse_skill_request("```json\n{\"skill_request\":[\"memory\"]}\n```"),
            vec!["memory"]
        );
        assert_eq!(
            parse_skill_request("```\n{\"skill_request\":[\"codegen\"]}\n```"),
            vec!["knowledge-base"]
        );
        assert!(is_skill_request_message(
            "```json\n{\"skill_request\":[\"memory\"]}\n```"
        ));
    }

    #[test]
    fn validate_filters_unknown_clusters() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let ids = validate_skill_request(&mode, r#"{"skill_request":["knowledge-base","bogus"]}"#);
        assert_eq!(ids, vec!["knowledge-base"]);
    }

    #[test]
    fn split_cluster_ref_slash_and_colon() {
        let a = split_skill_request_token("knowledge-base/how-to-read-tables");
        assert_eq!(a.cluster_id, "knowledge-base");
        assert_eq!(a.reference.as_deref(), Some("how-to-read-tables"));
        assert_eq!(a.as_token(), "knowledge-base/how-to-read-tables");

        let b = split_skill_request_token("codegen:how-to-read-tables.md");
        assert_eq!(b.cluster_id, "knowledge-base");
        assert_eq!(b.reference.as_deref(), Some("how-to-read-tables"));
    }

    #[test]
    fn validate_accepts_codegen_table_reference() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let ids = validate_skill_request(
            &mode,
            r#"{"skill_request":["knowledge-base/how-to-read-tables","codegen/nope"]}"#,
        );
        assert_eq!(ids, vec!["knowledge-base/how-to-read-tables"]);
    }
}
