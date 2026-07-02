use app_core::{AnalyticsServiceCtx, StorageContext, parse_uuid_or_app_error};
use avrag_auth::AuthContext;
use common::{
    AppError, CreateNotebookRequest, StatusOnlyResponse, UpdateNotebookRequest, new_id, now_rfc3339,
};
use contracts::notebooks::Notebook;
use uuid::Uuid;

use crate::analytics_helpers::record_product_event_if_available;
use crate::document_context::DocumentContext;

impl DocumentContext {
    pub async fn list_notebooks(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
    ) -> Vec<Notebook> {
        if let Some(store) = storage.document_store() {
            return store.list_notebooks(auth).await.unwrap_or_default();
        }
        let state = storage.inner().read().await;
        state
            .notebooks
            .values()
            .filter(|notebook| notebook.org_id == StorageContext::current_org_id(auth))
            .cloned()
            .collect()
    }

    pub async fn get_notebook(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        notebook_id: &str,
    ) -> Option<Notebook> {
        if let Some(store) = storage.document_store() {
            let notebook_id = Uuid::parse_str(notebook_id).ok()?;
            let notebook = store.get_notebook(auth, notebook_id).await.ok().flatten()?;
            return (notebook.org_id == StorageContext::current_org_id(auth)).then_some(notebook);
        }
        let state = storage.inner().read().await;
        state
            .notebooks
            .get(notebook_id)
            .filter(|notebook| notebook.org_id == StorageContext::current_org_id(auth))
            .cloned()
    }

    pub async fn create_notebook(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        analytics: &AnalyticsServiceCtx,
        req: CreateNotebookRequest,
    ) -> Result<Notebook, AppError> {
        if req.name.trim().is_empty() {
            return Err(AppError::validation(
                "name_required",
                "notebook name is required",
            ));
        }

        if let Some(store) = storage.document_store() {
            let notebook = store
                .create_notebook(auth, req.name.trim(), req.description.trim())
                .await?;
            record_product_event_if_available(
                auth,
                analytics,
                analytics::ProductEventName::NotebookCreated,
                analytics::Surface::Workspace,
                analytics::ResultTag::Success,
                None,
                Uuid::parse_str(&notebook.id).ok(),
                serde_json::json!({
                    "name": notebook.name.clone(),
                    "runtime_mode": storage.runtime_mode(),
                }),
            )
            .await;
            return Ok(notebook);
        }

        let now = now_rfc3339();
        let notebook = Notebook {
            id: new_id(),
            org_id: StorageContext::current_org_id(auth),
            owner_id: StorageContext::current_user_id(auth),
            name: req.name.trim().to_string(),
            title: req.name.trim().to_string(),
            description: req.description.trim().to_string(),
            document_count: 0,
            status_summary: std::collections::HashMap::new(),
            shared: false,
            created_at: now.clone(),
            updated_at: now,
        };

        {
            let mut state = storage.inner().write().await;
            state
                .notebooks
                .insert(notebook.id.clone(), notebook.clone());
        }
        record_product_event_if_available(
            auth,
            analytics,
            analytics::ProductEventName::NotebookCreated,
            analytics::Surface::Workspace,
            analytics::ResultTag::Success,
            None,
            Uuid::parse_str(&notebook.id).ok(),
            serde_json::json!({
                "name": notebook.name.clone(),
                "runtime_mode": storage.runtime_mode(),
            }),
        )
        .await;
        Ok(notebook)
    }

    pub async fn update_notebook(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        notebook_id: &str,
        req: UpdateNotebookRequest,
    ) -> Result<Notebook, AppError> {
        if let Some(store) = storage.document_store() {
            let notebook_id =
                parse_uuid_or_app_error(notebook_id, "notebook_not_found", "notebook not found")?;
            return store
                .update_notebook(
                    auth,
                    notebook_id,
                    Some(req.name.trim()),
                    Some(req.description.trim()),
                )
                .await?
                .ok_or_else(|| AppError::not_found("notebook_not_found", "notebook not found"));
        }

        let mut state = storage.inner().write().await;
        let notebook = state
            .notebooks
            .get_mut(notebook_id)
            .ok_or_else(|| AppError::not_found("notebook_not_found", "notebook not found"))?;
        if notebook.org_id != StorageContext::current_org_id(auth) {
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

    pub async fn delete_notebook(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        notebook_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        if let Some(store) = storage.document_store() {
            let notebook_id =
                parse_uuid_or_app_error(notebook_id, "notebook_not_found", "notebook not found")?;
            let deleted = store.delete_notebook(auth, notebook_id).await?;
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

        let mut state = storage.inner().write().await;
        let can_delete = state
            .notebooks
            .get(notebook_id)
            .map(|notebook| notebook.org_id == StorageContext::current_org_id(auth))
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
