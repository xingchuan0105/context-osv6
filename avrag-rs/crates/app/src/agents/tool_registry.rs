//! Agent-facing tool registry.
//!
//! Wraps `rag-core` tools and other agent capabilities into a uniform
//! `AgentTool` interface that the `AgentLoop` can dispatch.

use crate::agents::AgentKind;
use common::{ToolSpec, ToolResult, ToolStatus};
use std::collections::HashMap;

/// A tool that can be called by an agent during a tool-use loop.
#[async_trait::async_trait]
pub trait AgentTool: Send + Sync {
    /// Return the tool specification exposed to the LLM.
    fn spec(&self) -> ToolSpec;
    /// Execute the tool with the given arguments.
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult>;
}

/// Registry of all tools available to agents.
pub struct AgentToolRegistry {
    tools: HashMap<String, Box<dyn AgentTool>>,
}

impl AgentToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool.
    pub fn register(&mut self, tool: Box<dyn AgentTool>) {
        let name = tool.spec().name.clone();
        self.tools.insert(name, tool);
    }

    /// Get tool specs visible to a given agent kind.
    pub fn specs_for_kind(&self, kind: AgentKind) -> Vec<ToolSpec> {
        let allowed: &[&str] = match kind {
            AgentKind::Chat => &["load_skill", "compact_history"],
            AgentKind::Rag => &[
                "load_skill",
                "compact_history",
                "dense_retrieval",
                "lexical_retrieval",
                "graph_retrieval",
                "index_lookup",
                "doc_summary",
                "doc_metadata",
                "search_web",
            ],
            AgentKind::Search => &[
                "load_skill",
                "compact_history",
                "brave_search",
                "fetch_full_page",
            ],
        };
        allowed
            .iter()
            .filter_map(|name| self.tools.get(*name).map(|t| t.spec()))
            .collect()
    }

    /// Execute a tool by name.
    pub async fn execute(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> anyhow::Result<ToolResult> {
        match self.tools.get(name) {
            Some(tool) => tool.execute(args).await,
            None => Ok(ToolResult {
                tool: name.to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::NotFound,
                data: Some(serde_json::json!({"error": format!("tool '{}' not found", name)})),
                trace: None,
            }),
        }
    }
}

impl Default for AgentToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Placeholder tools for Phase A (to be replaced with real implementations).
// ---------------------------------------------------------------------------

/// Placeholder tool that returns a noop response.
pub struct PlaceholderTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

impl PlaceholderTool {
    pub fn load_skill() -> Self {
        Self {
            name: "load_skill".to_string(),
            description: "Load a skill file to get domain-specific instructions. \
                          Returns the skill content as a string.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Name of the skill to load"},
                    "lang": {"type": "string", "description": "Language code (e.g. 'zh', 'en')"}
                },
                "required": ["name"]
            }),
        }
    }

    pub fn compact_history() -> Self {
        Self {
            name: "compact_history".to_string(),
            description: "Compact conversation history by promoting older messages \
                          to summary layers. Call when context window is under pressure.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "keep_recent": {
                        "type": "integer",
                        "description": "Number of recent turns to keep in full text",
                        "default": 8
                    }
                }
            }),
        }
    }

    pub fn brave_search() -> Self {
        Self {
            name: "brave_search".to_string(),
            description: "Search the web using Brave LLM Context.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Search query"},
                    "vertical": {"type": "string", "enum": ["web", "news"], "default": "web"}
                },
                "required": ["query"]
            }),
        }
    }

    pub fn fetch_full_page() -> Self {
        Self {
            name: "fetch_full_page".to_string(),
            description: "Fetch the full content of a web page by URL.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "URL to fetch"}
                },
                "required": ["url"]
            }),
        }
    }
}

#[async_trait::async_trait]
impl AgentTool for PlaceholderTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name.clone(),
            version: "1.0".to_string(),
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
            output_schema: serde_json::json!({"type": "object"}),
        }
    }

    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        Ok(ToolResult {
            tool: self.name.clone(),
            version: "1.0".to_string(),
            status: ToolStatus::NotImplemented,
            data: Some(serde_json::json!({
                "status": "noop",
                "reason": "placeholder_tool_not_yet_implemented"
            })),
            trace: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_starts_empty() {
        let reg = AgentToolRegistry::new();
        assert!(reg.specs_for_kind(AgentKind::Chat).is_empty());
    }

    #[test]
    fn placeholder_load_skill_spec_is_valid() {
        let tool = PlaceholderTool::load_skill();
        let spec = tool.spec();
        assert_eq!(spec.name, "load_skill");
        assert!(!spec.description.is_empty());
    }

    #[test]
    fn chat_agent_sees_only_two_tools() {
        let mut reg = AgentToolRegistry::new();
        reg.register(Box::new(PlaceholderTool::load_skill()));
        reg.register(Box::new(PlaceholderTool::compact_history()));
        reg.register(Box::new(PlaceholderTool::brave_search()));

        let specs = reg.specs_for_kind(AgentKind::Chat);
        assert_eq!(specs.len(), 2);
        assert!(specs.iter().any(|s| s.name == "load_skill"));
        assert!(specs.iter().any(|s| s.name == "compact_history"));
    }

    #[test]
    fn rag_agent_sees_all_rag_tools() {
        let mut reg = AgentToolRegistry::new();
        reg.register(Box::new(PlaceholderTool::load_skill()));
        reg.register(Box::new(PlaceholderTool::compact_history()));
        reg.register(Box::new(PlaceholderTool::brave_search()));

        let specs = reg.specs_for_kind(AgentKind::Rag);
        // Only registered tools that match Rag allowed list: load_skill, compact_history
        // brave_search is NOT in Rag allowed list (it's for Search agent)
        assert_eq!(specs.len(), 2);
        assert!(specs.iter().any(|s| s.name == "load_skill"));
        assert!(specs.iter().any(|s| s.name == "compact_history"));
    }

    #[tokio::test]
    async fn execute_unknown_tool_returns_not_found() {
        let reg = AgentToolRegistry::new();
        let result = reg.execute("unknown", serde_json::json!({})).await.unwrap();
        assert_eq!(result.status, ToolStatus::NotFound);
    }

    #[tokio::test]
    async fn placeholder_returns_not_implemented() {
        let tool = PlaceholderTool::load_skill();
        let result = tool.execute(serde_json::json!({})).await.unwrap();
        assert_eq!(result.status, ToolStatus::NotImplemented);
    }
}
