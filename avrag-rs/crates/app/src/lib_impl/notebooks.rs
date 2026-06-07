use common::{
    AppError, CreateNotebookRequest, Notebook, StatusOnlyResponse, UpdateNotebookRequest, new_id,
    now_rfc3339,
};
use uuid::Uuid;

use crate::lib_impl::*;

impl AppState {
    pub async fn list_notebooks(&self) -> Vec<Notebook> {
        if let Some(pg) = &self.pg {
            return pg.list_notebooks(&self.auth).await.unwrap_or_default();
        }
        let state = self.inner.read().await;
        state
            .notebooks
            .values()
            .filter(|notebook| notebook.org_id == self.current_org_id())
            .cloned()
            .collect()
    }

    pub async fn get_notebook(&self, notebook_id: &str) -> Option<Notebook> {
        if let Some(pg) = &self.pg {
            let notebook_id = Uuid::parse_str(notebook_id).ok()?;
            return pg
                .get_notebook(&self.auth, notebook_id)
                .await
                .ok()
                .flatten();
        }
        let state = self.inner.read().await;
        state
            .notebooks
            .get(notebook_id)
            .filter(|notebook| notebook.org_id == self.current_org_id())
            .cloned()
    }

    pub async fn create_notebook(&self, req: CreateNotebookRequest) -> Result<Notebook, AppError> {
        if req.name.trim().is_empty() {
            return Err(AppError::validation(
                "name_required",
                "notebook name is required",
            ));
        }

        if let Some(pg) = &self.pg {
            let notebook = pg
                .create_notebook(&self.auth, req.name.trim(), req.description.trim())
                .await
                .map_err(map_pg_error)?;
            self.record_product_event_if_available(
                analytics::ProductEventName::NotebookCreated,
                analytics::Surface::Workspace,
                analytics::ResultTag::Success,
                None,
                Uuid::parse_str(&notebook.id).ok(),
                serde_json::json!({
                    "name": notebook.name.clone(),
                    "runtime_mode": self.runtime_mode(),
                }),
            )
            .await;
            return Ok(notebook);
        }

        let now = now_rfc3339();
        let notebook = Notebook {
            id: new_id(),
            org_id: self.current_org_id(),
            owner_id: self.current_user_id(),
            name: req.name.trim().to_string(),
            title: req.name.trim().to_string(),
            description: req.description.trim().to_string(),
            document_count: 0,
            status_summary: std::collections::HashMap::new(),
            shared: false,
            created_at: now.clone(),
            updated_at: now,
        };

        let mut state = self.inner.write().await;
        state
            .notebooks
            .insert(notebook.id.clone(), notebook.clone());
        self.record_product_event_if_available(
            analytics::ProductEventName::NotebookCreated,
            analytics::Surface::Workspace,
            analytics::ResultTag::Success,
            None,
            Uuid::parse_str(&notebook.id).ok(),
            serde_json::json!({
                "name": notebook.name.clone(),
                "runtime_mode": self.runtime_mode(),
            }),
        )
        .await;
        Ok(notebook)
    }

    pub async fn update_notebook(
        &self,
        notebook_id: &str,
        req: UpdateNotebookRequest,
    ) -> Result<Notebook, AppError> {
        if let Some(pg) = &self.pg {
            let notebook_id =
                parse_uuid_or_app_error(notebook_id, "notebook_not_found", "notebook not found")?;
            return pg
                .update_notebook(
                    &self.auth,
                    notebook_id,
                    req.name.trim(),
                    req.description.trim(),
                )
                .await
                .map_err(map_pg_error)?
                .ok_or_else(|| AppError::not_found("notebook_not_found", "notebook not found"));
        }

        let mut state = self.inner.write().await;
        let notebook = state
            .notebooks
            .get_mut(notebook_id)
            .ok_or_else(|| AppError::not_found("notebook_not_found", "notebook not found"))?;
        if notebook.org_id != self.current_org_id() {
            return Err(AppError::not_found(
                "notebook_not_found",
                "notebook not found",
            ));
        }

        if !req.name.trim().is_empty() {
            notebook.name = req.name.trim().to_string();
            notebook.title = notebook.name.clone();
        }
        notebook.description = req.description.trim().to_string();
        notebook.updated_at = now_rfc3339();
        Ok(notebook.clone())
    }

    pub async fn delete_notebook(&self, notebook_id: &str) -> Result<StatusOnlyResponse, AppError> {
        if let Some(pg) = &self.pg {
            let notebook_id =
                parse_uuid_or_app_error(notebook_id, "notebook_not_found", "notebook not found")?;
            let deleted = pg
                .delete_notebook(&self.auth, notebook_id)
                .await
                .map_err(map_pg_error)?;
            if !deleted {
                return Err(AppError::not_found(
                    "notebook_not_found",
                    "notebook not found",
                ));
            }
            return Ok(StatusOnlyResponse {
                status: "deleted".to_string(),
            });
        }

        let mut state = self.inner.write().await;
        let can_delete = state
            .notebooks
            .get(notebook_id)
            .map(|notebook| notebook.org_id == self.current_org_id())
            .unwrap_or(false);
        if !can_delete {
            return Err(AppError::not_found(
                "notebook_not_found",
                "notebook not found",
            ));
        }
        state.notebooks.remove(notebook_id);

        state
            .documents
            .retain(|_, stored| stored.document.notebook_id != notebook_id);
        let removed_sessions: Vec<String> = state
            .sessions
            .iter()
            .filter_map(|(id, session)| (session.notebook_id == notebook_id).then_some(id.clone()))
            .collect();
        for session_id in removed_sessions {
            state.sessions.remove(&session_id);
            state.messages.remove(&session_id);
        }
        Ok(StatusOnlyResponse {
            status: "deleted".to_string(),
        })
    }
}
