//! Resolve product chat capabilities from request fields (multiselect + legacy agent_type).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CapabilitySet {
    pub rag: bool,
    pub search: bool,
}

impl CapabilitySet {
    pub fn is_pure_chat(&self) -> bool {
        !self.rag && !self.search
    }

    /// Derived wire/telemetry label.
    pub fn agent_type_label(&self) -> &'static str {
        match (self.rag, self.search) {
            (false, false) => "chat",
            (true, false) => "rag",
            (false, true) => "search",
            (true, true) => "rag+search",
        }
    }

    pub fn as_string_list(&self) -> Vec<String> {
        let mut v = Vec::new();
        if self.rag {
            v.push("rag".into());
        }
        if self.search {
            v.push("search".into());
        }
        v
    }
}

/// Error when product write is requested.
pub fn write_disabled_error() -> common::AppError {
    common::AppError::validation(
        "write_mode_disabled",
        "Writing mode is no longer available. Use chat with optional RAG/Search capabilities.",
    )
}

/// Resolve capabilities from request fields.
/// - If `capabilities` is `Some`, normalize (rag/search only, dedupe) and **ignore** agent_type for caps.
/// - If `capabilities` is `None`, map legacy agent_type.
/// - `write` / `write_refine` → Err.
pub fn resolve_capabilities(
    capabilities: Option<&[String]>,
    agent_type: &str,
) -> Result<CapabilitySet, common::AppError> {
    let at = agent_type.trim();
    if at.eq_ignore_ascii_case("write") || at.eq_ignore_ascii_case("write_refine") {
        return Err(write_disabled_error());
    }

    if let Some(list) = capabilities {
        let mut set = CapabilitySet::default();
        for raw in list {
            match raw.trim().to_ascii_lowercase().as_str() {
                "rag" => set.rag = true,
                "search" => set.search = true,
                _ => {} // ignore unknown
            }
        }
        return Ok(set);
    }

    match at.to_ascii_lowercase().as_str() {
        "rag" => Ok(CapabilitySet {
            rag: true,
            search: false,
        }),
        "search" => Ok(CapabilitySet {
            rag: false,
            search: true,
        }),
        "chat" | "general" | "" => Ok(CapabilitySet::default()),
        "write" | "write_refine" => Err(write_disabled_error()),
        _ => Ok(CapabilitySet::default()), // unknown legacy → pure chat
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_capabilities_is_pure_chat() {
        let s = resolve_capabilities(Some(&[]), "rag").unwrap();
        assert!(s.is_pure_chat());
        assert_eq!(s.agent_type_label(), "chat");
    }

    #[test]
    fn capabilities_win_over_agent_type() {
        let list = vec!["search".into()];
        let s = resolve_capabilities(Some(&list), "rag").unwrap();
        assert!(!s.rag && s.search);
    }

    #[test]
    fn legacy_rag_maps() {
        let s = resolve_capabilities(None, "rag").unwrap();
        assert!(s.rag && !s.search);
    }

    #[test]
    fn write_rejected() {
        let err = resolve_capabilities(None, "write").unwrap_err();
        assert!(
            err.to_string().contains("write") || format!("{err:?}").contains("write_mode_disabled")
        );
    }

    #[test]
    fn dual_label() {
        let list = vec!["rag".into(), "search".into(), "nope".into()];
        let s = resolve_capabilities(Some(&list), "chat").unwrap();
        assert_eq!(s.agent_type_label(), "rag+search");
        assert_eq!(s.as_string_list(), vec!["rag", "search"]);
    }
}
