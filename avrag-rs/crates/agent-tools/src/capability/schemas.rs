//! Static mode schemas for CapabilityRegistry.
//!
//! Decoupled from the deprecated strategy runtime layer so registry metadata
//! can be served without pulling in strategy executors.

use super::ModeSchema;

/// Chat mode schema.
///
/// Runtime always exposes base skill `user_context` (see `mode_assemble`); listed
/// here so capability metadata matches the product tool surface.
pub fn chat_mode_schema() -> ModeSchema {
    ModeSchema {
        id: "chat".to_string(),
        tool_pool: vec!["user_context".to_string()],
        external_tools_used: vec![],
        requires_internet: false,
    }
}

/// RAG mode schema.
pub fn rag_mode_schema() -> ModeSchema {
    ModeSchema {
        id: "rag".to_string(),
        tool_pool: vec![],
        external_tools_used: vec![],
        requires_internet: false,
    }
}

/// Search mode schema.
pub fn search_mode_schema() -> ModeSchema {
    ModeSchema {
        id: "search".to_string(),
        tool_pool: vec![],
        external_tools_used: vec!["web_search".to_string()],
        requires_internet: true,
    }
}

/// Write mode schema (residual registry entry only).
///
/// Product `ConversationApp` rejects `agent_type=write` with `write_mode_disabled`
/// (2026-07-15 offline). Kept so historical registry metadata / tests stay stable.
pub fn write_mode_schema() -> ModeSchema {
    ModeSchema {
        id: "write".to_string(),
        tool_pool: vec![],
        external_tools_used: vec!["web_search".to_string()],
        requires_internet: true,
    }
}

/// All built-in mode schemas (chat, rag, search; write residual / product-offline).
pub fn standard_mode_schemas() -> Vec<ModeSchema> {
    vec![
        chat_mode_schema(),
        rag_mode_schema(),
        search_mode_schema(),
        write_mode_schema(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_schemas_match_registry_expectations() {
        let schemas = standard_mode_schemas();
        assert_eq!(schemas.len(), 4);
        assert!(schemas.iter().any(|s| s.id == "chat"));
        assert!(schemas.iter().any(|s| s.id == "rag"));
        assert!(schemas.iter().any(|s| s.id == "search"));
        assert!(schemas.iter().any(|s| s.id == "write"));
    }

    #[test]
    fn chat_mode_schema_has_expected_metadata() {
        let schema = chat_mode_schema();
        assert_eq!(schema.id, "chat");
        assert!(!schema.requires_internet);
        assert!(schema.tool_pool.iter().any(|t| t == "user_context"));
    }

    #[test]
    fn rag_mode_schema_has_expected_metadata() {
        let schema = rag_mode_schema();
        assert_eq!(schema.id, "rag");
    }

    #[test]
    fn search_mode_schema_has_expected_metadata() {
        let schema = search_mode_schema();
        assert_eq!(schema.id, "search");
        assert!(schema.requires_internet);
    }

    #[test]
    fn write_mode_schema_has_expected_metadata() {
        let schema = write_mode_schema();
        assert_eq!(schema.id, "write");
        assert!(schema.requires_internet);
        assert_eq!(schema.external_tools_used, vec!["web_search".to_string()]);
    }
}
