use std::collections::HashSet;

use super::config::{DiscloseAt, ModeConfig, SkillCluster};
use crate::agents::capability::CapabilityRegistry;
use crate::agents::progressive::{
    DisclosureContext, DisclosureTier, DisclosureUnit, PromptRegistry,
};
use crate::agents::runtime::AgentRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopPhase {
    Retrieve,
    Synthesis,
}

#[derive(Debug, Clone, Default)]
pub struct DisclosedState {
    pub disclosed_skill_ids: HashSet<String>,
    pub last_skill_request: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct AssembledContext {
    pub system_content: String,
    pub tools: Vec<common::ToolSpec>,
    pub newly_disclosed_skills: Vec<String>,
}

pub struct ContextAssembler;

impl ContextAssembler {
    pub fn assemble_retrieve(
        iteration: u8,
        mode: &ModeConfig,
        request: &AgentRequest,
        registry: &CapabilityRegistry,
        disclosed: &mut DisclosedState,
        last_assistant_content: Option<&str>,
    ) -> AssembledContext {
        let base = super::config::load_system_prompt(&mode.system_prompt_base).unwrap_or_default();
        let mut parts = vec![base];
        let mut newly_disclosed = Vec::new();

        let round_load = mode
            .disclosure
            .rounds
            .iter()
            .find(|r| r.round_idx == iteration)
            .map(|r| r.load.clone());

        let show_cluster_index = matches!(
            round_load,
            Some(super::config::DisclosureLoad::RetrieveClusterIndex)
                | Some(super::config::DisclosureLoad::Index)
                | None if iteration == 0
        );

        if show_cluster_index {
            parts.push(render_cluster_index(mode, registry, DiscloseAt::Retrieve));
        }

        if mode.id == "rag" && iteration == 0 {
            let mandatory = vec!["codegen".to_string()];
            let bodies = render_cluster_bodies(&mandatory, disclosed, None);
            if !bodies.is_empty() {
                parts.push(bodies);
                newly_disclosed.extend(
                    mandatory
                        .iter()
                        .filter(|id| disclosed.disclosed_skill_ids.insert((*id).clone()))
                        .cloned(),
                );
            }
        }

        let skill_request = disclosed
            .last_skill_request
            .clone()
            .or_else(|| parse_skill_request(last_assistant_content));

        if let Some(requested) = skill_request {
            disclosed.last_skill_request = Some(requested.clone());
            let bodies = render_cluster_bodies(&requested, disclosed, None);
            if !bodies.is_empty() {
                parts.push(bodies);
                newly_disclosed.extend(
                    expand_cluster_ids(mode, &requested)
                        .into_iter()
                        .filter(|id| disclosed.disclosed_skill_ids.insert(id.clone())),
                );
            }
        }

        if iteration == 0 && (mode.id == "rag" || mode.id == "search") {
            parts.push(format!(
                "Retrieval query: {}\nUser display query: {}",
                request.effective_query(),
                request.query
            ));
        }

        let tools = mode.tools_for_retrieve(iteration, request.format_hint.as_deref(), registry);

        AssembledContext {
            system_content: parts.join("\n\n"),
            tools,
            newly_disclosed_skills: newly_disclosed,
        }
    }

    pub fn assemble_synthesis(
        mode: &ModeConfig,
        request: &AgentRequest,
        registry: &CapabilityRegistry,
        disclosed: &mut DisclosedState,
    ) -> AssembledContext {
        let base = super::config::load_system_prompt(&mode.system_prompt_base).unwrap_or_default();
        let mut parts = vec![base];
        let mut newly_disclosed = Vec::new();

        for skill_id in mode.mandatory_synthesis_skills() {
            if let Some(body) = render_skill_body_with_deps(skill_id, disclosed) {
                parts.push(body);
                if disclosed.disclosed_skill_ids.insert(skill_id.clone()) {
                    newly_disclosed.push(skill_id.clone());
                }
            }
        }

        parts.push(render_cluster_index(mode, registry, DiscloseAt::Synthesis));

        if let Some(hint) = request.format_hint.as_deref() {
            parts.push(format!(
                "\n<format_hint>\nUser prefers format skill: {hint}. You may still choose a different format if inappropriate.\n</format_hint>"
            ));
        }

        if let Some(hint) = request
            .metadata
            .get("writing_hint")
            .and_then(|v| v.as_str())
        {
            parts.push(format!(
                "\n<writing_hint>\nUser prefers writing style: {hint}. You may override if inappropriate.\n</writing_hint>"
            ));
        }

        let choices = parse_synthesis_choices(request);

        if let Some(writing_ref) = choices.writing_ref.as_deref() {
            if let Some(body) = render_cluster_body("writing", disclosed, Some(writing_ref)) {
                parts.push(body);
                if disclosed
                    .disclosed_skill_ids
                    .insert(format!("writing:{writing_ref}"))
                {
                    newly_disclosed.push("writing".to_string());
                }
            }
        } else if let Some(hint) = request
            .metadata
            .get("writing_hint")
            .and_then(|v| v.as_str())
        {
            if let Some(ref_slug) = map_writing_hint(hint) {
                if let Some(body) = render_cluster_body("writing", disclosed, Some(&ref_slug)) {
                    parts.push(body);
                    if disclosed
                        .disclosed_skill_ids
                        .insert(format!("writing:{ref_slug}"))
                    {
                        newly_disclosed.push("writing".to_string());
                    }
                }
            }
        }

        if let Some(format_ref) = choices.format_ref.as_deref() {
            if let Some(body) = render_cluster_body("format", disclosed, Some(format_ref)) {
                parts.push(body);
                if disclosed
                    .disclosed_skill_ids
                    .insert(format!("format:{format_ref}"))
                {
                    newly_disclosed.push("format".to_string());
                }
            }
        } else if let Some(hint) = request.format_hint.as_deref() {
            if let Some(ref_slug) = map_format_hint(hint) {
                if let Some(body) = render_cluster_body("format", disclosed, Some(&ref_slug)) {
                    parts.push(body);
                    if disclosed
                        .disclosed_skill_ids
                        .insert(format!("format:{ref_slug}"))
                    {
                        newly_disclosed.push("format".to_string());
                    }
                }
            }
        }

        if mode.id == "rag" || mode.id == "search" {
            parts.push(format!(
                "Retrieval query: {}\nUser display query: {}",
                request.effective_query(),
                request.query
            ));
        }

        AssembledContext {
            system_content: parts.join("\n\n"),
            tools: vec![],
            newly_disclosed_skills: newly_disclosed,
        }
    }
}

struct SynthesisChoices {
    writing_ref: Option<String>,
    format_ref: Option<String>,
}

fn parse_synthesis_choices(request: &AgentRequest) -> SynthesisChoices {
    let writing_ref = request
        .metadata
        .get("writing_ref")
        .or_else(|| request.metadata.get("writing_choice"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let format_ref = request
        .metadata
        .get("format_ref")
        .or_else(|| request.metadata.get("format_choice"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    SynthesisChoices {
        writing_ref,
        format_ref,
    }
}

pub fn parse_skill_request(content: Option<&str>) -> Option<Vec<String>> {
    let content = content?;
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(content) {
        if let Some(arr) = value.get("skill_request").and_then(|v| v.as_array()) {
            let ids: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
            if !ids.is_empty() {
                return Some(ids);
            }
        }
    }
    if let Some(start) = content.find("\"skill_request\"") {
        let slice = &content[start..];
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&format!("{{{slice}}}")) {
            if let Some(arr) = value.get("skill_request").and_then(|v| v.as_array()) {
                let ids: Vec<String> = arr
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect();
                if !ids.is_empty() {
                    return Some(ids);
                }
            }
        }
    }
    for cluster_id in ["codegen", "search", "memory", "writing", "format"] {
        if content.contains(&format!("\"{cluster_id}\""))
            || content.contains(&format!("skill_request: {cluster_id}"))
            || content.contains(&format!("request cluster `{cluster_id}`"))
            || content.contains(&format!("请求 **{cluster_id}**"))
            || content.contains(&format!("请求 {cluster_id} 簇"))
        {
            return Some(vec![cluster_id.to_string()]);
        }
    }
    None
}

fn expand_cluster_ids(mode: &ModeConfig, requested: &[String]) -> Vec<String> {
    let mut ids = Vec::new();
    for item in requested {
        if mode.skill_catalog.cluster_by_id(item).is_some() {
            if !ids.contains(item) {
                ids.push(item.clone());
            }
        } else if !ids.contains(item) {
            ids.push(item.clone());
        }
    }
    ids
}

fn render_cluster_index(
    mode: &ModeConfig,
    registry: &CapabilityRegistry,
    phase: DiscloseAt,
) -> String {
    let clusters = mode.skill_catalog.clusters_at(phase);
    if clusters.is_empty() {
        return String::new();
    }
    let title = match phase {
        DiscloseAt::Retrieve => "<retrieve_cluster_index>",
        DiscloseAt::Synthesis => "<synthesis_skill_index>",
    };
    let end = match phase {
        DiscloseAt::Retrieve => "</retrieve_cluster_index>",
        DiscloseAt::Synthesis => "</synthesis_skill_index>",
    };
    let mut text = format!("\n{title}\n");
    for cluster in clusters {
        text.push_str(&render_cluster_entry(cluster, registry));
    }
    if !mode.tool_pool.is_empty() && phase == DiscloseAt::Retrieve {
        text.push_str("\n**tool_pool**: ");
        text.push_str(&mode.tool_pool.join(", "));
        text.push('\n');
    }
    text.push_str(end);
    text
}

fn render_cluster_entry(cluster: &SkillCluster, registry: &CapabilityRegistry) -> String {
    let mut line = format!("- **{}**", cluster.id);
    let desc = cluster
        .description
        .as_deref()
        .or_else(|| registry.skill(&cluster.id).map(|m| m.description.as_str()));
    if let Some(desc) = desc {
        line.push_str(&format!(": {desc}"));
    }
    if cluster.atomic {
        line.push_str(" (atomic)");
    }
    line.push('\n');
    line
}

fn render_cluster_bodies(
    requested: &[String],
    disclosed: &mut DisclosedState,
    reference: Option<&str>,
) -> String {
    let mut bodies = Vec::new();
    for cluster_id in requested {
        let key = if let Some(r) = reference {
            format!("{cluster_id}:{r}")
        } else {
            cluster_id.clone()
        };
        if disclosed.disclosed_skill_ids.contains(&key) {
            continue;
        }
        if let Some(body) = render_cluster_body(cluster_id, disclosed, reference) {
            bodies.push(body);
            disclosed.disclosed_skill_ids.insert(key);
        }
    }
    if bodies.is_empty() {
        String::new()
    } else {
        format!("\n<skill_bodies>\n{}\n</skill_bodies>", bodies.join("\n\n"))
    }
}

fn render_cluster_body(
    cluster_id: &str,
    disclosed: &DisclosedState,
    reference_slug: Option<&str>,
) -> Option<String> {
    let prompt_registry = PromptRegistry::standard_cached();
    let skill = prompt_registry.skill(cluster_id)?;
    let is_atomic = skill.metadata().get("atomic") == Some(&"true".to_string());

    for dep in skill.dependencies() {
        if !disclosed.disclosed_skill_ids.contains(dep) {
            if let Some(dep_skill) = prompt_registry.skill(dep) {
                let ctx = DisclosureContext::with_tier(DisclosureTier::Load);
                let _ = dep_skill.render(&ctx);
            }
        }
    }

    let ctx = DisclosureContext::with_tier(if is_atomic && reference_slug.is_none() {
        DisclosureTier::Runtime
    } else {
        DisclosureTier::Load
    });
    let mut parts = vec![skill.render(&ctx)];

    if let Some(slug) = reference_slug {
        let ref_key = normalize_reference_key(slug);
        if let Some(content) = skill.references().get(&ref_key) {
            parts.push(format!("### Reference: {ref_key}\n{content}"));
        }
    }

    Some(parts.join("\n\n"))
}

fn render_skill_body_with_deps(skill_id: &str, disclosed: &DisclosedState) -> Option<String> {
    let prompt_registry = PromptRegistry::standard_cached();
    let skill = prompt_registry.skill(skill_id)?;
    let ctx = DisclosureContext::with_tier(DisclosureTier::Load);
    let mut parts = Vec::new();
    for dep in skill.dependencies() {
        if !disclosed.disclosed_skill_ids.contains(dep) {
            if let Some(dep_skill) = prompt_registry.skill(dep) {
                parts.push(dep_skill.render(&ctx));
            }
        }
    }
    parts.push(skill.render(&ctx));
    Some(parts.join("\n\n"))
}

fn normalize_reference_key(slug: &str) -> String {
    let slug = slug.trim();
    if slug.ends_with(".md") {
        slug.to_string()
    } else {
        format!("{slug}.md")
    }
}

fn map_writing_hint(hint: &str) -> Option<String> {
    let hint = hint.trim();
    let slug = hint
        .strip_suffix("-writing")
        .or_else(|| hint.strip_prefix("writing/"))
        .unwrap_or(hint);
    match slug {
        "tone-guidance" | "tone" => Some("tone".to_string()),
        "concise-writing" | "concise" => Some("concise".to_string()),
        "professional-writing" | "professional" => Some("professional".to_string()),
        "academic-writing" | "academic" => Some("academic".to_string()),
        "storytelling" => Some("storytelling".to_string()),
        "brainstorming" => Some("brainstorming".to_string()),
        _ if !slug.is_empty() => Some(slug.to_string()),
        _ => None,
    }
}

fn map_format_hint(hint: &str) -> Option<String> {
    let hint = hint.trim();
    match hint {
        "html-renderer" => Some("html-renderer".to_string()),
        "ppt-generation" => Some("ppt-generation".to_string()),
        "framework-extraction" => Some("framework-extraction".to_string()),
        "teaching" => Some("teaching".to_string()),
        _ if !hint.is_empty() => Some(hint.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_skill_request_from_json() {
        let content = r#"{"skill_request": ["codegen"]}"#;
        let ids = parse_skill_request(Some(content)).unwrap();
        assert_eq!(ids, vec!["codegen"]);
    }

    #[test]
    fn expand_codegen_cluster_is_single_id() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let ids = expand_cluster_ids(&mode, &["codegen".to_string()]);
        assert_eq!(ids, vec!["codegen"]);
    }

    #[test]
    fn synthesis_clusters_only_at_synthesis_phase() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let registry = CapabilityRegistry::standard_cached();
        let retrieve = render_cluster_index(&mode, registry, DiscloseAt::Retrieve);
        let synthesis = render_cluster_index(&mode, registry, DiscloseAt::Synthesis);
        assert!(retrieve.contains("codegen"));
        assert!(!retrieve.contains("writing"));
        assert!(synthesis.contains("writing"));
        assert!(!synthesis.contains("html-renderer:"));
        assert!(!synthesis.contains("professional-writing"));
    }

    #[test]
    fn rag_retrieve_tools_always_empty() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let registry = CapabilityRegistry::standard_cached();
        assert!(mode.tools_for_retrieve(0, None, registry).is_empty());
        assert!(
            mode.tools_for_retrieve(1, Some("html-renderer"), registry)
                .is_empty()
        );
    }

    #[test]
    fn rag_round_zero_discloses_codegen_bundle() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let registry = CapabilityRegistry::standard_cached();
        let mut disclosed = DisclosedState::default();
        let ctx = ContextAssembler::assemble_retrieve(
            0,
            &mode,
            &crate::agents::runtime::AgentRequest {
                kind: crate::agents::AgentKind::Rag,
                query: "test".to_string(),
                resolved_query: "test".to_string(),
                query_resolution: None,
                notebook_id: None,
                session_id: None,
                doc_scope: vec![],
                messages: vec![],
                session_summary: None,
                user_preferences: None,
                debug: false,
                stream: false,
                language: None,
                auth_context: serde_json::json!({}),
                docscope_metadata: None,
                metadata: Default::default(),
                cancellation_token: None,
                guard_pipeline: None,
                preferred_tools: vec![],
                format_hint: None,
                max_iterations: None,
            },
            &registry,
            &mut disclosed,
            None,
        );
        assert!(ctx.system_content.contains("dense_search"));
        assert!(!ctx.system_content.contains("rag-codegen-guide"));
        assert!(ctx.system_content.contains("Retrieval query: test"));
        assert!(ctx.tools.is_empty());
    }
}
