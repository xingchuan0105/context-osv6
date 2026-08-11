//! Lead plan-time context blocks (design §13.2).
//!
//! Host assembles these for the Lead planning turn. No retrieval hit bodies.

use serde::{Deserialize, Serialize};

/// One document in scope (title/short profile only — not full text).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocScopeSummary {
    pub doc_id: String,
    #[serde(default)]
    pub title: String,
    /// Optional short profile line (from docscope skill / metadata).
    #[serde(default)]
    pub profile_line: String,
}

/// What LeadPlan sees beyond raw user message + history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LeadPlanContext {
    pub caps_rag: bool,
    pub caps_search: bool,
    /// Empty vec + `rag` active ⇒ host states「本轮无挂载文档」in observation.
    #[serde(default)]
    pub doc_scope: Vec<DocScopeSummary>,
    #[serde(default)]
    pub workspace_id: Option<String>,
}

impl LeadPlanContext {
    pub fn has_docs(&self) -> bool {
        !self.doc_scope.is_empty()
    }

    /// Compact markdown block for host observation (model channel).
    /// Placeholders filled by caller into `prompts/loop/lead-plan-context.tmpl.md`.
    pub fn doc_lines(&self) -> String {
        if self.doc_scope.is_empty() {
            return String::new();
        }
        self.doc_scope
            .iter()
            .map(|d| {
                let title = if d.title.trim().is_empty() {
                    "(untitled)"
                } else {
                    d.title.trim()
                };
                if d.profile_line.trim().is_empty() {
                    format!("- `{}` {title}", d.doc_id)
                } else {
                    format!(
                        "- `{}` {title} — {}",
                        d.doc_id,
                        d.profile_line.trim()
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_lines_empty() {
        let c = LeadPlanContext {
            caps_rag: true,
            caps_search: false,
            doc_scope: vec![],
            workspace_id: None,
        };
        assert!(!c.has_docs());
        assert!(c.doc_lines().is_empty());
    }

    #[test]
    fn doc_lines_format() {
        let c = LeadPlanContext {
            caps_rag: true,
            caps_search: true,
            doc_scope: vec![DocScopeSummary {
                doc_id: "d1".into(),
                title: "Report".into(),
                profile_line: "12 pages".into(),
            }],
            workspace_id: Some("ws".into()),
        };
        let lines = c.doc_lines();
        assert!(lines.contains("`d1`"));
        assert!(lines.contains("Report"));
        assert!(lines.contains("12 pages"));
    }
}
