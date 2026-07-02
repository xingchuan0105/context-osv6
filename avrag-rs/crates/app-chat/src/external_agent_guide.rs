use contracts::chat::AgentOperationGuide;

use crate::agents::capability::CapabilityRegistry;
use crate::agents::r#loop::config::load_mode_config;
use crate::agents::progressive::{
    DisclosureContext, DisclosureTier, DisclosureUnit, PromptRegistry,
};

const RAG_SUMMARY: &str = "RAG uses Python SDK codegen only. Emit <code language=\"python\"> blocks calling client.dense_search, client.chunk_fetch, etc. Do not call native retrieval tool schemas.";
const SEARCH_SUMMARY: &str = "Search uses native tool calls (web_search, web_fetch). Do not use codegen/SDK blocks in search mode.";
const INDEX_SUMMARY: &str = "Ingestion uses MCP workspace tools plus HTTP PUT for file bytes. Flow: create_upload → PUT upload_url → complete_upload → poll document_status until completed.";
const WORKSPACE_CREATE_SUMMARY: &str = "Personal product: humans create workspaces in the UI, then share notebook_id plus a workspace API key (index+query). Do not rely on account/org-scoped keys for normal automation.";

pub fn load_invoke_operation_guide(mode: &str) -> Option<AgentOperationGuide> {
    match mode {
        "rag" => Some(build_rag_guide()),
        "search" => Some(build_search_guide()),
        "index" => Some(build_index_guide()),
        "workspace.create" => Some(build_workspace_create_guide()),
        _ => None,
    }
}

pub fn attach_operation_guide(
    mut response: contracts::chat::ChatResponse,
) -> contracts::chat::ChatResponse {
    response.agent_operation_guide = load_invoke_operation_guide(&response.agent_type);
    response
}

fn build_rag_guide() -> AgentOperationGuide {
    let instructions = render_skill_instructions("codegen");
    AgentOperationGuide {
        mode: "rag".to_string(),
        summary: RAG_SUMMARY.to_string(),
        instructions,
        tool_schemas: Vec::new(),
    }
}

fn build_search_guide() -> AgentOperationGuide {
    let instructions = render_skill_instructions("search");
    let tool_schemas = load_mode_config("search")
        .map(|mode| mode.tools_for_retrieve(CapabilityRegistry::standard_cached()))
        .unwrap_or_default();

    AgentOperationGuide {
        mode: "search".to_string(),
        summary: SEARCH_SUMMARY.to_string(),
        instructions,
        tool_schemas,
    }
}

fn build_index_guide() -> AgentOperationGuide {
    let instructions = render_skill_instructions("index");
    AgentOperationGuide {
        mode: "index".to_string(),
        summary: INDEX_SUMMARY.to_string(),
        instructions,
        tool_schemas: Vec::new(),
    }
}

fn build_workspace_create_guide() -> AgentOperationGuide {
    let instructions = render_skill_instructions("workspace-create");
    AgentOperationGuide {
        mode: "workspace.create".to_string(),
        summary: WORKSPACE_CREATE_SUMMARY.to_string(),
        instructions,
        tool_schemas: Vec::new(),
    }
}

fn render_skill_instructions(skill_id: &str) -> String {
    let registry = PromptRegistry::standard_cached();
    let Some(skill) = registry.skill(skill_id) else {
        return String::new();
    };
    let ctx = DisclosureContext::with_tier(DisclosureTier::Runtime);
    skill.render(&ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rag_invoke_guide_uses_codegen_mode() {
        let guide = load_invoke_operation_guide("rag").expect("rag guide");
        assert_eq!(guide.mode, "rag");
        assert!(guide.summary.contains("codegen"));
        assert!(guide.tool_schemas.is_empty());
    }

    #[test]
    fn search_invoke_guide_exposes_native_tool_schemas() {
        let guide = load_invoke_operation_guide("search").expect("search guide");
        assert_eq!(guide.mode, "search");
        assert!(guide.summary.contains("web_search"));
        assert!(!guide.tool_schemas.is_empty());
    }

    #[test]
    fn index_invoke_guide_is_available() {
        let guide = load_invoke_operation_guide("index").expect("index guide");
        assert_eq!(guide.mode, "index");
        assert!(guide.summary.contains("create_upload"));
    }

    #[test]
    fn workspace_create_invoke_guide_is_available() {
        let guide =
            load_invoke_operation_guide("workspace.create").expect("workspace.create guide");
        assert_eq!(guide.mode, "workspace.create");
        assert!(guide.summary.contains("workspace API key"));
    }

    #[test]
    fn attach_operation_guide_sets_field_from_agent_type() {
        let response = attach_operation_guide(contracts::chat::ChatResponse {
            answer: String::new(),
            answer_blocks: Vec::new(),
            session_id: "s".to_string(),
            agent_type: "search".to_string(),
            sources: Vec::new(),
            citations: Vec::new(),
            trace: contracts::chat::TraceInfo {
                mode: "search".to_string(),
            },
            degrade_trace: Vec::new(),
            planner_output: None,
            mode_debug: None,
            message_id: None,
            guard_report: None,
            tool_results: Vec::new(),
            usage: None,
            agent_operation_guide: None,
        });
        assert_eq!(
            response
                .agent_operation_guide
                .as_ref()
                .map(|g| g.mode.as_str()),
            Some("search")
        );
    }
}
