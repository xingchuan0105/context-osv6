fn map_notebook(row: PgRow) -> Result<Notebook, PgStorageError> {
    let id: Uuid = row.try_get("id")?;
    let org_id: Uuid = row.try_get("org_id")?;
    let owner_id: Option<Uuid> = row.try_get("owner_id")?;
    let title: String = row.try_get("title")?;
    let description: String = row.try_get("description")?;
    let created_at: DateTime<Utc> = row.try_get("created_at")?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at")?;
    let document_count: i64 = row.try_get("document_count").unwrap_or(0);
    let status_summary_json: serde_json::Value = row
        .try_get("status_summary")
        .unwrap_or_else(|_| serde_json::json!({}));
    let status_summary: std::collections::HashMap<String, i64> =
        serde_json::from_value(status_summary_json).unwrap_or_default();
    let shared: bool = row.try_get("shared").unwrap_or(false);
    Ok(Notebook {
        id: id.to_string(),
        org_id: org_id.to_string(),
        owner_id: owner_id.map(|value| value.to_string()).unwrap_or_default(),
        name: title.clone(),
        title,
        description,
        created_at: created_at.to_rfc3339(),
        updated_at: updated_at.to_rfc3339(),
        document_count,
        status_summary,
        shared,
    })
}

fn map_document(row: PgRow) -> Result<Document, PgStorageError> {
    let id: Uuid = row.try_get("id")?;
    let org_id: Uuid = row.try_get("org_id")?;
    let notebook_id: Uuid = row.try_get("notebook_id")?;
    let file_name: String = row.try_get("file_name")?;
    let mime_type: Option<String> = row.try_get("mime_type")?;
    let file_size: i64 = row.try_get("file_size")?;
    let status: String = row.try_get("status")?;
    let chunk_count: i32 = row.try_get("chunk_count")?;
    let created_at: DateTime<Utc> = row.try_get("created_at")?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at")?;
    Ok(Document {
        id: id.to_string(),
        org_id: org_id.to_string(),
        notebook_id: notebook_id.to_string(),
        owner_id: String::new(),
        file_name,
        mime_type: mime_type.unwrap_or_default(),
        file_size: u64::try_from(file_size).unwrap_or_default(),
        status: parse_document_status(&status),
        chunk_count: usize::try_from(chunk_count).unwrap_or_default(),
        created_at: created_at.to_rfc3339(),
        updated_at: updated_at.to_rfc3339(),
    })
}

fn map_session(row: PgRow) -> Result<ChatSession, PgStorageError> {
    let id: Uuid = row.try_get("id")?;
    let notebook_id: Uuid = row.try_get("notebook_id")?;
    let title: Option<String> = row.try_get("title")?;
    let agent_type: String = row.try_get("agent_type")?;
    let pinned: bool = row.try_get("pinned").unwrap_or(false);
    let created_at: DateTime<Utc> = row.try_get("created_at")?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at")?;
    Ok(ChatSession {
        id: id.to_string(),
        notebook_id: notebook_id.to_string(),
        title,
        agent_type,
        pinned,
        created_at: created_at.to_rfc3339(),
        updated_at: updated_at.to_rfc3339(),
    })
}

fn map_message(row: PgRow) -> Result<ChatMessage, PgStorageError> {
    let citations_value: serde_json::Value = row.try_get("citations")?;
    let citations = serde_json::from_value::<Vec<Citation>>(citations_value)?;
    let created_at: DateTime<Utc> = row.try_get("created_at")?;
    let session_id: Uuid = row.try_get("session_id")?;
    let role: String = row.try_get("role")?;
    let content: String = row.try_get("content")?;
    let answer_blocks_value: serde_json::Value =
        row.try_get("answer_blocks").unwrap_or_else(|_| json!([]));
    let answer_blocks = if role == "assistant" {
        let parsed = serde_json::from_value::<Vec<contracts::chat::AnswerBlock>>(answer_blocks_value)
            .unwrap_or_default();
        if parsed.is_empty() {
            common::answer_blocks_from_rendered_answer(&content, &citations)
        } else {
            parsed
        }
    } else {
        Vec::new()
    };
    let tool_results_value: serde_json::Value =
        row.try_get("tool_results").unwrap_or_else(|_| json!([]));
    let tool_results = serde_json::from_value::<Vec<contracts::ToolResult>>(tool_results_value)
        .unwrap_or_default()
        .into_iter()
        .map(Into::into)
        .collect();
    let turn_metadata_value: serde_json::Value =
        row.try_get("turn_metadata").unwrap_or_else(|_| json!({}));
    let turn_metadata = if turn_metadata_value.is_null()
        || turn_metadata_value.as_object().is_some_and(|m| m.is_empty())
    {
        None
    } else {
        Some(turn_metadata_value)
    };
    let resolved_query: Option<String> = row.try_get("resolved_query").ok().flatten();
    Ok(ChatMessage {
        id: row.try_get("id")?,
        session_id: session_id.to_string(),
        role,
        content,
        answer_blocks,
        agent_id: row.try_get("agent_id").ok(),
        agent_name: row.try_get("agent_name").ok(),
        agent_icon: row.try_get("agent_icon").ok(),
        citations,
        tool_results,
        turn_metadata,
        resolved_query,
        created_at: created_at.to_rfc3339(),
    })
}

fn map_api_key(row: PgRow) -> Result<ApiKeyRow, PgStorageError> {
    let id: Uuid = row.try_get("id")?;
    let org_id: Uuid = row.try_get("org_id")?;
    let notebook_id: Option<Uuid> = row.try_get("notebook_id").ok().flatten();
    let created_by: Option<Uuid> = row.try_get("created_by").ok().flatten();
    let created_at: DateTime<Utc> = row.try_get("created_at")?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at")?;
    let expires_at: Option<DateTime<Utc>> = row.try_get("expires_at").ok().flatten();
    let last_used_at: Option<DateTime<Utc>> = row.try_get("last_used_at").ok().flatten();
    let permissions = row
        .try_get::<Vec<String>, _>("permissions")
        .unwrap_or_else(|_| vec!["query".to_string()]);
    Ok(ApiKeyRow {
        id: id.to_string(),
        org_id: org_id.to_string(),
        notebook_id: notebook_id
            .map(|value| value.to_string())
            .unwrap_or_default(),
        key_prefix: row.try_get("key_prefix")?,
        name: row.try_get("name")?,
        permissions,
        rate_limit_rpm: u32::try_from(row.try_get::<i32, _>("rate_limit_rpm")?).unwrap_or(60),
        expires_at: expires_at.map(|value| value.to_rfc3339()),
        last_used_at: last_used_at.map(|value| value.to_rfc3339()),
        is_active: row.try_get("is_active")?,
        created_by: created_by
            .map(|value| value.to_string())
            .unwrap_or_default(),
        created_at: created_at.to_rfc3339(),
        updated_at: updated_at.to_rfc3339(),
    })
}

fn map_notification(row: PgRow) -> Result<NotificationRow, PgStorageError> {
    let id: Uuid = row.try_get("id")?;
    let org_id: Uuid = row.try_get("org_id")?;
    let user_id: Uuid = row.try_get("user_id")?;
    let data_value: serde_json::Value = row.try_get("data")?;
    let data = match data_value {
        serde_json::Value::Object(map) => map.into_iter().collect(),
        _ => Default::default(),
    };
    let created_at: DateTime<Utc> = row.try_get("created_at")?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at")?;
    let read_at: Option<DateTime<Utc>> = row.try_get("read_at").ok().flatten();
    Ok(NotificationRow {
        id: id.to_string(),
        org_id: org_id.to_string(),
        user_id: user_id.to_string(),
        event_type: row.try_get("event_type")?,
        title: row.try_get("title")?,
        body: row.try_get("body")?,
        data,
        read_at: read_at.map(|value| value.to_rfc3339()),
        created_at: created_at.to_rfc3339(),
        updated_at: updated_at.to_rfc3339(),
    })
}

fn map_user_profile(row: PgRow) -> Result<UserProfileRow, PgStorageError> {
    let user_id: Uuid = row.try_get("user_id")?;
    let org_id: Uuid = row.try_get("org_id")?;
    let expertise_domains = json_string_vec(row.try_get("expertise_domains")?);
    let frequently_asked_topics = json_string_vec(row.try_get("frequently_asked_topics")?);
    let inferred_at: DateTime<Utc> = row.try_get("inferred_at")?;
    Ok(UserProfileRow {
        user_id,
        org_id: OrgId::from(org_id),
        expertise_domains,
        preferred_answer_style: row.try_get("preferred_answer_style").ok(),
        frequently_asked_topics,
        custom_preferences: row.try_get("custom_preferences")?,
        structured_profile: row.try_get("structured_profile").unwrap_or_else(|_| serde_json::json!({})),
        inferred_at,
        inference_version: row.try_get("inference_version")?,
    })
}

fn map_indexed_chunk(row: PgRow) -> Result<IndexedChunk, PgStorageError> {
    let chunk_id: Uuid = row.try_get("id")?;
    let doc_id: Uuid = row.try_get("document_id")?;
    let page = row
        .try_get::<Option<i32>, _>("page")
        .ok()
        .flatten()
        .map(i64::from);
    let content: String = row.try_get("content")?;
    let score = row.try_get::<Option<f32>, _>("rank").ok().flatten();
    let metadata = row.try_get("metadata").unwrap_or_else(|_| json!({}));
    Ok(IndexedChunk {
        chunk_id: chunk_id.to_string(),
        doc_id: doc_id.to_string(),
        page,
        content,
        score,
        metadata,
    })
}

fn map_document_task_seed(row: PgRow) -> Result<DocumentTaskSeed, PgStorageError> {
    let document_id: Uuid = row.try_get("id")?;
    let org_id: Uuid = row.try_get("org_id")?;
    let notebook_id: Uuid = row.try_get("notebook_id")?;
    let filename: String = row.try_get("file_name")?;
    let mime_type: Option<String> = row.try_get("mime_type")?;
    let file_size: i64 = row.try_get("file_size")?;
    let object_path: Option<String> = row.try_get("object_path")?;
    let status: String = row.try_get("status")?;
    Ok(DocumentTaskSeed {
        document_id: document_id.to_string(),
        org_id: org_id.to_string(),
        notebook_id: notebook_id.to_string(),
        filename,
        mime_type: mime_type.unwrap_or_else(|| "application/octet-stream".to_string()),
        file_size: u64::try_from(file_size).unwrap_or_default(),
        object_path: object_path.unwrap_or_default(),
        status: parse_document_status(&status),
    })
}

fn map_document_upload_validation(row: PgRow) -> Result<DocumentUploadValidation, PgStorageError> {
    let upload_size_bytes: Option<i64> = row.try_get("upload_size_bytes")?;
    Ok(DocumentUploadValidation {
        upload_size_bytes: upload_size_bytes.and_then(|value| u64::try_from(value).ok()),
        upload_sha256: row.try_get("upload_sha256")?,
        upload_validated_at: row.try_get("upload_validated_at")?,
        upload_validation_error: row.try_get("upload_validation_error")?,
    })
}

fn map_ingestion_task(row: PgRow) -> Result<IngestionTask, PgStorageError> {
    let task_id: Uuid = row.try_get("task_id")?;
    let org_id: Uuid = row.try_get("org_id")?;
    let notebook_id: Uuid = row.try_get("notebook_id")?;
    let document_id: Uuid = row.try_get("document_id")?;
    let kind: String = row.try_get("kind")?;
    let requested_by: Option<Uuid> = row.try_get("requested_by")?;
    let enqueued_at: DateTime<Utc> = row.try_get("enqueued_at")?;
    let payload: serde_json::Value = row.try_get("payload")?;
    let lock_token: Option<Uuid> = row.try_get("lock_token").ok().flatten();
    Ok(IngestionTask {
        task_id: task_id.to_string(),
        kind: parse_ingestion_kind(&kind),
        org_id: org_id.to_string(),
        notebook_id: notebook_id.to_string(),
        document_id: document_id.to_string(),
        requested_by: requested_by.map(|value| value.to_string()),
        idempotency_key: row.try_get("idempotency_key")?,
        enqueued_at: enqueued_at.to_rfc3339(),
        payload: serde_json::from_value::<IngestionTaskPayload>(payload)?,
        lock_token: lock_token.map(|value| value.to_string()),
        attempt_count: row.try_get("attempt_count").unwrap_or(0),
        max_attempts: row.try_get("max_attempts").unwrap_or(ingestion_types::DEFAULT_MAX_ATTEMPTS),
    })
}

fn map_document_cleanup_task(row: PgRow) -> Result<DocumentCleanupTask, PgStorageError> {
    Ok(DocumentCleanupTask {
        task_id: row.try_get("task_id")?,
        org_id: row.try_get("org_id")?,
        notebook_id: row.try_get("notebook_id")?,
        document_id: row.try_get("document_id")?,
        requested_by: row.try_get("requested_by")?,
        idempotency_key: row.try_get("idempotency_key")?,
        payload: row.try_get("payload")?,
        lock_token: row.try_get("lock_token").ok().flatten(),
        attempt_count: row.try_get("attempt_count").unwrap_or(0),
        max_attempts: row.try_get("max_attempts").unwrap_or(5),
    })
}
