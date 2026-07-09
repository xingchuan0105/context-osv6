use app_bootstrap::AppState;
use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use super::super::{app_error_response, error_response};
use super::notes::load_workspace_notes;
use crate::middleware::RequestState;
use crate::auth_guard::{ensure_user_workspace_access, require_user_session};

fn pinned_source_count(
    preferences: &contracts::preferences::UserPreferences,
    workspace_id: &str,
) -> i64 {
    preferences
        .dashboard
        .workspace_preferences
        .iter()
        .find(|pref| pref.workspace_id == workspace_id)
        .map(|pref| pref.pinned_source_ids.len() as i64)
        .unwrap_or(0)
}

struct WorkspaceAnalysisCollector;

impl WorkspaceAnalysisCollector {
    fn collect_overview(
        &self,
        notebook: &contracts::workspaces::Workspace,
    ) -> contracts::workspaces::WorkspaceAnalysisOverview {
        contracts::workspaces::WorkspaceAnalysisOverview {
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
        workspace_id: &str,
    ) -> contracts::workspaces::WorkspaceAnalysisSources {
        let (mut ready, mut failed) = (0i64, 0i64);
        for source in sources {
            match source.status {
                contracts::documents::DocumentStatus::Completed => ready += 1,
                contracts::documents::DocumentStatus::Failed => failed += 1,
                _ => {}
            }
        }
        let processing = (sources.len() as i64) - ready - failed;
        let pinned = pinned_source_count(preferences, workspace_id);
        contracts::workspaces::WorkspaceAnalysisSources {
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
        sessions: &[contracts::workspaces::ChatSession],
    ) -> contracts::workspaces::WorkspaceAnalysisThreads {
        let latest = sessions
            .iter()
            .max_by(|a, b| a.updated_at.cmp(&b.updated_at));
        contracts::workspaces::WorkspaceAnalysisThreads {
            total: sessions.len() as i64,
            pinned: sessions.iter().filter(|s| s.pinned).count() as i64,
            latest_activity_at: latest.map(|s| s.updated_at.clone()),
            latest_mode: latest.map(|s| s.agent_type.clone()),
        }
    }

    fn collect_notes(
        &self,
        notes: &[contracts::workspaces::WorkspaceNote],
    ) -> contracts::workspaces::WorkspaceAnalysisNotes {
        let promoted = notes
            .iter()
            .filter(|n| n.promoted_document_id.is_some())
            .count() as i64;
        let latest = notes.iter().map(|n| n.updated_at.clone()).max();
        contracts::workspaces::WorkspaceAnalysisNotes {
            total: notes.len() as i64,
            latest_edited_at: latest,
            promoted_to_source: promoted,
        }
    }

    async fn collect_access(
        &self,
        state: &AppState,
        workspace_id: &str,
    ) -> contracts::workspaces::WorkspaceAnalysisAccess {
        let workspace_id = workspace_id.to_string();
        let (member_count, share_enabled, active_api_key_count) = tokio::join!(
            async { state.share().share_member_count(&workspace_id).await },
            async { state.share().share_enabled_for_workspace(&workspace_id).await },
            async {
                state.admin_api()
                    .list_api_keys(&workspace_id)
                    .await
                    .map(|items| items.into_iter().filter(|k| k.is_active).count() as i64)
                    .unwrap_or(0)
            },
        );
        contracts::workspaces::WorkspaceAnalysisAccess {
            share_enabled,
            member_count,
            active_api_key_count,
        }
    }

    fn build_alerts(
        &self,
        sources: &contracts::workspaces::WorkspaceAnalysisSources,
        sessions: &[contracts::workspaces::ChatSession],
        notes: &[contracts::workspaces::WorkspaceNote],
        notes_summary: &contracts::workspaces::WorkspaceAnalysisNotes,
    ) -> Vec<contracts::workspaces::WorkspaceAnalysisAlert> {
        let mut alerts = Vec::new();
        if sources.ready == 0 {
            alerts.push(contracts::workspaces::WorkspaceAnalysisAlert {
                level: "warning".to_string(),
                code: "no_ready_sources".to_string(),
                message: "No ready sources are available for RAG chat.".to_string(),
            });
        }
        if sources.failed > 0 {
            alerts.push(contracts::workspaces::WorkspaceAnalysisAlert {
                level: "warning".to_string(),
                code: "failed_sources".to_string(),
                message: format!("{} sources need attention or reindexing.", sources.failed),
            });
        }
        if sessions.is_empty() {
            alerts.push(contracts::workspaces::WorkspaceAnalysisAlert {
                level: "info".to_string(),
                code: "no_threads".to_string(),
                message: "This notebook does not have any threads yet.".to_string(),
            });
        }
        if !notes.is_empty() && notes_summary.promoted_to_source == 0 {
            alerts.push(contracts::workspaces::WorkspaceAnalysisAlert {
                level: "info".to_string(),
                code: "notes_not_promoted".to_string(),
                message: "Notes exist, but none have been promoted into shared sources yet."
                    .to_string(),
            });
        }
        alerts
    }
}

pub(crate) async fn get_workspace_analysis_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(workspace_id): Path<String>,
) -> Response {
    if let Err(error) = require_user_session(
        state.auth(),
        "notebook analysis requires a signed-in user session",
    ) {
        return app_error_response(error);
    }
    if let Err(error) = ensure_user_workspace_access(&state, &workspace_id).await {
        return app_error_response(error);
    }
    let Some(notebook) = state.docs().get_workspace(&workspace_id).await else {
        return error_response(StatusCode::NOT_FOUND, "not_found", "Workspace not found");
    };

    let docs = state.docs();
    let chat = state.agent();
    let prefs = state.prefs();
    let (sources, sessions, preferences, notes) = tokio::join!(
        docs.list_documents(Some(&workspace_id), None),
        chat.list_sessions(Some(&workspace_id)),
        prefs.current(),
        load_workspace_notes(&state, &workspace_id),
    );
    let preferences = match preferences {
        Ok(preferences) => preferences,
        Err(error) => return app_error_response(error),
    };
    let notes = match notes {
        Ok(notes) => notes,
        Err(error) => return app_error_response(error),
    };

    let collector = WorkspaceAnalysisCollector;
    let overview = collector.collect_overview(&notebook);
    let sources_summary = collector.collect_sources(&sources, &preferences, &workspace_id);
    let threads = collector.collect_threads(&sessions);
    let notes_summary = collector.collect_notes(&notes);
    let access = collector.collect_access(&state, &workspace_id).await;
    let alerts = collector.build_alerts(&sources_summary, &sessions, &notes, &notes_summary);

    (
        StatusCode::OK,
        Json(contracts::workspaces::WorkspaceAnalysisResponse {
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
