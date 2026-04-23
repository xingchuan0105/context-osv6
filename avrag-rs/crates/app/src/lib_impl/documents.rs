impl AppState {
    pub async fn list_documents(
        &self,
        notebook_id: Option<&str>,
        document_id: Option<&str>,
    ) -> Vec<Document> {
        if let Some(pg) = &self.pg {
            let notebook_uuid = notebook_id.and_then(|value| Uuid::parse_str(value).ok());
            let document_uuid = document_id.and_then(|value| Uuid::parse_str(value).ok());
            return pg
                .list_documents(&self.auth, notebook_uuid, document_uuid)
                .await
                .unwrap_or_default();
        }
        let state = self.inner.read().await;
        state
            .documents
            .values()
            .filter(|stored| {
                stored.document.org_id == self.current_org_id()
                    && notebook_id
                        .map(|id| stored.document.notebook_id == id)
                        .unwrap_or(true)
                    && document_id
                        .map(|id| stored.document.id == id)
                        .unwrap_or(true)
            })
            .map(|stored| stored.document.clone())
            .collect()
    }

    pub async fn create_document_upload(
        &self,
        notebook_id: &str,
        req: CreateDocumentRequest,
    ) -> Result<CreateDocumentUploadResponse, AppError> {
        if req.filename.trim().is_empty() {
            return Err(AppError::validation(
                "filename_required",
                "filename is required",
            ));
        }
        ingestion::parser::ParseRouter::ensure_supported_file_type(
            req.filename.trim(),
            &req.mime_type,
        )
        .map_err(|error| AppError::validation(error.code(), error.to_string()))?;

        if let Some(pg) = &self.pg {
            self.ensure_metric_quota("storage_bytes", req.file_size as i64)
                .await?;
            let notebook_id =
                parse_uuid_or_app_error(notebook_id, "notebook_not_found", "notebook not found")?;
            let notebook = pg
                .get_notebook(&self.auth, notebook_id)
                .await
                .map_err(map_pg_error)?;
            if notebook.is_none() {
                return Err(AppError::not_found(
                    "notebook_not_found",
                    "notebook not found",
                ));
            }
            let document = pg
                .create_document(
                    &self.auth,
                    notebook_id,
                    req.filename.trim(),
                    req.file_size,
                    &req.mime_type,
                )
                .await
                .map_err(map_pg_error)?;
            let seed = pg
                .get_document_task_seed(
                    &self.auth,
                    parse_uuid_or_app_error(
                        &document.id,
                        "document_not_found",
                        "document not found",
                    )?,
                )
                .await
                .map_err(map_pg_error)?
                .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
            self.record_product_event_if_available(
                analytics::ProductEventName::DocumentUploadStarted,
                analytics::Surface::Workspace,
                analytics::ResultTag::Success,
                None,
                Some(notebook_id),
                serde_json::json!({
                    "document_id": document.id.clone(),
                    "filename": req.filename.trim(),
                    "file_size": req.file_size,
                    "mime_type": req.mime_type,
                    "source": "file_upload",
                }),
            )
            .await;
            self.record_storage_cost_event_if_available(
                analytics::CostEventName::UploadBytesMetered,
                "upload",
                Some(notebook_id),
                req.file_size as i64,
                "file_upload",
                serde_json::json!({
                    "document_id": document.id.clone(),
                    "filename": req.filename.trim(),
                    "mime_type": req.mime_type,
                }),
            )
            .await;
            return Ok(CreateDocumentUploadResponse {
                document_id: document.id.clone(),
                upload_url: self.signed_upload_url(&document.id, &seed.object_path, None)?,
                status: "pending".to_string(),
            });
        }

        let notebook = self
            .get_notebook(notebook_id)
            .await
            .ok_or_else(|| AppError::not_found("notebook_not_found", "notebook not found"))?;

        let now = now_rfc3339();
        let document_id = new_id();
        let mime_type = req.mime_type.clone();
        let document = Document {
            id: document_id.clone(),
            org_id: self.current_org_id(),
            notebook_id: notebook.id.clone(),
            owner_id: self.current_user_id(),
            file_name: req.filename.trim().to_string(),
            mime_type,
            file_size: req.file_size,
            status: DocumentStatus::Pending,
            chunk_count: 0,
            created_at: now.clone(),
            updated_at: now,
        };

        let stored = StoredDocument {
            document,
            content: String::new(),
            summary: None,
            parsed_items: Vec::new(),
        };

        {
            let mut state = self.inner.write().await;
            state.documents.insert(document_id.clone(), stored);
        }
        self.record_product_event_if_available(
            analytics::ProductEventName::DocumentUploadStarted,
            analytics::Surface::Workspace,
            analytics::ResultTag::Success,
            None,
            Uuid::parse_str(notebook_id).ok(),
            serde_json::json!({
                "document_id": document_id,
                "filename": req.filename.trim(),
                "file_size": req.file_size,
                "mime_type": req.mime_type,
                "source": "file_upload",
            }),
        )
        .await;
        self.record_storage_cost_event_if_available(
            analytics::CostEventName::UploadBytesMetered,
            "upload",
            Uuid::parse_str(notebook_id).ok(),
            req.file_size as i64,
            "file_upload",
            serde_json::json!({
                "document_id": document_id.clone(),
                "filename": req.filename.trim(),
                "mime_type": req.mime_type,
            }),
        )
        .await;

        Ok(CreateDocumentUploadResponse {
            document_id: document_id.clone(),
            upload_url: self.signed_upload_url(
                &document_id,
                &format!(
                    "{}/{}/{}/{}",
                    self.current_org_id(),
                    notebook_id,
                    document_id,
                    sanitize_filename(req.filename.trim())
                ),
                None,
            )?,
            status: "pending".to_string(),
        })
    }

    pub async fn put_uploaded_document(
        &self,
        document_id: &str,
        body: Vec<u8>,
    ) -> Result<StatusOnlyResponse, AppError> {
        if let Some(pg) = &self.pg {
            let document_id =
                parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
            let seed = pg
                .get_document_task_seed(&self.auth, document_id)
                .await
                .map_err(map_pg_error)?
                .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
            self.object_store
                .put(&seed.object_path, &body)
                .await
                .map_err(|error| AppError::internal(error.to_string()))?;
            return Ok(StatusOnlyResponse {
                status: "uploaded".to_string(),
            });
        }

        let mut state = self.inner.write().await;
        let stored = state
            .documents
            .get_mut(document_id)
            .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
        if stored.document.org_id != self.current_org_id() {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        }

        let content = String::from_utf8(body).unwrap_or_else(|_| {
            "Binary upload received. Parsed preview is not available.".to_string()
        });
        let parsed_items = build_parsed_preview(&content);
        stored.document.file_size = content.len() as u64;
        stored.document.chunk_count = parsed_items.len();
        stored.document.status = DocumentStatus::Queued;
        stored.document.updated_at = now_rfc3339();
        stored.content = content.clone();
        stored.summary = Some(build_summary(&content));
        stored.parsed_items = parsed_items;
        Ok(StatusOnlyResponse {
            status: "uploaded".to_string(),
        })
    }

    pub async fn complete_document_upload(
        &self,
        document_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        if let Some(pg) = &self.pg {
            let document_uuid =
                parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
            let seed = pg
                .get_document_task_seed(&self.auth, document_uuid)
                .await
                .map_err(map_pg_error)?
                .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
            let _ = pg
                .set_document_status(&self.auth, document_uuid, DocumentStatus::Queued)
                .await
                .map_err(map_pg_error)?;
            let notebook_id = Uuid::parse_str(&seed.notebook_id).ok();
            let metadata = serde_json::json!({
                "document_id": seed.document_id.clone(),
                "filename": seed.filename.clone(),
                "file_size": seed.file_size,
                "mime_type": seed.mime_type.clone(),
                "status": "queued",
            });
            self.enqueue_ingest_task(seed).await?;
            self.record_product_event_if_available(
                analytics::ProductEventName::DocumentUploadCompleted,
                analytics::Surface::Workspace,
                analytics::ResultTag::Success,
                None,
                notebook_id,
                metadata,
            )
            .await;
            return Ok(StatusOnlyResponse {
                status: "queued".to_string(),
            });
        }

        let (notebook_id, file_name, file_size, mime_type) = {
            let mut state = self.inner.write().await;
            let stored = state
                .documents
                .get_mut(document_id)
                .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
            stored.document.status = DocumentStatus::Queued;
            stored.document.updated_at = now_rfc3339();
            (
                stored.document.notebook_id.clone(),
                stored.document.file_name.clone(),
                stored.document.file_size,
                stored.document.mime_type.clone(),
            )
        };
        self.record_product_event_if_available(
            analytics::ProductEventName::DocumentUploadCompleted,
            analytics::Surface::Workspace,
            analytics::ResultTag::Success,
            None,
            Uuid::parse_str(&notebook_id).ok(),
            serde_json::json!({
                "document_id": document_id,
                "filename": file_name,
                "file_size": file_size,
                "mime_type": mime_type,
                "status": "queued",
            }),
        )
        .await;
        Ok(StatusOnlyResponse {
            status: "queued".to_string(),
        })
    }

    pub async fn transition_document_status(
        &self,
        document_id: &str,
        status: DocumentStatus,
    ) -> Result<(), AppError> {
        if let Some(pg) = &self.pg {
            let document_id =
                parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
            let updated = pg
                .set_document_status(&self.auth, document_id, status)
                .await
                .map_err(map_pg_error)?;
            if !updated {
                return Err(AppError::not_found(
                    "document_not_found",
                    "document not found",
                ));
            }
            return Ok(());
        }

        let mut state = self.inner.write().await;
        let stored = state
            .documents
            .get_mut(document_id)
            .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
        if stored.document.org_id != self.current_org_id() {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        }
        stored.document.status = status;
        stored.document.updated_at = now_rfc3339();
        Ok(())
    }

    pub async fn simulate_ingestion(&self, document_id: String) {
        info!(document_id, "starting simulated ingestion");
        let _ = self
            .transition_document_status(&document_id, DocumentStatus::Processing)
            .await;
        sleep(Duration::from_secs(1)).await;
        let _ = self
            .transition_document_status(&document_id, DocumentStatus::Completed)
            .await;
        info!(document_id, "completed simulated ingestion");
    }

    pub async fn update_document(
        &self,
        document_id: &str,
        req: UpdateDocumentRequest,
    ) -> Result<StatusOnlyResponse, AppError> {
        if let Some(pg) = &self.pg {
            let document_id =
                parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
            let notebook_id = req
                .notebook_id
                .as_deref()
                .map(|value| {
                    parse_uuid_or_app_error(value, "notebook_not_found", "notebook not found")
                })
                .transpose()?;
            let updated = pg
                .update_document(
                    &self.auth,
                    document_id,
                    req.filename.as_deref(),
                    notebook_id,
                    req.status.clone(),
                )
                .await
                .map_err(map_pg_error)?;
            if !updated {
                return Err(AppError::not_found(
                    "document_not_found",
                    "document not found",
                ));
            }
            return Ok(StatusOnlyResponse {
                status: "updated".to_string(),
            });
        }

        let mut state = self.inner.write().await;
        let stored = state
            .documents
            .get_mut(document_id)
            .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
        if stored.document.org_id != self.current_org_id() {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        }

        if let Some(filename) = req.filename {
            stored.document.file_name = filename;
        }
        if let Some(notebook_id) = req.notebook_id {
            stored.document.notebook_id = notebook_id;
        }
        if let Some(status) = req.status {
            stored.document.status = status;
        }
        stored.document.updated_at = now_rfc3339();
        Ok(StatusOnlyResponse {
            status: "updated".to_string(),
        })
    }

    pub async fn delete_document(&self, document_id: &str) -> Result<StatusOnlyResponse, AppError> {
        if let Some(pg) = &self.pg {
            let document_id =
                parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
            let deleted = pg
                .delete_document(&self.auth, document_id)
                .await
                .map_err(map_pg_error)?;
            if !deleted {
                return Err(AppError::not_found(
                    "document_not_found",
                    "document not found",
                ));
            }
            return Ok(StatusOnlyResponse {
                status: "deleted".to_string(),
            });
        }

        let mut state = self.inner.write().await;
        let can_delete = state
            .documents
            .get(document_id)
            .map(|stored| stored.document.org_id == self.current_org_id())
            .unwrap_or(false);
        if !can_delete {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        }
        state.documents.remove(document_id);
        Ok(StatusOnlyResponse {
            status: "deleted".to_string(),
        })
    }

    pub async fn reindex_document(
        &self,
        document_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        if let Some(pg) = &self.pg {
            let document_id =
                parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
            let seed = pg
                .get_document_task_seed(&self.auth, document_id)
                .await
                .map_err(map_pg_error)?
                .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
            pg.set_document_status(&self.auth, document_id, DocumentStatus::Queued)
                .await
                .map_err(map_pg_error)?;
            let notebook_id = Uuid::parse_str(&seed.notebook_id).ok();
            let metadata = serde_json::json!({
                "document_id": seed.document_id.clone(),
                "filename": seed.filename.clone(),
                "reason": "manual",
            });
            self.enqueue_reindex_task(seed).await?;
            self.record_product_event_if_available(
                analytics::ProductEventName::DocumentReindexed,
                analytics::Surface::Workspace,
                analytics::ResultTag::Success,
                None,
                notebook_id,
                metadata,
            )
            .await;
            return Ok(StatusOnlyResponse {
                status: "queued".to_string(),
            });
        }

        self.transition_document_status(document_id, DocumentStatus::Queued)
            .await?;
        self.record_product_event_if_available(
            analytics::ProductEventName::DocumentReindexed,
            analytics::Surface::Workspace,
            analytics::ResultTag::Success,
            None,
            None,
            serde_json::json!({
                "document_id": document_id.to_string(),
                "reason": "manual",
            }),
        )
        .await;
        Ok(StatusOnlyResponse {
            status: "queued".to_string(),
        })
    }

    pub async fn get_document_content(
        &self,
        document_id: &str,
    ) -> Result<DocumentContentResponse, AppError> {
        if let Some(pg) = &self.pg {
            let document_id =
                parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
            return pg
                .get_document_content(&self.auth, document_id)
                .await
                .map_err(map_pg_error)?
                .ok_or_else(|| AppError::not_found("document_not_found", "document not found"));
        }

        let state = self.inner.read().await;
        let stored = state
            .documents
            .get(document_id)
            .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
        if stored.document.org_id != self.current_org_id() {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        }
        Ok(DocumentContentResponse {
            content: stored.content.clone(),
            summary: stored.summary.clone(),
        })
    }

    pub async fn get_parsed_preview(
        &self,
        document_id: &str,
        cursor: usize,
        limit: usize,
    ) -> Result<ParsedPreviewResponse, AppError> {
        if let Some(pg) = &self.pg {
            let document_id =
                parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
            return pg
                .get_parsed_preview(&self.auth, document_id, cursor, limit)
                .await
                .map_err(map_pg_error)?
                .ok_or_else(|| AppError::not_found("document_not_found", "document not found"));
        }

        let state = self.inner.read().await;
        let stored = state
            .documents
            .get(document_id)
            .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
        if stored.document.org_id != self.current_org_id() {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        }
        let items = stored
            .parsed_items
            .iter()
            .skip(cursor)
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        let next_cursor = cursor + items.len();
        Ok(ParsedPreviewResponse {
            items,
            has_more: next_cursor < stored.parsed_items.len(),
            next_cursor,
            summary: stored.summary.clone(),
        })
    }

}
