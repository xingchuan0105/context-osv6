//! Capabilities API — GET /agent/capabilities response types and handler.
//!
//! Returns versioned metadata for all registered tools, skills, and strategies.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Response for GET /agent/capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitiesResponse {
    pub api_version: String,
    pub registry_version: String,
    pub tools: Vec<ToolCapability>,
    pub skills: Vec<SkillCapability>,
    pub strategies: BTreeMap<String, StrategySchema>,
}

/// Public representation of a tool capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCapability {
    pub id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<serde_json::Value>,
    pub risk_level: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_deps: Vec<String>,
    #[serde(default)]
    pub deprecated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecation_note: Option<String>,
}

/// Public representation of a skill capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCapability {
    pub id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applicable_strategies: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_tools: Vec<String>,
    pub risk_level: String,
    #[serde(default)]
    pub deprecated: bool,
}

/// Schema describing a strategy's states and transitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategySchema {
    #[serde(default)]
    pub id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<TransitionSchema>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_tools_used: Vec<String>,
    #[serde(default)]
    pub requires_internet: bool,
    pub max_budget: u8,
}

/// A single allowed transition between states.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionSchema {
    pub from: String,
    pub to: String,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// Build the capabilities response from the current registry state.
pub fn build_capabilities_response() -> CapabilitiesResponse {
    let registry = super::CapabilityRegistry::standard_cached();

    let tools = registry
        .list_tools()
        .into_iter()
        .map(|meta| ToolCapability {
            id: meta.id.clone(),
            version: meta.version.clone(),
            description: Some(meta.description.clone()),
            input_schema: Some(meta.input_schema.clone()),
            output_schema: Some(meta.output_schema.clone()),
            risk_level: format!("{:?}", meta.risk_level).to_lowercase(),
            permissions: meta
                .permissions
                .iter()
                .map(|p| format!("{:?}", p).to_lowercase())
                .collect(),
            external_deps: meta.external_deps.clone(),
            deprecated: meta.deprecation.is_some(),
            deprecation_note: meta.deprecation.as_ref().map(|d| d.note.clone()),
        })
        .collect();

    let skills = registry
        .list_skills()
        .into_iter()
        .map(|meta| SkillCapability {
            id: meta.id.clone(),
            version: meta.version.clone(),
            description: Some(meta.description.clone()),
            applicable_strategies: meta.applicable_strategies.clone(),
            required_tools: meta.required_tools.clone(),
            risk_level: format!("{:?}", meta.risk_level).to_lowercase(),
            deprecated: meta.deprecation.is_some(),
        })
        .collect();

    let strategies: BTreeMap<String, StrategySchema> = registry
        .list_strategies()
        .into_iter()
        .map(|s| (s.id.clone(), s.clone()))
        .collect();

    CapabilitiesResponse {
        api_version: "v5".to_string(),
        registry_version: "1.0.0".to_string(),
        tools,
        skills,
        strategies,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_capabilities_includes_strategies() {
        let resp = build_capabilities_response();
        assert_eq!(resp.api_version, "v5");
        assert!(resp.strategies.contains_key("chat"));
        assert!(resp.strategies.contains_key("rag"));
        assert!(resp.strategies.contains_key("search"));
    }

    #[test]
    fn strategy_schemas_have_expected_budgets() {
        let resp = build_capabilities_response();
        assert_eq!(resp.strategies["chat"].max_budget, 1);
        assert_eq!(resp.strategies["rag"].max_budget, 4);
        assert_eq!(resp.strategies["search"].max_budget, 3);
    }

    #[test]
    fn search_requires_internet() {
        let resp = build_capabilities_response();
        assert!(resp.strategies["search"].requires_internet);
        assert!(!resp.strategies["chat"].requires_internet);
        assert!(!resp.strategies["rag"].requires_internet);
    }

    #[test]
    fn capabilities_response_serde_roundtrip() {
        let resp = build_capabilities_response();
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: CapabilitiesResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.api_version, "v5");
        assert_eq!(parsed.strategies.len(), 3);
    }
}
