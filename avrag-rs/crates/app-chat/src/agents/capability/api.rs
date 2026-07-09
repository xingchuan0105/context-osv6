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
    pub modes: BTreeMap<String, ModeSchema>,
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

/// Schema describing a mode's metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModeSchema {
    #[serde(default)]
    pub id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_tools_used: Vec<String>,
    #[serde(default)]
    pub requires_internet: bool,
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

    let modes: BTreeMap<String, ModeSchema> = registry
        .list_modes()
        .into_iter()
        .map(|s| (s.id.clone(), s.clone()))
        .collect();

    CapabilitiesResponse {
        api_version: "v6".to_string(),
        registry_version: "1.0.0".to_string(),
        tools,
        skills,
        modes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_capabilities_includes_modes() {
        let resp = build_capabilities_response();
        assert_eq!(resp.api_version, "v6");
        assert!(resp.modes.contains_key("chat"));
        assert!(resp.modes.contains_key("rag"));
        assert!(resp.modes.contains_key("search"));
        assert!(resp.modes.contains_key("write"));
    }

    #[test]
    fn search_requires_internet() {
        let resp = build_capabilities_response();
        assert!(resp.modes["search"].requires_internet);
        assert!(!resp.modes["chat"].requires_internet);
        assert!(!resp.modes["rag"].requires_internet);
    }

    #[test]
    fn capabilities_response_serde_roundtrip() {
        let resp = build_capabilities_response();
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: CapabilitiesResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.api_version, "v6");
        assert_eq!(parsed.modes.len(), 4);
    }
}
