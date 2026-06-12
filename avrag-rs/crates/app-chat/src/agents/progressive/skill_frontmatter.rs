//! Lightweight YAML-like frontmatter parser for Skill prompt files.
//!
//! Perplexity-style SKILL.md frontmatter:
//! ```text
//! ---
//! name: skill-name
//! description: "Load when ..."
//! version: "1.0"
//! depends: ["other_skill"]
//! ---
//! <body>
//! ```
//!
//! We parse this manually (no serde_yaml dependency) because the
//! schema is tiny and fixed: name, description, version, depends.

use std::collections::HashMap;

/// Parsed frontmatter from a skill file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkillFrontmatter {
    pub name: String,
    pub description: String,
    pub version: String,
    pub depends: Vec<String>,
    /// Extra metadata fields (author, category, etc.)
    pub metadata: HashMap<String, String>,
}

impl SkillFrontmatter {}

/// Parse a skill file that may contain YAML-like frontmatter.
///
/// Returns `(frontmatter, body)`.
/// If no frontmatter is found, returns `(None, content)`.
pub fn parse_skill_file(content: &str) -> (Option<SkillFrontmatter>, String) {
    let trimmed = content.trim_start();

    // Must start with "---" followed by newline (or end of string)
    if !trimmed.starts_with("---") {
        return (None, content.to_string());
    }

    let after_open = &trimmed[3..];
    let after_open = after_open.strip_prefix('\r').unwrap_or(after_open);
    let after_open = after_open.strip_prefix('\n').unwrap_or(after_open);

    // Find closing "---"
    let Some(close_idx) = after_open.find("\n---") else {
        return (None, content.to_string());
    };

    let yaml_block = &after_open[..close_idx];
    let body_start = close_idx + 4; // skip "\n---"
    let body = after_open[body_start..].trim_start().to_string();

    let fm = parse_yaml_block(yaml_block);
    (Some(fm), body)
}

fn parse_yaml_block(block: &str) -> SkillFrontmatter {
    let mut fm = SkillFrontmatter::default();

    for line in block.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((key, raw_value)) = line.split_once(':') else {
            continue;
        };

        let key = key.trim();
        let raw_value = raw_value.trim();

        match key {
            "name" => fm.name = strip_quotes(raw_value).to_string(),
            "description" => fm.description = strip_quotes(raw_value).to_string(),
            "version" => fm.version = strip_quotes(raw_value).to_string(),
            "depends" => fm.depends = parse_string_array(raw_value),
            other => {
                fm.metadata
                    .insert(other.to_string(), strip_quotes(raw_value).to_string());
            }
        }
    }

    fm
}

/// Remove surrounding single or double quotes.
fn strip_quotes(s: &str) -> &str {
    let s = s.trim();
    if s.len() >= 2 {
        let first = s.chars().next().unwrap();
        let last = s.chars().last().unwrap();
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            return &s[1..s.len() - 1];
        }
    }
    s
}

/// Parse a JSON-like string array: `["a", "b"]` or `[]`.
fn parse_string_array(s: &str) -> Vec<String> {
    let s = s.trim();
    if !s.starts_with('[') || !s.ends_with(']') {
        return Vec::new();
    }
    let inner = &s[1..s.len() - 1];
    if inner.trim().is_empty() {
        return Vec::new();
    }

    inner
        .split(',')
        .map(|item| strip_quotes(item.trim()).to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_frontmatter() {
        let content = r#"---
name: rag_plan
description: "Load when retrieving evidence."
version: "1.0"
depends: ["rag-execute"]
---
You are the planner.
"#;
        let (fm, body) = parse_skill_file(content);
        let fm = fm.unwrap();
        assert_eq!(fm.name, "rag_plan");
        assert_eq!(fm.description, "Load when retrieving evidence.");
        assert_eq!(fm.version, "1.0");
        assert_eq!(fm.depends, vec!["rag-execute"]);
        assert_eq!(body.trim(), "You are the planner.");
    }

    #[test]
    fn parse_empty_depends() {
        let content = r#"---
name: chat
description: Load when chatting.
version: "1.0"
depends: []
---
Body here.
"#;
        let (fm, _) = parse_skill_file(content);
        assert!(fm.unwrap().depends.is_empty());
    }

    #[test]
    fn parse_no_frontmatter() {
        let content = "You are the planner.\n";
        let (fm, body) = parse_skill_file(content);
        assert!(fm.is_none());
        assert_eq!(body, content);
    }

    #[test]
    fn parse_multiple_depends() {
        let content = r#"---
name: rag_answer
description: "Load when answering."
depends: ["citation_rules", "rag-plan"]
---
Body.
"#;
        let (fm, _) = parse_skill_file(content);
        assert_eq!(fm.unwrap().depends, vec!["citation_rules", "rag-plan"]);
    }

    #[test]
    fn parse_metadata_extra_fields() {
        let content = r#"---
name: test
description: "Load when testing."
author: "team-rag"
category: "planner"
---
Body.
"#;
        let (fm, _) = parse_skill_file(content);
        let fm = fm.unwrap();
        assert_eq!(fm.metadata.get("author").unwrap(), "team-rag");
        assert_eq!(fm.metadata.get("category").unwrap(), "planner");
    }
}
