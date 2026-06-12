use app_bootstrap::AppState;
use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::RequestState;
use super::super::{app_error_response, error_response};
use super::notes::load_notebook_notes;

fn pinned_source_count(preferences: &contracts::preferences::UserPreferences, notebook_id: &str) -> i64 {
    preferences
        .dashboard
        .workspace_preferences
        .iter()
        .find(|pref| pref.notebook_id == notebook_id)
        .map(|pref| pref.pinned_source_ids.len() as i64)
        .unwrap_or(0)
}

struct NotebookAnalysisCollector;

impl NotebookAnalysisCollector {
    fn collect_overview(
        &self,
        notebook: &contracts::notebooks::Notebook,
    ) -> contracts::notebooks::NotebookAnalysisOverview {
        contracts::notebooks::NotebookAnalysisOverview {
            title: notebook.title.clone(),
            description: notebook.description.clone(),
            updated_at: notebook.updated_at.clone(),
            document_count: notebook.document_count,
        }
    }

    fn collect_sources(
        &self,
        sources: &[common::Document],
        preferences: &contracts::preferences::UserPreferences,
        notebook_id: &str,
    ) -> contracts::notebooks::NotebookAnalysisSources {
        let (mut ready, mut failed) = (0i64, 0i64);
        for source in sources {
            match source.status {
                contracts::documents::DocumentStatus::Completed => ready += 1,
                contracts::documents::DocumentStatus::Failed => failed += 1,
                _ => {}
            }
        }
        let processing = (sources.len() as i64) - ready - failed;
        let pinned = pinned_source_count(preferences, notebook_id);
        contracts::notebooks::NotebookAnalysisSources {
            total: sources.len() as i64,
            ready,
            processing: processing.max(0),
            failed,
            selected: 0,
            pinned,
        }
    }

    fn collect_threads(
        &self,
        sessions: &[contracts::notebooks::ChatSession],
    ) -> contracts::notebooks::NotebookAnalysisThreads {
        let latest = sessions
            .iter()
            .max_by(|a, b| a.updated_at.cmp(&b.updated_at));
        contracts::notebooks::NotebookAnalysisThreads {
            total: sessions.len() as i64,
            pinned: sessions.iter().filter(|s| s.pinned).count() as i64,
            latest_activity_at: latest.map(|s| s.updated_at.clone()),
            latest_mode: latest.map(|s| s.agent_type.clone()),
        }
    }

    fn collect_notes(
        &self,
        notes: &[contracts::notebooks::NotebookNote],
    ) -> contracts::notebooks::NotebookAnalysisNotes {
        let promoted = notes
            .iter()
            .filter(|n| n.promoted_document_id.is_some())
            .count() as i64;
        let latest = notes.iter().map(|n| n.updated_at.clone()).max();
        contracts::notebooks::NotebookAnalysisNotes {
            total: notes.len() as i64,
            latest_edited_at: latest,
            promoted_to_source: promoted,
        }
    }

    async fn collect_access(
        &self,
        state: &AppState,
        notebook_id: &str,
    ) -> contracts::notebooks::NotebookAnalysisAccess {
        let notebook_id = notebook_id.to_string();
        let (member_count, share_enabled, active_api_key_count) = tokio::join!(
            async { state.share_member_count(&notebook_id).await },
            async { state.share_enabled_for_notebook(&notebook_id).await },
            async {
                state
                    .list_api_keys(&notebook_id)
                    .await
                    .map(|items| items.into_iter().filter(|k| k.is_active).count() as i64)
                    .unwrap_or(0)
            },
        );
        contracts::notebooks::NotebookAnalysisAccess {
            share_enabled,
            member_count,
            active_api_key_count,
        }
    }

    fn build_alerts(
        &self,
        sources: &contracts::notebooks::NotebookAnalysisSources,
        sessions: &[contracts::notebooks::ChatSession],
        notes: &[contracts::notebooks::NotebookNote],
        notes_summary: &contracts::notebooks::NotebookAnalysisNotes,
    ) -> Vec<contracts::notebooks::NotebookAnalysisAlert> {
        let mut alerts = Vec::new();
        if sources.ready == 0 {
            alerts.push(contracts::notebooks::NotebookAnalysisAlert {
                level: "warning".to_string(),
                code: "no_ready_sources".to_string(),
                message: "No ready sources are available for RAG chat.".to_string(),
            });
        }
        if sources.failed > 0 {
            alerts.push(contracts::notebooks::NotebookAnalysisAlert {
                level: "warning".to_string(),
                code: "failed_sources".to_string(),
                message: format!("{} sources need attention or reindexing.", sources.failed),
            });
        }
        if sessions.is_empty() {
            alerts.push(contracts::notebooks::NotebookAnalysisAlert {
                level: "info".to_string(),
                code: "no_threads".to_string(),
                message: "This notebook does not have any threads yet.".to_string(),
            });
        }
        if !notes.is_empty() && notes_summary.promoted_to_source == 0 {
            alerts.push(contracts::notebooks::NotebookAnalysisAlert {
                level: "info".to_string(),
                code: "notes_not_promoted".to_string(),
                message: "Notes exist, but none have been promoted into shared sources yet."
                    .to_string(),
            });
        }
        alerts
    }
}

pub(crate) async fn get_notebook_analysis_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(notebook_id): Path<String>,
) -> Response {
    let Some(notebook) = state.get_notebook(&notebook_id).await else {
        return error_response(StatusCode::NOT_FOUND, "not_found", "Notebook not found");
    };

    let sources = state.list_documents(Some(&notebook_id), None).await;
    let sessions = state.list_sessions(Some(&notebook_id)).await;
    let preferences = match state.current_user_preferences().await {
        Ok(preferences) => preferences,
        Err(error) => return app_error_response(error),
    };
    let notes = match load_notebook_notes(&state, &notebook_id).await {
        Ok(notes) => notes,
        Err(error) => return app_error_response(error),
    };

    let collector = NotebookAnalysisCollector;
    let overview = collector.collect_overview(&notebook);
    let sources_summary = collector.collect_sources(&sources, &preferences, &notebook_id);
    let threads = collector.collect_threads(&sessions);
    let notes_summary = collector.collect_notes(&notes);
    let access = collector.collect_access(&state, &notebook_id).await;
    let alerts = collector.build_alerts(&sources_summary, &sessions, &notes, &notes_summary);

    (
        StatusCode::OK,
        Json(contracts::notebooks::NotebookAnalysisResponse {
            overview,
            sources: sources_summary,
            threads,
            notes: notes_summary,
            access,
            alerts,
        }),
    )
        .into_response()
}
