use contracts::chat::AgentOperationGuide;

use agent_tools::progressive::{DisclosureContext, DisclosureTier, DisclosureUnit, PromptRegistry};

const RAG_SUMMARY: &str =
    include_str!("../../../prompts/agent-guide/rag-summary.md");
const SEARCH_SUMMARY: &str =
    include_str!("../../../prompts/agent-guide/search-summary.md");
const INDEX_SUMMARY: &str =
    include_str!("../../../prompts/agent-guide/index-summary.md");
const WORKSPACE_CREATE_SUMMARY: &str =
    include_str!("../../../prompts/agent-guide/workspace-create-summary.md");

/// Prefetch / MCP operation guides for external agents.
///
/// Product modes with guides: `rag`, `search`, `index`, `workspace.create`.
/// Dual `rag+search` has no separate guide (clients compose rag/search).
/// Write is product-offline (`write_mode_disabled`) — no guide.
pub fn load_invoke_operation_guide(mode: &str) -> Option<AgentOperationGuide> {
    match mode {
        "rag" => Some(build_rag_guide()),
        "search" => Some(build_search_guide()),
        "index" => Some(build_index_guide()),
        "workspace.create" => Some(build_workspace_create_guide()),
        // write / write_refine / chat pure: no external-agent guide
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
    let instructions = render_skill_instructions("knowledge-base");
    AgentOperationGuide {
        mode: "rag".to_string(),
        summary: RAG_SUMMARY.to_string(),
        instructions,
        tool_schemas: Vec::new(),
    }
}

fn build_search_guide() -> AgentOperationGuide {
    // A1/A6: no native tool schemas — SaC client.web/fetch only.
    let instructions = render_skill_instructions("search");
    AgentOperationGuide {
        mode: "search".to_string(),
        summary: SEARCH_SUMMARY.to_string(),
        instructions,
        tool_schemas: Vec::new(),
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
    fn rag_invoke_guide_uses_sac_sdk() {
        let guide = load_invoke_operation_guide("rag").expect("rag guide");
        assert_eq!(guide.mode, "rag");
        assert!(
            guide.summary.contains("SaC") || guide.summary.contains("client.dense"),
            "summary: {}",
            guide.summary
        );
        assert!(guide.tool_schemas.is_empty());
    }

    #[test]
    fn search_invoke_guide_uses_sac_sdk() {
        let guide = load_invoke_operation_guide("search").expect("search guide");
        assert_eq!(guide.mode, "search");
        assert!(
            guide.summary.contains("client.web") || guide.summary.contains("SaC"),
            "summary: {}",
            guide.summary
        );
        assert!(
            guide.tool_schemas.is_empty(),
            "no native web tool schemas after SaC"
        );
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
