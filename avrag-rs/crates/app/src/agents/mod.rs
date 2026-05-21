use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentKind {
    Chat,
    Rag,
    Search,
    Composite,
}

impl fmt::Display for AgentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentKind::Chat => write!(f, "chat"),
            AgentKind::Rag => write!(f, "rag"),
            AgentKind::Search => write!(f, "search"),
            AgentKind::Composite => write!(f, "composite"),
        }
    }
}

impl AgentKind {
    /// Parse agent type string into canonical AgentKind.
    /// `general` is accepted as a compatibility alias for `Chat`.
    pub fn parse(agent_type: &str) -> Option<Self> {
        match agent_type.to_ascii_lowercase().as_str() {
            "chat" | "general" => Some(AgentKind::Chat),
            "rag" => Some(AgentKind::Rag),
            "search" => Some(AgentKind::Search),
            "composite" => Some(AgentKind::Composite),
            _ => None,
        }
    }

    /// Return the canonical string representation.
    pub fn as_canonical_str(&self) -> &'static str {
        match self {
            AgentKind::Chat => "chat",
            AgentKind::Rag => "rag",
            AgentKind::Search => "search",
            AgentKind::Composite => "composite",
        }
    }
}

pub mod audit;
pub mod capability;
pub mod content_guard;
pub mod error_kind;
pub mod eval_framework;
pub mod evaluator;
pub mod events;
pub mod progressive;
pub mod react_loop;
pub mod redteam;
pub mod replay;
pub mod rig_adapter;
pub mod runtime;
pub mod service;
pub mod skills;
pub mod sse_sink;
pub mod strategy;
pub mod unified;
pub mod untrusted_input;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_kind_parse_chat() {
        assert_eq!(AgentKind::parse("chat"), Some(AgentKind::Chat));
    }

    #[test]
    fn test_agent_kind_parse_general_alias() {
        assert_eq!(AgentKind::parse("general"), Some(AgentKind::Chat));
        assert_eq!(AgentKind::parse("GENERAL"), Some(AgentKind::Chat));
        assert_eq!(AgentKind::parse("General"), Some(AgentKind::Chat));
    }

    #[test]
    fn test_agent_kind_parse_rag() {
        assert_eq!(AgentKind::parse("rag"), Some(AgentKind::Rag));
        assert_eq!(AgentKind::parse("RAG"), Some(AgentKind::Rag));
    }

    #[test]
    fn test_agent_kind_parse_search() {
        assert_eq!(AgentKind::parse("search"), Some(AgentKind::Search));
        assert_eq!(AgentKind::parse("SEARCH"), Some(AgentKind::Search));
    }

    #[test]
    fn test_agent_kind_parse_composite() {
        assert_eq!(AgentKind::parse("composite"), Some(AgentKind::Composite));
        assert_eq!(AgentKind::parse("COMPOSITE"), Some(AgentKind::Composite));
    }

    #[test]
    fn test_agent_kind_parse_unknown() {
        assert_eq!(AgentKind::parse("unknown"), None);
        assert_eq!(AgentKind::parse(""), None);
    }

    #[test]
    fn test_agent_kind_canonical_str() {
        assert_eq!(AgentKind::Chat.as_canonical_str(), "chat");
        assert_eq!(AgentKind::Rag.as_canonical_str(), "rag");
        assert_eq!(AgentKind::Search.as_canonical_str(), "search");
        assert_eq!(AgentKind::Composite.as_canonical_str(), "composite");
    }

    #[test]
    fn test_agent_kind_display() {
        assert_eq!(AgentKind::Chat.to_string(), "chat");
        assert_eq!(AgentKind::Rag.to_string(), "rag");
        assert_eq!(AgentKind::Search.to_string(), "search");
        assert_eq!(AgentKind::Composite.to_string(), "composite");
    }

    #[test]
    fn test_agent_kind_serde_roundtrip() {
        for kind in [AgentKind::Chat, AgentKind::Rag, AgentKind::Search, AgentKind::Composite] {
            let json = serde_json::to_string(&kind).unwrap();
            let parsed: AgentKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, parsed);
        }
    }
}
