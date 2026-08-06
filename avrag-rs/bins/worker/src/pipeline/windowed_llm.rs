//! Windowed profile+summary + triplet extraction (design 2026-08-06).

use anyhow::Result;
use avrag_storage_pg::TocEntry;
use common::SummaryMetadata;
use contracts::auth_runtime::AuthContext;
use ingestion::IngestionTask;
use common::{Domain, Era, Genre};
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use super::document_pipeline::ParseRunState;
use super::helpers::{
    PROFILE_SEED_TEMPERATURE, SUMMARY_TEMPERATURE, TRIPLET_TEMPERATURE, record_graph_degrade,
};
use super::ingestion_session::{DocumentIngestionSession, compose_window_system};
use super::processor::PgTaskProcessor;
use super::triplet_extraction::{
    ExtractedTriplet, TripletExtractionOutput, parse_triplet_response_no_chunk,
    triplet_extraction_enabled,
};
use super::window_split::split_document_windows;
use crate::ingestion_guard::ensure_ingestion_side_effects_allowed;

const PS_JOINT_PROMPT: &str = include_str!("../../../../prompts/pipeline/profile-summary.joint.md");
const PS_USER: &str = include_str!("../../../../prompts/templates/profile-summary-user.tmpl");
const PS_MERGE_PROMPT: &str =
    include_str!("../../../../prompts/pipeline/profile-summary-merge.md");
const PS_MERGE_SYSTEM: &str =
    include_str!("../../../../prompts/pipeline/profile-summary-merge.system.md");
const TRIPLET_PROMPT: &str =
    include_str!("../../../../prompts/pipeline/triplet-extraction.system.md");
const TRIPLET_USER: &str =
    include_str!("../../../../prompts/templates/triplet-extraction-user.tmpl");

#[derive(Debug, Default, Deserialize, Serialize)]
struct PsMetadata {
    language: Option<String>,
    domain: Option<String>,
    genre: Option<String>,
    era: Option<String>,
    author: Option<String>,
    publication_date: Option<String>,
    title: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PsSection {
    title: String,
    #[serde(default)]
    heading_level: i32,
    #[serde(default)]
    rank: i32,
    #[serde(default)]
    overview: Option<String>,
    #[serde(default)]
    children: Vec<PsSection>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ProfileSummaryJson {
    #[serde(default)]
    metadata: PsMetadata,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    sections: Vec<PsSection>,
}

#[derive(Debug, Default)]
pub(crate) struct WindowedLlmResult {
    pub(crate) toc_entries: Vec<TocEntry>,
    pub(crate) summary_text: String,
    pub(crate) profile_metadata: Option<SummaryMetadata>,
    pub(crate) triplets: TripletExtractionOutput,
    /// Sum of provider prompt_tokens across all LLM turns (PS + triplet + merge).
    pub(crate) prompt_tokens: u32,
    /// Sum of provider completion_tokens across all LLM turns.
    pub(crate) completion_tokens: u32,
}

/// Run even-window PS (turn1) + triplets (turn2 same session); multi-window LLM merge for PS.
pub(crate) async fn run_windowed_ps_and_triplets(
    processor: &PgTaskProcessor,
    context: &AuthContext,
    task: &IngestionTask,
    document_id: Uuid,
    workspace_id: Uuid,
    filename: &str,
    doc_title: &str,
    raw_text: &str,
    parse_run_state: &mut ParseRunState,
) -> WindowedLlmResult {
    let Some(llm) = processor.llm.ingestion_llm.clone() else {
        info!(document_id = %document_id, "ingestion llm not configured; skip windowed extraction");
        return WindowedLlmResult::default();
    };

    let windows = split_document_windows(raw_text);
    if windows.is_empty() || windows.iter().all(|w| w.trim().is_empty()) {
        return WindowedLlmResult::default();
    }

    info!(
        document_id = %document_id,
        windows = windows.len(),
        "windowed ingestion llm begin"
    );

    let mut ps_jsons: Vec<ProfileSummaryJson> = Vec::new();
    let mut triplet_out = TripletExtractionOutput::default();
    let mut triplet_map: std::collections::HashMap<(String, String, String), ExtractedTriplet> =
        std::collections::HashMap::new();
    // Billing counters for the windowed stage (PS + optional triplet + merge).
    let mut prompt_tokens: u32 = 0;
    let mut completion_tokens: u32 = 0;
    // Only true triplet-turn totals ride pending_triplets into index stage.
    let mut triplet_turn_tokens: u32 = 0;
    let run_triplets = triplet_extraction_enabled();

    for (wi, window) in windows.iter().enumerate() {
        if window.trim().is_empty() {
            continue;
        }
        let system = compose_window_system(window);
        let mut session = DocumentIngestionSession::new(llm.clone());

        let ps_user = format!("{PS_JOINT_PROMPT}\n\n{PS_USER}");
        match session
            .seed(&system, &ps_user, Some(PROFILE_SEED_TEMPERATURE))
            .await
        {
            Ok(turn) => {
                prompt_tokens = prompt_tokens.saturating_add(turn.usage.prompt_tokens);
                completion_tokens =
                    completion_tokens.saturating_add(turn.usage.completion_tokens);
                match parse_ps_json(&turn.content) {
                    Ok(ps) => ps_jsons.push(ps),
                    Err(error) => {
                        record_graph_degrade(
                            &mut parse_run_state.outputs,
                            format!("window {wi} profile+summary parse failed: {error}"),
                        );
                    }
                }
            }
            Err(error) => {
                record_graph_degrade(
                    &mut parse_run_state.outputs,
                    format!("window {wi} profile+summary failed: {error}"),
                );
                continue;
            }
        }

        if !run_triplets {
            continue;
        }

        let tri_user = format!("{TRIPLET_PROMPT}\n\n{TRIPLET_USER}");
        match session
            .produce(&tri_user, Some(TRIPLET_TEMPERATURE))
            .await
        {
            Ok(turn) => {
                prompt_tokens = prompt_tokens.saturating_add(turn.usage.prompt_tokens);
                completion_tokens =
                    completion_tokens.saturating_add(turn.usage.completion_tokens);
                triplet_turn_tokens =
                    triplet_turn_tokens.saturating_add(turn.usage.total_tokens);
                match parse_triplet_response_no_chunk(&turn.content) {
                    Ok(triplets) => {
                        for triplet in triplets {
                            let key = (
                                triplet.subject.to_lowercase(),
                                triplet.predicate.to_lowercase(),
                                triplet.object.to_lowercase(),
                            );
                            if let Some(existing) = triplet_map.get_mut(&key) {
                                if triplet.confidence > existing.confidence {
                                    existing.confidence = triplet.confidence;
                                }
                            } else {
                                triplet_map.insert(key, triplet);
                            }
                        }
                    }
                    Err(error) => {
                        record_graph_degrade(
                            &mut parse_run_state.outputs,
                            format!("window {wi} triplet parse failed: {error}"),
                        );
                    }
                }
            }
            Err(error) => {
                record_graph_degrade(
                    &mut parse_run_state.outputs,
                    format!("window {wi} triplet failed: {error}"),
                );
            }
        }
    }

    triplet_out.triplets = triplet_map.into_values().collect();
    // Index-stage `triplet_extraction_tokens` must only reflect triplet turns.
    triplet_out.total_tokens = triplet_turn_tokens;

    let final_ps = if ps_jsons.is_empty() {
        None
    } else if ps_jsons.len() == 1 {
        Some(ps_jsons.remove(0))
    } else {
        match merge_ps_with_llm(processor, &ps_jsons).await {
            Ok((ps, usage)) => {
                prompt_tokens = prompt_tokens.saturating_add(usage.prompt_tokens);
                completion_tokens =
                    completion_tokens.saturating_add(usage.completion_tokens);
                Some(ps)
            }
            Err(error) => {
                record_graph_degrade(
                    &mut parse_run_state.outputs,
                    format!(
                        "profile+summary LLM merge failed: {error}; using deterministic union of all windows"
                    ),
                );
                Some(merge_ps_deterministic(&ps_jsons))
            }
        }
    };

    let mut result = WindowedLlmResult {
        triplets: triplet_out,
        prompt_tokens,
        completion_tokens,
        ..Default::default()
    };

    if let Some(ps) = final_ps {
        result.summary_text = ps.summary.clone();
        result.toc_entries = toc_from_ps_sections(&ps.sections);
        result.profile_metadata = Some(summary_metadata_from_ps(
            &document_id.to_string(),
            doc_title,
            filename,
            &ps.metadata,
        ));

        if !result.toc_entries.is_empty() {
            if ensure_ingestion_side_effects_allowed(
                &processor.storage.repo,
                context,
                task,
                document_id,
                "toc writes",
            )
            .await
            .is_ok()
            {
                if let Err(error) = processor
                    .storage
                    .repo
                    .bootstrap()
                    .replace_document_toc(context, workspace_id, document_id, &result.toc_entries)
                    .await
                {
                    info!(document_id = %document_id, error = %error, "failed to write document toc");
                }
            }
        }

        let meta = result.profile_metadata.clone().unwrap_or_else(|| {
            summary_metadata_from_ps(
                &document_id.to_string(),
                doc_title,
                filename,
                &PsMetadata::default(),
            )
        });

        if ensure_ingestion_side_effects_allowed(
            &processor.storage.repo,
            context,
            task,
            document_id,
            "profile metadata write",
        )
        .await
        .is_ok()
        {
            if let Err(error) = processor
                .storage
                .repo
                .documents()
                .update_document_profile(
                    context,
                    document_id,
                    &meta,
                    Some(&task.task_id),
                    task.lock_token.as_deref(),
                )
                .await
            {
                info!(document_id = %document_id, error = %error, "failed to write document profile metadata");
            }
        }

        if !result.summary_text.trim().is_empty() {
            if ensure_ingestion_side_effects_allowed(
                &processor.storage.repo,
                context,
                task,
                document_id,
                "summary update",
            )
            .await
            .is_ok()
            {
                let summary = common::SummaryOutput {
                    summary_text: result.summary_text.clone(),
                    summary_metadata: meta,
                };
                if let Err(error) = processor
                    .storage
                    .repo
                    .documents()
                    .update_document_summary(
                        context,
                        document_id,
                        &summary,
                        Some(&task.task_id),
                        task.lock_token.as_deref(),
                    )
                    .await
                {
                    info!(document_id = %document_id, error = %error, "failed to update document summary");
                }
            }
        }
    }

    // Only hand off pending triplets when the feature is on (otherwise empty + no paid turn).
    if run_triplets {
        parse_run_state.pending_triplets = Some(result.triplets.clone());
    } else {
        parse_run_state.pending_triplets = None;
    }
    result
}

async fn merge_ps_with_llm(
    processor: &PgTaskProcessor,
    windows: &[ProfileSummaryJson],
) -> Result<(ProfileSummaryJson, avrag_llm::LlmUsage)> {
    let Some(llm) = processor.llm.ingestion_llm.clone() else {
        anyhow::bail!("no ingestion llm");
    };
    let payload = serde_json::to_string(windows)?;
    let user = format!("{PS_MERGE_PROMPT}\n\nWindow JSONs:\n{payload}");
    let messages = vec![
        avrag_llm::ChatMessage::system(PS_MERGE_SYSTEM),
        avrag_llm::ChatMessage::user(user),
    ];
    let response = llm
        .complete_with_max_tokens(&messages, Some(SUMMARY_TEMPERATURE), 8_192)
        .await?;
    let usage = response.usage.clone();
    Ok((parse_ps_json(&response.content)?, usage))
}

/// Non-LLM fallback: union metadata (prefer non-empty), join summaries, concatenate sections.
fn merge_ps_deterministic(windows: &[ProfileSummaryJson]) -> ProfileSummaryJson {
    let mut meta = PsMetadata::default();
    let mut summaries = Vec::new();
    let mut sections = Vec::new();
    for (i, w) in windows.iter().enumerate() {
        fill_meta_prefer_nonempty(&mut meta, &w.metadata);
        let s = w.summary.trim();
        if !s.is_empty() {
            if windows.len() > 1 {
                summaries.push(format!("[window {}/{}]\n{s}", i + 1, windows.len()));
            } else {
                summaries.push(s.to_string());
            }
        }
        sections.extend(w.sections.iter().cloned());
    }
    // re-rank flat list order after concat
    for (i, sec) in sections.iter_mut().enumerate() {
        sec.rank = i as i32;
    }
    ProfileSummaryJson {
        metadata: meta,
        summary: summaries.join("\n\n"),
        sections,
    }
}

fn fill_meta_prefer_nonempty(dst: &mut PsMetadata, src: &PsMetadata) {
    fn take(dst: &mut Option<String>, src: &Option<String>) {
        if dst.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true) {
            if let Some(s) = src {
                if !s.trim().is_empty() && s.trim() != "unknown" {
                    *dst = Some(s.clone());
                }
            }
        }
    }
    take(&mut dst.language, &src.language);
    take(&mut dst.domain, &src.domain);
    take(&mut dst.genre, &src.genre);
    take(&mut dst.era, &src.era);
    take(&mut dst.author, &src.author);
    take(&mut dst.publication_date, &src.publication_date);
    take(&mut dst.title, &src.title);
}

fn parse_ps_json(content: &str) -> Result<ProfileSummaryJson> {
    let normalized = strip_json_fences(content);
    Ok(serde_json::from_str(&normalized)?)
}

fn strip_json_fences(content: &str) -> String {
    let trimmed = content.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }
    let mut body = String::new();
    for line in trimmed.lines().skip(1) {
        if line.trim() == "```" {
            break;
        }
        if !body.is_empty() {
            body.push('\n');
        }
        body.push_str(line);
    }
    body.trim().to_string()
}

fn toc_from_ps_sections(sections: &[PsSection]) -> Vec<TocEntry> {
    let mut entries = Vec::new();
    flatten_sections(sections, None, &mut entries);
    // Re-rank document order
    for (i, e) in entries.iter_mut().enumerate() {
        e.rank = i as i32;
    }
    entries
}

fn flatten_sections(
    sections: &[PsSection],
    parent_id: Option<Uuid>,
    out: &mut Vec<TocEntry>,
) {
    for section in sections {
        let id = Uuid::new_v4();
        let level = section.heading_level.clamp(1, 6);
        out.push(TocEntry {
            id,
            parent_id,
            title: section.title.trim().to_string(),
            heading_level: level,
            page: None,
            chunk_id: None,
            rank: section.rank,
            overview: section
                .overview
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        });
        if !section.children.is_empty() {
            flatten_sections(&section.children, Some(id), out);
        }
    }
}

fn summary_metadata_from_ps(
    doc_id: &str,
    title: &str,
    filename: &str,
    meta: &PsMetadata,
) -> SummaryMetadata {
    let language = meta
        .language
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("unknown")
        .to_string();
    SummaryMetadata {
        doc_id: doc_id.to_string(),
        filename: filename.to_string(),
        docname: meta
            .title
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(title)
            .to_string(),
        language,
        domain: meta
            .domain
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(Domain::from)
            .unwrap_or(Domain::Unknown),
        genre: meta
            .genre
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(Genre::from)
            .unwrap_or(Genre::Unknown),
        era: meta
            .era
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(Era::from)
            .unwrap_or(Era::Unknown),
        author: meta.author.clone().filter(|s| !s.trim().is_empty()),
        publication_date: meta
            .publication_date
            .clone()
            .filter(|s| !s.trim().is_empty()),
    }
}
