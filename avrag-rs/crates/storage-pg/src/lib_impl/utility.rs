fn parse_document_status(value: &str) -> DocumentStatus {
    match value {
        "enqueueing" => DocumentStatus::Enqueueing,
        "queued" => DocumentStatus::Queued,
        "processing" => DocumentStatus::Processing,
        "completed" => DocumentStatus::Completed,
        "failed" => DocumentStatus::Failed,
        _ => DocumentStatus::Pending,
    }
}

fn document_status_str(status: &DocumentStatus) -> &'static str {
    match status {
        DocumentStatus::Pending => "pending",
        DocumentStatus::Enqueueing => "enqueueing",
        DocumentStatus::Queued => "queued",
        DocumentStatus::Processing => "processing",
        DocumentStatus::Completed => "completed",
        DocumentStatus::Failed => "failed",
    }
}

fn agent_name(agent_type: &str) -> &'static str {
    match agent_type {
        "search" => "网络搜索助手",
        "general" => "通用聊天助手",
        _ => "知识库助手",
    }
}

fn agent_icon(agent_type: &str) -> &'static str {
    match agent_type {
        "search" => "🔍",
        "general" => "💬",
        _ => "📚",
    }
}

fn build_object_path(
    context: &AuthContext,
    notebook_id: Uuid,
    document_id: Uuid,
    filename: &str,
) -> String {
    format!(
        "{}/{}/{}/{}",
        context.org_id(),
        notebook_id,
        document_id,
        sanitize_filename(filename)
    )
}

fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            other => other,
        })
        .collect()
}

fn parse_rfc3339(value: &str) -> Result<DateTime<Utc>, PgStorageError> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| PgStorageError::NotFound("invalid timestamp".to_string()))
}

fn json_string_vec(value: serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::Array(items) => items
            .into_iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

fn ingestion_kind_str(kind: &IngestionTaskKind) -> &'static str {
    match kind {
        IngestionTaskKind::IngestDocument => "ingest_document",
        IngestionTaskKind::ReindexDocument => "reindex_document",
    }
}

fn parse_ingestion_kind(value: &str) -> IngestionTaskKind {
    match value {
        "reindex_document" => IngestionTaskKind::ReindexDocument,
        _ => IngestionTaskKind::IngestDocument,
    }
}

fn audit_action_str(action: &ingestion::AuditAction) -> &'static str {
    match action {
        ingestion::AuditAction::TaskEnqueued => "task_enqueued",
        ingestion::AuditAction::TaskStarted => "task_started",
        ingestion::AuditAction::TaskCompleted => "task_completed",
        ingestion::AuditAction::TaskFailed => "task_failed",
        ingestion::AuditAction::StateTransition => "state_transition",
        ingestion::AuditAction::InputGuardBlock => "input_guard_block",
        ingestion::AuditAction::OutputGuardBlock => "output_guard_block",
        ingestion::AuditAction::OutputGuardRedact => "output_guard_redact",
        ingestion::AuditAction::OutputGuardFlag => "output_guard_flag",
    }
}
