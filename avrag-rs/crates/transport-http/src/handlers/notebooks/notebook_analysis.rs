//! Pure domain logic for assembling a notebook's analysis summary.
//!
//! Previously this logic lived inline inside the HTTP handler
//! (`get_notebook_analysis_handler`), mixing pure data aggregation with HTTP
//! plumbing. It is extracted here so the handler stays a thin transport layer
//! and the aggregation is independently testable.
//!
//! `collect_access` is deliberately kept in the handler because it fans out to
//! `AppState` service calls; everything here is a pure transformation over
//! contract/common types.

use common::Document;
use contracts::documents::DocumentStatus;
use contracts::notebooks::{
    ChatSession, Notebook, NotebookAnalysisAlert, NotebookAnalysisNotes, NotebookAnalysisOverview,
    NotebookAnalysisSources, NotebookAnalysisThreads,
};
use contracts::preferences::UserPreferences;

/// Count pinned sources for a notebook from the user's preferences.
fn pinned_source_count(preferences: &UserPreferences, notebook_id: &str) -> i64 {
    preferences
        .dashboard
        .workspace_preferences
        .iter()
        .find(|pref| pref.notebook_id == notebook_id)
        .map(|pref| pref.pinned_source_ids.len() as i64)
        .unwrap_or(0)
}

pub(super) fn collect_overview(notebook: &Notebook) -> NotebookAnalysisOverview {
    NotebookAnalysisOverview {
        title: notebook.title.clone(),
        description: notebook.description.clone(),
        updated_at: notebook.updated_at.clone(),
        document_count: notebook.document_count,
    }
}

pub(super) fn collect_sources(
    sources: &[Document],
    preferences: &UserPreferences,
    notebook_id: &str,
) -> NotebookAnalysisSources {
    let (mut ready, mut failed) = (0i64, 0i64);
    for source in sources {
        match source.status {
            DocumentStatus::Completed => ready += 1,
            DocumentStatus::Failed => failed += 1,
            _ => {}
        }
    }
    let processing = (sources.len() as i64) - ready - failed;
    let pinned = pinned_source_count(preferences, notebook_id);
    NotebookAnalysisSources {
        total: sources.len() as i64,
        ready,
        processing: processing.max(0),
        failed,
        selected: 0,
        pinned,
    }
}

pub(super) fn collect_threads(sessions: &[ChatSession]) -> NotebookAnalysisThreads {
    let latest = sessions
        .iter()
        .max_by(|a, b| a.updated_at.cmp(&b.updated_at));
    NotebookAnalysisThreads {
        total: sessions.len() as i64,
        pinned: sessions.iter().filter(|s| s.pinned).count() as i64,
        latest_activity_at: latest.map(|s| s.updated_at.clone()),
        latest_mode: latest.map(|s| s.agent_type.clone()),
    }
}

pub(super) fn collect_notes(notes: &[contracts::notebooks::NotebookNote]) -> NotebookAnalysisNotes {
    let promoted = notes
        .iter()
        .filter(|n| n.promoted_document_id.is_some())
        .count() as i64;
    let latest = notes.iter().map(|n| n.updated_at.clone()).max();
    NotebookAnalysisNotes {
        total: notes.len() as i64,
        latest_edited_at: latest,
        promoted_to_source: promoted,
    }
}

/// Build user-facing alerts from the aggregated sub-summaries. Pure: no I/O.
pub(super) fn build_alerts(
    sources: &NotebookAnalysisSources,
    sessions: &[ChatSession],
    notes: &[contracts::notebooks::NotebookNote],
    notes_summary: &NotebookAnalysisNotes,
) -> Vec<NotebookAnalysisAlert> {
    let mut alerts = Vec::new();
    if sources.ready == 0 {
        alerts.push(NotebookAnalysisAlert {
            level: "warning".to_string(),
            code: "no_ready_sources".to_string(),
            message: "No ready sources are available for RAG chat.".to_string(),
        });
    }
    if sources.failed > 0 {
        alerts.push(NotebookAnalysisAlert {
            level: "warning".to_string(),
            code: "failed_sources".to_string(),
            message: format!("{} sources need attention or reindexing.", sources.failed),
        });
    }
    if sessions.is_empty() {
        alerts.push(NotebookAnalysisAlert {
            level: "info".to_string(),
            code: "no_threads".to_string(),
            message: "This notebook does not have any threads yet.".to_string(),
        });
    }
    if !notes.is_empty() && notes_summary.promoted_to_source == 0 {
        alerts.push(NotebookAnalysisAlert {
            level: "info".to_string(),
            code: "notes_not_promoted".to_string(),
            message: "Notes exist, but none have been promoted into shared sources yet."
                .to_string(),
        });
    }
    alerts
}
