use app::AppState;

fn pinned_source_count(preferences: &common::UserPreferences, notebook_id: &str) -> i64 {
    preferences
        .dashboard
        .workspace_preferences
        .iter()
        .find(|pref| pref.notebook_id == notebook_id)
        .map(|pref| pref.pinned_source_ids.len() as i64)
        .unwrap_or(0)
}

pub(super) struct NotebookAnalysisCollector;

impl NotebookAnalysisCollector {
    pub(super) fn collect_overview(
        &self,
        notebook: &common::Notebook,
    ) -> common::NotebookAnalysisOverview {
        common::NotebookAnalysisOverview {
            title: notebook.title.clone(),
            description: notebook.description.clone(),
            updated_at: notebook.updated_at.clone(),
            document_count: notebook.document_count,
        }
    }

    pub(super) fn collect_sources(
        &self,
        sources: &[common::Document],
        preferences: &common::UserPreferences,
        notebook_id: &str,
    ) -> common::NotebookAnalysisSources {
        let (mut ready, mut failed) = (0i64, 0i64);
        for source in sources {
            match source.status {
                common::DocumentStatus::Completed => ready += 1,
                common::DocumentStatus::Failed => failed += 1,
                _ => {}
            }
        }
        let processing = (sources.len() as i64) - ready - failed;
        let pinned = pinned_source_count(preferences, notebook_id);
        common::NotebookAnalysisSources {
            total: sources.len() as i64,
            ready,
            processing: processing.max(0),
            failed,
            selected: 0,
            pinned,
        }
    }

    pub(super) fn collect_threads(
        &self,
        sessions: &[common::ChatSession],
    ) -> common::NotebookAnalysisThreads {
        let latest = sessions
            .iter()
            .max_by(|a, b| a.updated_at.cmp(&b.updated_at));
        common::NotebookAnalysisThreads {
            total: sessions.len() as i64,
            pinned: sessions.iter().filter(|s| s.pinned).count() as i64,
            latest_activity_at: latest.map(|s| s.updated_at.clone()),
            latest_mode: latest.map(|s| s.agent_type.clone()),
        }
    }

    pub(super) fn collect_notes(
        &self,
        notes: &[common::NotebookNote],
    ) -> common::NotebookAnalysisNotes {
        let promoted = notes
            .iter()
            .filter(|n| n.promoted_document_id.is_some())
            .count() as i64;
        let latest = notes.iter().map(|n| n.updated_at.clone()).max();
        common::NotebookAnalysisNotes {
            total: notes.len() as i64,
            latest_edited_at: latest,
            promoted_to_source: promoted,
        }
    }

    pub(super) async fn collect_access(
        &self,
        state: &AppState,
        notebook_id: &str,
    ) -> common::NotebookAnalysisAccess {
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
        common::NotebookAnalysisAccess {
            share_enabled,
            member_count,
            active_api_key_count,
        }
    }

    pub(super) fn build_alerts(
        &self,
        sources: &common::NotebookAnalysisSources,
        sessions: &[common::ChatSession],
        notes: &[common::NotebookNote],
        notes_summary: &common::NotebookAnalysisNotes,
    ) -> Vec<common::NotebookAnalysisAlert> {
        let mut alerts = Vec::new();
        if sources.ready == 0 {
            alerts.push(common::NotebookAnalysisAlert {
                level: "warning".to_string(),
                code: "no_ready_sources".to_string(),
                message: "No ready sources are available for RAG chat.".to_string(),
            });
        }
        if sources.failed > 0 {
            alerts.push(common::NotebookAnalysisAlert {
                level: "warning".to_string(),
                code: "failed_sources".to_string(),
                message: format!("{} sources need attention or reindexing.", sources.failed),
            });
        }
        if sessions.is_empty() {
            alerts.push(common::NotebookAnalysisAlert {
                level: "info".to_string(),
                code: "no_threads".to_string(),
                message: "This notebook does not have any threads yet.".to_string(),
            });
        }
        if !notes.is_empty() && notes_summary.promoted_to_source == 0 {
            alerts.push(common::NotebookAnalysisAlert {
                level: "info".to_string(),
                code: "notes_not_promoted".to_string(),
                message: "Notes exist, but none have been promoted into shared sources yet."
                    .to_string(),
            });
        }
        alerts
    }
}
