use std::collections::HashSet;

use super::super::assembler::DisclosedState;
use super::config::{DiscloseAt, ModeConfig, SkillCluster};
use crate::runtime::AgentRequest;
use agent_tools::capability::CapabilityRegistry;
use agent_tools::progressive::{DisclosureContext, DisclosureTier, DisclosureUnit, PromptRegistry};

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
    ///
    /// When `request` is provided and the mode is not a worker handoff, optional
    /// writing/format refs from the request are disclosed on retrieve as well
    /// (SaC ProseOnly often skips a separate synthesis assembly).
    pub fn plan_retrieve(
        mode: &ModeConfig,
        first_round: bool,
        skill_request: Option<&[String]>,
        already_disclosed: &HashSet<String>,
        request: Option<&AgentRequest>,
    ) -> DisclosurePlan {
        let mut slices = Vec::new();

        if first_round {
            slices.push(DisclosureSlice::ClusterIndex(DiscloseAt::Retrieve));
        }

        // Mandatory retrieve clusters (e.g. knowledge-base skill) must stay in the system
        // prompt every round — not only iteration 0 — so the model can recover
        // from sandbox errors with method signatures in context.
        for cluster_id in &mode.skill_catalog.mandatory.retrieve {
            push_cluster_body(&mut slices, cluster_id, None, already_disclosed, true);
        }

        // Table structure reference (ontology + few-shot): default-disclose once
        // with knowledge-base so markitdown pipe tables are in context without skill_request.
        if first_round
            && mode
                .skill_catalog
                .mandatory
                .retrieve
                .iter()
                .any(|id| id == "knowledge-base")
        {
            push_cluster_body(
                &mut slices,
                "knowledge-base",
                Some("how-to-read-tables"),
                already_disclosed,
                false,
            );
        }

        if let Some(requested) = skill_request {
            for token in requested {
                let tok = crate::react_loop::skill_request::split_skill_request_token(token);
                push_cluster_body(
                    &mut slices,
                    &tok.cluster_id,
                    tok.reference.as_deref(),
                    already_disclosed,
                    false,
                );
            }
        }

        // SaC: writing/format on retrieve when the request carries style/shape hints
        // (in-loop DirectAnswer skips assemble_synthesis).
        if first_round && !mode.worker_handoff {
            if let Some(req) = request {
                push_writing_format_slices(&mut slices, req, already_disclosed);
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

        // Synthesis cluster index — noise for worker handoff; SaC answer loops keep it.
        if !mode.worker_handoff {
            slices.push(DisclosureSlice::ClusterIndex(DiscloseAt::Synthesis));
            // Prefer explicit synthesis choices, else same hint mapping as retrieve.
            if choices.writing_ref.is_some() || choices.format_ref.is_some() {
                if let Some(writing_ref) = choices.writing_ref.as_deref() {
                    push_cluster_body(
                        &mut slices,
                        "writing",
                        Some(writing_ref),
                        already_disclosed,
                        false,
                    );
                }
                if let Some(format_ref) = choices.format_ref.as_deref() {
                    push_cluster_body(
                        &mut slices,
                        "format",
                        Some(format_ref),
                        already_disclosed,
                        false,
                    );
                }
            } else {
                push_writing_format_slices(&mut slices, request, already_disclosed);
            }
        }

        if mode.inject_retrieval_query {
            slices.push(DisclosureSlice::RetrievalQuery);
        }

        DisclosurePlan { slices }
    }
}

/// Disclose at most one writing + one format reference from request hints.
fn push_writing_format_slices(
    slices: &mut Vec<DisclosureSlice>,
    request: &AgentRequest,
    already_disclosed: &HashSet<String>,
) {
    let choices = parse_synthesis_choices(request);
    if let Some(writing_ref) = choices.writing_ref.as_deref() {
        push_cluster_body(
            slices,
            "writing",
            Some(writing_ref),
            already_disclosed,
            false,
        );
    } else if let Some(hint) = request
        .metadata
        .get("writing_hint")
        .and_then(|v| v.as_str())
    {
        if let Some(ref_slug) = map_writing_hint(hint) {
            push_cluster_body(slices, "writing", Some(&ref_slug), already_disclosed, false);
        }
    }

    if let Some(format_ref) = choices.format_ref.as_deref() {
        push_cluster_body(slices, "format", Some(format_ref), already_disclosed, false);
    } else if let Some(hint) = request.format_hint.as_deref() {
        if let Some(ref_slug) = map_format_hint(hint) {
            push_cluster_body(slices, "format", Some(&ref_slug), already_disclosed, false);
        }
    }
}

fn push_cluster_body(
    slices: &mut Vec<DisclosureSlice>,
    cluster_id: &str,
    reference: Option<&str>,
    already_disclosed: &HashSet<String>,
    repeat_each_round: bool,
) {
    let key = if let Some(r) = reference {
        format!("{cluster_id}:{r}")
    } else {
        cluster_id.to_string()
    };
    if !repeat_each_round && already_disclosed.contains(&key) {
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
                    let repeat_each_round = mode
                        .skill_catalog
                        .mandatory
                        .retrieve
                        .iter()
                        .any(|id| id == cluster_id);
                    if disclosed.disclosed_skill_ids.contains(&key) && !repeat_each_round {
                        continue;
                    }
                    if let Some(body) = render_cluster_body(cluster_id, disclosed, ref_slug) {
                        let body = inject_cluster_runtime_context(cluster_id, body, request);
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
                    parts.push(format!("Retrieval query: {}", request.query));
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
    // Progressive refs: always render body at Load tier. Do **not** use Runtime
    // (which dumps every reference/* spoke). Spokes load only when
    // `reference_slug` is set (writing/format metadata, or
    // skill_request `cluster/slug`).
    let _ = skill.metadata().get("atomic"); // kept for future routing; Load for all
    let ctx = DisclosureContext::with_tier(DisclosureTier::Load);
    let mut parts = Vec::new();
    for dep in skill.dependencies() {
        if !disclosed.disclosed_skill_ids.contains(dep) {
            if let Some(dep_skill) = prompt_registry.skill(dep) {
                parts.push(dep_skill.render(&ctx));
            }
        }
    }
    // If only a reference is requested and the cluster body was already
    // disclosed this run, skip re-printing the full body.
    let body_key = cluster_id.to_string();
    let body_already = disclosed.disclosed_skill_ids.contains(&body_key);
    if !(body_already && reference_slug.is_some()) {
        parts.push(skill.render(&ctx));
    }

    if let Some(slug) = reference_slug {
        let ref_key = normalize_reference_key(slug);
        if let Some(content) = skill.references().get(&ref_key) {
            parts.push(format!("### Reference: {ref_key}\n{content}"));
        }
    }

    Some(parts.join("\n\n"))
}

/// Append cluster-specific runtime context (e.g. `docscope_metadata` for the
/// `metadata` cluster) to a rendered cluster body. Currently only the
/// `metadata` cluster carries runtime-injected context.
fn inject_cluster_runtime_context(
    cluster_id: &str,
    body: String,
    request: &AgentRequest,
) -> String {
    // WP3 renames the cluster to `docscope`; accept both until the switch is done.
    if cluster_id == "docscope" {
        if let Some(meta) = &request.docscope_metadata {
            let json = serde_json::to_string_pretty(meta).unwrap_or_default();
            return format!("{body}\n\n<docscope_metadata>\n{json}\n</docscope_metadata>");
        }
    }
    body
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

    fn rag_mode() -> ModeConfig {
        let mut mode = super::super::config::load_mode_config("rag").unwrap();
        // D9: mandatory retrieve is derived at assembly time (memory base +
        // capability skill); YAML no longer carries it.
        mode.skill_catalog.mandatory.retrieve = super::super::derive_mandatory_retrieve(true, false);
        mode
    }

    #[test]
    fn retrieve_first_round_includes_index_mandatory_and_query() {
        let mode = rag_mode();
        let plan = DisclosurePlanner::plan_retrieve(&mode, true, None, &HashSet::new(), None);
        assert!(
            plan.slices
                .contains(&DisclosureSlice::ClusterIndex(DiscloseAt::Retrieve))
        );
        assert!(plan
            .slices
            .iter()
            .any(|s| matches!(s, DisclosureSlice::ClusterBody { cluster_id, .. } if cluster_id == "knowledge-base")));
        assert!(plan.slices.contains(&DisclosureSlice::RetrievalQuery));
    }

    #[test]
    fn retrieve_later_round_includes_mandatory_codegen() {
        let mode = rag_mode();
        let plan = DisclosurePlanner::plan_retrieve(&mode, false, None, &HashSet::new(), None);
        assert!(
            !plan
                .slices
                .iter()
                .any(|s| matches!(s, DisclosureSlice::ClusterIndex(_)))
        );
        assert!(plan.slices.iter().any(
            |s| matches!(s, DisclosureSlice::ClusterBody { cluster_id, .. } if cluster_id == "knowledge-base")
        ));
        assert!(!plan.slices.contains(&DisclosureSlice::RetrievalQuery));
    }

    #[test]
    fn skill_request_adds_undisclosed_cluster_body() {
        let mode = rag_mode();
        let mut disclosed = HashSet::new();
        disclosed.insert("knowledge-base".to_string());
        let plan = DisclosurePlanner::plan_retrieve(
            &mode,
            false,
            Some(&["memory".to_string()]),
            &disclosed,
            None,
        );
        assert!(plan.slices.iter().any(|s| {
            matches!(s, DisclosureSlice::ClusterBody { cluster_id, .. } if cluster_id == "memory")
        }));
        assert!(plan.slices.iter().any(|s| {
            matches!(s, DisclosureSlice::ClusterBody { cluster_id, .. } if cluster_id == "knowledge-base")
        }));
    }

    #[test]
    fn chat_mode_no_retrieval_query_by_default() {
        let mode = super::super::config::load_mode_config("chat").unwrap();
        let plan = DisclosurePlanner::plan_retrieve(&mode, true, None, &HashSet::new(), None);
        assert!(!plan.slices.contains(&DisclosureSlice::RetrievalQuery));
    }
}
