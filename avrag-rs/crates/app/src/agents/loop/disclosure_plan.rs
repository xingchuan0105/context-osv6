use std::collections::HashSet;

use super::assembler::DisclosedState;
use super::config::{DiscloseAt, ModeConfig, SkillCluster};
use crate::agents::capability::CapabilityRegistry;
use crate::agents::progressive::{
    DisclosureContext, DisclosureTier, DisclosureUnit, PromptRegistry,
};
use crate::agents::runtime::AgentRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisclosureSlice {
    ClusterIndex(DiscloseAt),
    ClusterBody {
        cluster_id: String,
        reference: Option<String>,
    },
    MandatorySkillBody {
        skill_id: String,
    },
    RetrievalQuery,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DisclosurePlan {
    pub slices: Vec<DisclosureSlice>,
}

#[derive(Debug, Clone, Default)]
pub struct SynthesisChoices {
    pub writing_ref: Option<String>,
    pub format_ref: Option<String>,
}

pub struct DisclosurePlanner;

impl DisclosurePlanner {
    /// `first_round` is assembler internal state (`iteration == 0`), not a config key.
    pub fn plan_retrieve(
        mode: &ModeConfig,
        first_round: bool,
        skill_request: Option<&[String]>,
        already_disclosed: &HashSet<String>,
    ) -> DisclosurePlan {
        let mut slices = Vec::new();

        if first_round {
            slices.push(DisclosureSlice::ClusterIndex(DiscloseAt::Retrieve));
            for cluster_id in &mode.skill_catalog.mandatory.retrieve {
                push_cluster_body(&mut slices, cluster_id, None, already_disclosed);
            }
        }

        if let Some(requested) = skill_request {
            for cluster_id in requested {
                push_cluster_body(&mut slices, cluster_id, None, already_disclosed);
            }
        }

        if first_round && mode.inject_retrieval_query {
            slices.push(DisclosureSlice::RetrievalQuery);
        }

        DisclosurePlan { slices }
    }

    pub fn plan_synthesis(
        mode: &ModeConfig,
        request: &AgentRequest,
        choices: &SynthesisChoices,
        already_disclosed: &HashSet<String>,
    ) -> DisclosurePlan {
        let mut slices = Vec::new();

        for skill_id in mode.mandatory_synthesis_skills() {
            if !already_disclosed.contains(skill_id) {
                slices.push(DisclosureSlice::MandatorySkillBody {
                    skill_id: skill_id.clone(),
                });
            }
        }

        slices.push(DisclosureSlice::ClusterIndex(DiscloseAt::Synthesis));

        if let Some(writing_ref) = choices.writing_ref.as_deref() {
            push_cluster_body(&mut slices, "writing", Some(writing_ref), already_disclosed);
        } else if let Some(hint) = request
            .metadata
            .get("writing_hint")
            .and_then(|v| v.as_str())
        {
            if let Some(ref_slug) = map_writing_hint(hint) {
                push_cluster_body(&mut slices, "writing", Some(&ref_slug), already_disclosed);
            }
        }

        if let Some(format_ref) = choices.format_ref.as_deref() {
            push_cluster_body(&mut slices, "format", Some(format_ref), already_disclosed);
        } else if let Some(hint) = request.format_hint.as_deref() {
            if let Some(ref_slug) = map_format_hint(hint) {
                push_cluster_body(&mut slices, "format", Some(&ref_slug), already_disclosed);
            }
        }

        if mode.inject_retrieval_query {
            slices.push(DisclosureSlice::RetrievalQuery);
        }

        DisclosurePlan { slices }
    }
}

fn push_cluster_body(
    slices: &mut Vec<DisclosureSlice>,
    cluster_id: &str,
    reference: Option<&str>,
    already_disclosed: &HashSet<String>,
) {
    let key = if let Some(r) = reference {
        format!("{cluster_id}:{r}")
    } else {
        cluster_id.to_string()
    };
    if already_disclosed.contains(&key) {
        return;
    }
    slices.push(DisclosureSlice::ClusterBody {
        cluster_id: cluster_id.to_string(),
        reference: reference.map(str::to_string),
    });
}

pub struct RenderedSlices {
    pub text: String,
    pub newly_disclosed: Vec<String>,
}

pub struct DisclosureRenderer<'a> {
    capability: &'a CapabilityRegistry,
}

impl<'a> DisclosureRenderer<'a> {
    pub fn new(capability: &'a CapabilityRegistry) -> Self {
        Self { capability }
    }

    /// Read SKILL.md / index and assemble text. Mutates `disclosed` (sole mutation site).
    pub fn render(
        &self,
        plan: &DisclosurePlan,
        mode: &ModeConfig,
        request: &AgentRequest,
        disclosed: &mut DisclosedState,
    ) -> RenderedSlices {
        let mut parts = Vec::new();
        let mut newly_disclosed = Vec::new();

        for slice in &plan.slices {
            match slice {
                DisclosureSlice::ClusterIndex(phase) => {
                    let text = render_cluster_index(mode, self.capability, *phase);
                    if !text.is_empty() {
                        parts.push(text);
                    }
                }
                DisclosureSlice::ClusterBody {
                    cluster_id,
                    reference,
                } => {
                    let ref_slug = reference.as_deref();
                    let key = if let Some(r) = ref_slug {
                        format!("{cluster_id}:{r}")
                    } else {
                        cluster_id.clone()
                    };
                    if disclosed.disclosed_skill_ids.contains(&key) {
                        continue;
                    }
                    if let Some(body) = render_cluster_body(cluster_id, disclosed, ref_slug) {
                        parts.push(body);
                        if disclosed.disclosed_skill_ids.insert(key.clone()) {
                            newly_disclosed.push(cluster_id.clone());
                        }
                    }
                }
                DisclosureSlice::MandatorySkillBody { skill_id } => {
                    if disclosed.disclosed_skill_ids.contains(skill_id) {
                        continue;
                    }
                    if let Some(body) = render_skill_body_with_deps(skill_id, disclosed) {
                        parts.push(body);
                        if disclosed.disclosed_skill_ids.insert(skill_id.clone()) {
                            newly_disclosed.push(skill_id.clone());
                        }
                    }
                }
                DisclosureSlice::RetrievalQuery => {
                    parts.push(format!(
                        "Retrieval query: {}\nUser display query: {}",
                        request.effective_query(),
                        request.query
                    ));
                }
            }
        }

        let text = if parts.is_empty() {
            String::new()
        } else {
            parts.join("\n\n")
        };
        RenderedSlices {
            text,
            newly_disclosed,
        }
    }
}

pub fn parse_synthesis_choices(request: &AgentRequest) -> SynthesisChoices {
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

fn render_cluster_body(
    cluster_id: &str,
    disclosed: &DisclosedState,
    reference_slug: Option<&str>,
) -> Option<String> {
    let prompt_registry = PromptRegistry::standard_cached();
    let skill = prompt_registry.skill(cluster_id)?;
    let is_atomic = skill.metadata().get("atomic") == Some(&"true".to_string());
    let ctx = DisclosureContext::with_tier(if is_atomic && reference_slug.is_none() {
        DisclosureTier::Runtime
    } else {
        DisclosureTier::Load
    });
    let mut parts = Vec::new();
    for dep in skill.dependencies() {
        if !disclosed.disclosed_skill_ids.contains(dep) {
            if let Some(dep_skill) = prompt_registry.skill(dep) {
                parts.push(dep_skill.render(&ctx));
            }
        }
    }
    parts.push(skill.render(&ctx));

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
    use crate::agents::runtime::AgentRequest;

    fn rag_mode() -> ModeConfig {
        super::super::config::load_mode_config("rag").unwrap()
    }

    #[test]
    fn retrieve_first_round_includes_index_mandatory_and_query() {
        let mode = rag_mode();
        let plan = DisclosurePlanner::plan_retrieve(&mode, true, None, &HashSet::new());
        assert!(plan.slices.contains(&DisclosureSlice::ClusterIndex(DiscloseAt::Retrieve)));
        assert!(plan
            .slices
            .iter()
            .any(|s| matches!(s, DisclosureSlice::ClusterBody { cluster_id, .. } if cluster_id == "codegen")));
        assert!(plan.slices.contains(&DisclosureSlice::RetrievalQuery));
    }

    #[test]
    fn retrieve_later_round_skips_index_and_mandatory() {
        let mode = rag_mode();
        let plan = DisclosurePlanner::plan_retrieve(&mode, false, None, &HashSet::new());
        assert!(!plan
            .slices
            .iter()
            .any(|s| matches!(s, DisclosureSlice::ClusterIndex(_))));
        assert!(!plan
            .slices
            .iter()
            .any(|s| matches!(s, DisclosureSlice::ClusterBody { cluster_id, .. } if cluster_id == "codegen")));
    }

    #[test]
    fn skill_request_adds_undisclosed_cluster_body() {
        let mode = rag_mode();
        let mut disclosed = HashSet::new();
        disclosed.insert("codegen".to_string());
        let plan = DisclosurePlanner::plan_retrieve(
            &mode,
            false,
            Some(&["memory".to_string()]),
            &disclosed,
        );
        assert!(plan.slices.iter().any(|s| {
            matches!(s, DisclosureSlice::ClusterBody { cluster_id, .. } if cluster_id == "memory")
        }));
        assert!(!plan.slices.iter().any(|s| {
            matches!(s, DisclosureSlice::ClusterBody { cluster_id, .. } if cluster_id == "codegen")
        }));
    }

    #[test]
    fn chat_mode_no_retrieval_query_by_default() {
        let mode = super::super::config::load_mode_config("chat").unwrap();
        let plan = DisclosurePlanner::plan_retrieve(&mode, true, None, &HashSet::new());
        assert!(!plan.slices.contains(&DisclosureSlice::RetrievalQuery));
    }
}
