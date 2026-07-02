use app_core::domain_rows::{
    ConversationHistoryHit, DocumentAssetRow, DocumentDeletionOutcome, DocumentScopeState,
    DocumentTaskSeed, DocumentUploadMutationOutcome, DocumentUploadQueueOutcome, IndexedChunk,
    MultimodalChunkRow, NotificationCreateParams, UserProfileRow,
};

pub fn document_task_seed(value: avrag_storage_pg::DocumentTaskSeed) -> DocumentTaskSeed {
    DocumentTaskSeed {
        document_id: value.document_id,
        org_id: value.org_id,
        notebook_id: value.notebook_id,
        filename: value.filename,
        mime_type: value.mime_type,
        file_size: value.file_size,
        object_path: value.object_path,
        status: value.status,
    }
}

pub fn document_upload_mutation_outcome(
    value: avrag_storage_pg::DocumentUploadMutationOutcome,
) -> DocumentUploadMutationOutcome {
    match value {
        avrag_storage_pg::DocumentUploadMutationOutcome::Updated => {
            DocumentUploadMutationOutcome::Updated
        }
        avrag_storage_pg::DocumentUploadMutationOutcome::NotFound => {
            DocumentUploadMutationOutcome::NotFound
        }
        avrag_storage_pg::DocumentUploadMutationOutcome::StatusConflict(status) => {
            DocumentUploadMutationOutcome::StatusConflict(status)
        }
    }
}

pub fn document_upload_queue_outcome(
    value: avrag_storage_pg::DocumentUploadQueueOutcome,
) -> DocumentUploadQueueOutcome {
    match value {
        avrag_storage_pg::DocumentUploadQueueOutcome::Queued { task_inserted } => {
            DocumentUploadQueueOutcome::Queued { task_inserted }
        }
        avrag_storage_pg::DocumentUploadQueueOutcome::NotFound => {
            DocumentUploadQueueOutcome::NotFound
        }
        avrag_storage_pg::DocumentUploadQueueOutcome::StatusConflict(status) => {
            DocumentUploadQueueOutcome::StatusConflict(status)
        }
    }
}

pub fn document_deletion_outcome(
    value: avrag_storage_pg::DocumentDeletionOutcome,
) -> DocumentDeletionOutcome {
    match value {
        avrag_storage_pg::DocumentDeletionOutcome::Queued { task_inserted } => {
            DocumentDeletionOutcome::Queued { task_inserted }
        }
        avrag_storage_pg::DocumentDeletionOutcome::AlreadyDeleting { task_inserted } => {
            DocumentDeletionOutcome::AlreadyDeleting { task_inserted }
        }
        avrag_storage_pg::DocumentDeletionOutcome::AlreadyDeleted => {
            DocumentDeletionOutcome::AlreadyDeleted
        }
        avrag_storage_pg::DocumentDeletionOutcome::NotFound => DocumentDeletionOutcome::NotFound,
    }
}

pub fn document_scope_state(value: avrag_storage_pg::DocumentScopeState) -> DocumentScopeState {
    DocumentScopeState {
        document_id: value.document_id,
        status: value.status,
    }
}

pub fn user_profile_row_to_pg(value: &UserProfileRow) -> avrag_storage_pg::UserProfileRow {
    avrag_storage_pg::UserProfileRow {
        user_id: value.user_id,
        org_id: value.org_id,
        expertise_domains: value.expertise_domains.clone(),
        preferred_answer_style: value.preferred_answer_style.clone(),
        frequently_asked_topics: value.frequently_asked_topics.clone(),
        custom_preferences: value.custom_preferences.clone(),
        structured_profile: value.structured_profile.clone(),
        inferred_at: value.inferred_at,
        inference_version: value.inference_version.clone(),
    }
}

pub fn user_profile_row(value: avrag_storage_pg::UserProfileRow) -> UserProfileRow {
    UserProfileRow {
        user_id: value.user_id,
        org_id: value.org_id,
        expertise_domains: value.expertise_domains,
        preferred_answer_style: value.preferred_answer_style,
        frequently_asked_topics: value.frequently_asked_topics,
        custom_preferences: value.custom_preferences,
        structured_profile: value.structured_profile,
        inferred_at: value.inferred_at,
        inference_version: value.inference_version,
    }
}

pub fn conversation_history_hit(
    value: avrag_storage_pg::ConversationHistoryHit,
) -> ConversationHistoryHit {
    ConversationHistoryHit {
        message_id: value.message_id,
        session_id: value.session_id,
        role: value.role,
        content: value.content,
        created_at: value.created_at,
    }
}

pub fn notification_create_params(
    value: NotificationCreateParams,
) -> avrag_storage_pg::NotificationCreateParams {
    avrag_storage_pg::NotificationCreateParams {
        user_id: value.user_id,
        event_type: value.event_type,
        title: value.title,
        body: value.body,
        data: value.data,
        channels: value.channels,
    }
}

pub fn document_asset_row(value: avrag_storage_pg::DocumentAssetRow) -> DocumentAssetRow {
    DocumentAssetRow {
        asset_id: value.asset_id,
        org_id: value.org_id,
        notebook_id: value.notebook_id,
        document_id: value.document_id,
        parse_run_id: value.parse_run_id,
        page: value.page,
        asset_kind: value.asset_kind,
        storage_path: value.storage_path,
        mime_type: value.mime_type,
        width: value.width,
        height: value.height,
        caption: value.caption,
        parser_backend: value.parser_backend,
        created_at: value.created_at,
    }
}

pub fn multimodal_chunk_row(value: avrag_storage_pg::MultimodalChunkRow) -> MultimodalChunkRow {
    MultimodalChunkRow {
        chunk_id: value.chunk_id,
        org_id: value.org_id,
        notebook_id: value.notebook_id,
        document_id: value.document_id,
        parse_run_id: value.parse_run_id,
        asset_id: value.asset_id,
        page: value.page,
        context_text: value.context_text,
        caption: value.caption,
        normalized_text: value.normalized_text,
        parser_backend: value.parser_backend,
        metadata: value.metadata,
        created_at: value.created_at,
    }
}

pub fn indexed_chunk(value: avrag_storage_pg::IndexedChunk) -> IndexedChunk {
    IndexedChunk {
        chunk_id: value.chunk_id,
        doc_id: value.doc_id,
        page: value.page,
        content: value.content,
        score: value.score,
        metadata: value.metadata,
    }
}
