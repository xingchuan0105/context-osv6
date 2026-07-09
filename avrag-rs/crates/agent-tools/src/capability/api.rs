//! Capabilities API — GET /agent/capabilities response types and handler.
//!
//! Returns versioned metadata for skills, modes, and tools disclosed via each
//! mode's `tool_pool` (ADR-0006 §5a: Capability surface, not full catalog dump).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Response for GET /agent/capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitiesResponse {
    pub api_version: String,
    pub registry_version: String,
    /// Union of tools listed in product modes' `tool_pool` (catalog meta only).
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
    /// Retrieve-phase tool ids from mode YAML (`tool_pool`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_pool: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_tools_used: Vec<String>,
    #[serde(default)]
    pub requires_internet: bool,
}

// ---------------------------------------------------------------------------
// Mode YAML tool_pool (product modes only)
// ---------------------------------------------------------------------------

/// Product mode id → modes/*.yaml file stem (write uses write_refine config).
const PRODUCT_MODE_FILES: &[(&str, &str)] = &[
    ("chat", "chat"),
    ("rag", "rag"),
    ("search", "search"),
    ("write", "write_refine"),
];

#[derive(Debug, Deserialize)]
struct ModeYamlToolPool {
    #[serde(default)]
    tool_pool: Vec<String>,
    /// RAG (and others) may keep `tool_pool` empty for on-demand skill disclosure
    /// while still auto-invoking a retrieval tool — include it in product capability list.
    #[serde(default)]
    auto_fallback: Option<ModeYamlAutoFallback>,
}

#[derive(Debug, Deserialize)]
struct ModeYamlAutoFallback {
    #[serde(default)]
    tool_id: Option<String>,
}

fn resolve_mode_yaml_path(file_stem: &str) -> Option<std::path::PathBuf> {
    let rel = format!("modes/{file_stem}.yaml");
    let candidates = [
        std::path::PathBuf::from(&rel),
        std::env::var("CARGO_MANIFEST_DIR")
            .ok()
            .map(|m| std::path::PathBuf::from(m).join("../..").join(&rel))
            .unwrap_or_default(),
    ];
    for path in candidates {
        if path.is_file() {
            return Some(path);
        }
    }
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let check = dir.join("modes").join(format!("{file_stem}.yaml"));
        if check.is_file() {
            return Some(check);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

fn parse_mode_yaml(file_stem: &str) -> Option<ModeYamlToolPool> {
    let path = resolve_mode_yaml_path(file_stem)?;
    let content = std::fs::read_to_string(&path).ok()?;
    serde_yaml::from_str(&content).ok()
}

/// YAML `tool_pool` only (ModeConfig retrieve disclosure; may be empty for RAG).
fn load_mode_tool_pool(file_stem: &str) -> Vec<String> {
    parse_mode_yaml(file_stem)
        .map(|m| m.tool_pool)
        .unwrap_or_default()
}

/// Product capability disclosure for a mode: `tool_pool` ∪ `auto_fallback.tool_id`.
fn load_mode_disclosed_tools(file_stem: &str) -> Vec<String> {
    let Some(m) = parse_mode_yaml(file_stem) else {
        return vec![];
    };
    let mut ids = m.tool_pool;
    if let Some(fb) = m.auto_fallback.and_then(|a| a.tool_id) {
        if !fb.is_empty() && !ids.iter().any(|x| x == &fb) {
            ids.push(fb);
        }
    }
    ids
}

/// Union of disclosed tool ids across product modes (chat/rag/search/write).
pub fn product_mode_tool_pool_union() -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for (_, file_stem) in PRODUCT_MODE_FILES {
        for id in load_mode_disclosed_tools(file_stem) {
            ids.insert(id);
        }
    }
    ids
}

fn product_mode_yaml_tool_pools() -> BTreeMap<String, Vec<String>> {
    PRODUCT_MODE_FILES
        .iter()
        .map(|(mode_id, file_stem)| ((*mode_id).to_string(), load_mode_tool_pool(file_stem)))
        .collect()
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// Build the capabilities response from the current registry state.
///
/// Tools listed are **only** those appearing in at least one product mode's
/// `tool_pool` (not the full executable catalog).
pub fn build_capabilities_response() -> CapabilitiesResponse {
    let registry = super::CapabilityRegistry::standard_cached();
    let pools = product_mode_yaml_tool_pools();
    let allowed = product_mode_tool_pool_union();

    let mut tools: Vec<ToolCapability> = allowed
        .iter()
        .filter_map(|id| registry.tool(id))
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
    tools.sort_by(|a, b| a.id.cmp(&b.id));

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
        .map(|s| {
            let tool_pool = pools.get(&s.id).cloned().unwrap_or_default();
            (
                s.id.clone(),
                ModeSchema {
                    id: s.id.clone(),
                    tool_pool,
                    external_tools_used: s.external_tools_used.clone(),
                    requires_internet: s.requires_internet,
                },
            )
        })
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

    #[test]
    fn capabilities_tools_only_from_mode_tool_pools() {
        let resp = build_capabilities_response();
        let allowed = product_mode_tool_pool_union();
        // Full catalog is larger than disclosed pool when pools are non-empty.
        let catalog_len = super::super::CapabilityRegistry::standard_cached().tool_count();
        if !allowed.is_empty() {
            assert!(
                resp.tools.len() <= allowed.len(),
                "tools should not exceed union pool size"
            );
            assert!(
                resp.tools.len() < catalog_len || catalog_len == allowed.len(),
                "must not dump full catalog when tool_pool is subset"
            );
        }
        for t in &resp.tools {
            assert!(
                allowed.contains(&t.id),
                "tool {} not in any mode tool_pool",
                t.id
            );
        }
        // search mode pool tools that exist in catalog appear
        if allowed.contains("web_search") {
            assert!(resp.tools.iter().any(|t| t.id == "web_search"));
        }
        // RAG keeps tool_pool empty for loop semantics; auto_fallback.tool_id is disclosed.
        assert!(
            allowed.contains("dense_retrieval"),
            "rag auto_fallback dense_retrieval must appear in disclosure union"
        );
        assert!(resp.tools.iter().any(|t| t.id == "dense_retrieval"));
        assert_eq!(
            resp.modes["search"].tool_pool,
            load_mode_tool_pool("search")
        );
        // mode.tool_pool in response is the YAML tool_pool only (not auto_fallback merge)
        // — product tools[] is the union; mode field stays faithful to ModeConfig.
    }
}
