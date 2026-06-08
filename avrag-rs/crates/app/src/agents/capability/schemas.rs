//! Static strategy schemas for CapabilityRegistry.
//!
//! Decoupled from the deprecated strategy runtime layer so registry metadata
//! can be served without pulling in strategy executors.

use super::{StrategySchema, TransitionSchema};

/// Chat strategy: Plan → [ExecuteAtomic] → Answer.
pub fn chat_schema() -> StrategySchema {
    StrategySchema {
        id: "chat".to_string(),
        states: vec![
            "Plan".to_string(),
            "ExecuteAtomic".to_string(),
            "Answer".to_string(),
        ],
        transitions: vec![
            TransitionSchema {
                from: "Plan".to_string(),
                to: "ExecuteAtomic".to_string(),
            },
            TransitionSchema {
                from: "Plan".to_string(),
                to: "Answer".to_string(),
            },
            TransitionSchema {
                from: "ExecuteAtomic".to_string(),
                to: "Answer".to_string(),
            },
        ],
        external_tools_used: vec![],
        requires_internet: false,
        max_budget: 1,
    }
}

/// RAG strategy: Plan → ExecuteRetrieve → Answer (with optional replan loop).
pub fn rag_schema() -> StrategySchema {
    StrategySchema {
        id: "rag".to_string(),
        states: vec![
            "Plan".to_string(),
            "ExecuteRetrieve".to_string(),
            "Answer".to_string(),
        ],
        transitions: vec![
            TransitionSchema {
                from: "Plan".to_string(),
                to: "ExecuteRetrieve".to_string(),
            },
            TransitionSchema {
                from: "Plan".to_string(),
                to: "Answer".to_string(),
            },
            TransitionSchema {
                from: "ExecuteRetrieve".to_string(),
                to: "Plan".to_string(),
            },
            TransitionSchema {
                from: "ExecuteRetrieve".to_string(),
                to: "Answer".to_string(),
            },
        ],
        external_tools_used: vec![],
        requires_internet: false,
        max_budget: 4,
    }
}

/// Search strategy: Decompose → ParallelSearch → Aggregate → Answer.
pub fn search_schema() -> StrategySchema {
    StrategySchema {
        id: "search".to_string(),
        states: vec![
            "Decompose".to_string(),
            "ParallelSearch".to_string(),
            "Aggregate".to_string(),
            "Answer".to_string(),
        ],
        transitions: vec![
            TransitionSchema {
                from: "Decompose".to_string(),
                to: "ParallelSearch".to_string(),
            },
            TransitionSchema {
                from: "ParallelSearch".to_string(),
                to: "Aggregate".to_string(),
            },
            TransitionSchema {
                from: "Aggregate".to_string(),
                to: "Answer".to_string(),
            },
        ],
        external_tools_used: vec!["web_search".to_string()],
        requires_internet: true,
        max_budget: 3,
    }
}

/// All built-in strategy schemas (chat, rag, search).
pub fn standard_strategy_schemas() -> Vec<StrategySchema> {
    vec![chat_schema(), rag_schema(), search_schema()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_schemas_match_registry_expectations() {
        let schemas = standard_strategy_schemas();
        assert_eq!(schemas.len(), 3);
        assert!(schemas.iter().any(|s| s.id == "chat"));
        assert!(schemas.iter().any(|s| s.id == "rag"));
        assert!(schemas.iter().any(|s| s.id == "search"));
    }

    #[test]
    fn chat_schema_matches_state_machine() {
        let schema = chat_schema();
        assert_eq!(schema.states, vec!["Plan", "ExecuteAtomic", "Answer"]);
        assert_eq!(schema.max_budget, 1);
        assert!(!schema.requires_internet);
    }

    #[test]
    fn rag_schema_matches_state_machine() {
        let schema = rag_schema();
        assert_eq!(schema.states, vec!["Plan", "ExecuteRetrieve", "Answer"]);
        assert_eq!(schema.max_budget, 4);
    }

    #[test]
    fn search_schema_matches_state_machine() {
        let schema = search_schema();
        assert_eq!(
            schema.states,
            vec!["Decompose", "ParallelSearch", "Aggregate", "Answer"]
        );
        assert!(schema.requires_internet);
    }
}
