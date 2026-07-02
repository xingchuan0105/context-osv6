use app_core::{
    DocumentDeletionOutcome, DocumentUploadMutationOutcome, DocumentUploadQueueOutcome,
};
use common::{AppError, ParsedPreviewItem};
use contracts::documents::DocumentStatus;

pub fn status_label(status: &DocumentStatus) -> &'static str {
    match status {
        DocumentStatus::Pending => "pending",
        DocumentStatus::Enqueueing => "enqueueing",
        DocumentStatus::Queued => "queued",
        DocumentStatus::Processing => "processing",
        DocumentStatus::Completed => "completed",
        DocumentStatus::Failed => "failed",
        DocumentStatus::Deleting => "deleting",
        DocumentStatus::Deleted => "deleted",
        DocumentStatus::UploadInvalid => "upload_invalid",
    }
}

pub fn build_summary(content: &str) -> String {
    let compact = content.split_whitespace().collect::<Vec<_>>().join(" ");
    compact.chars().take(180).collect()
}

pub fn build_parsed_preview(content: &str) -> Vec<ParsedPreviewItem> {
    let mut items = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        items.push(ParsedPreviewItem {
            kind: "paragraph".to_string(),
            text: trimmed.to_string(),
            page: 1,
            cursor: index,
        });
    }
    if items.is_empty() {
        items.push(ParsedPreviewItem {
            kind: "paragraph".to_string(),
            text: "Document uploaded but no previewable text was extracted.".to_string(),
            page: 1,
            cursor: 0,
        });
    }
    items
}

pub fn document_is_deleting_or_deleted(status: &DocumentStatus) -> bool {
    matches!(status, DocumentStatus::Deleting | DocumentStatus::Deleted)
}

pub fn document_upload_status_is_mutable_for_app(status: &DocumentStatus) -> bool {
    matches!(
        status,
        DocumentStatus::Pending | DocumentStatus::UploadInvalid
    )
}

pub fn handle_document_deletion_outcome(
    outcome: DocumentDeletionOutcome,
) -> Result<common::StatusOnlyResponse, AppError> {
    match outcome {
        DocumentDeletionOutcome::Queued { .. }
        | DocumentDeletionOutcome::AlreadyDeleting { .. } => Ok(common::StatusOnlyResponse {
            status: "deleting".to_string(),
        }),
        DocumentDeletionOutcome::AlreadyDeleted => Ok(common::StatusOnlyResponse {
            status: "deleted".to_string(),
        }),
        DocumentDeletionOutcome::NotFound => Err(AppError::not_found(
            "document_not_found",
            "document not found",
        )),
    }
}

pub fn upload_status_conflict_error(status: &DocumentStatus) -> AppError {
    AppError::conflict(
        "upload_not_mutable",
        format!(
            "document upload cannot be modified while status is {}",
            status.as_str()
        ),
    )
}

pub fn handle_upload_invalid_outcome(
    outcome: DocumentUploadMutationOutcome,
) -> Result<(), AppError> {
    match outcome {
        DocumentUploadMutationOutcome::Updated => Ok(()),
        DocumentUploadMutationOutcome::NotFound => Err(AppError::not_found(
            "document_not_found",
            "document not found",
        )),
        DocumentUploadMutationOutcome::StatusConflict(status) => {
            Err(upload_status_conflict_error(&status))
        }
    }
}

pub fn handle_upload_queue_outcome(outcome: DocumentUploadQueueOutcome) -> Result<bool, AppError> {
    match outcome {
        DocumentUploadQueueOutcome::Queued { task_inserted } => Ok(task_inserted),
        DocumentUploadQueueOutcome::NotFound => Err(AppError::not_found(
            "document_not_found",
            "document not found",
        )),
        DocumentUploadQueueOutcome::StatusConflict(status) => {
            Err(upload_status_conflict_error(&status))
        }
    }
}

pub fn build_docscope_metadata(metadata: Vec<common::SummaryMetadata>) -> common::DocScopeMetadata {
    let mut languages = Vec::new();
    let mut domains = Vec::new();
    let mut genres = Vec::new();
    let mut eras = Vec::new();

    for meta in &metadata {
        if !meta.language.is_empty() && meta.language != "unknown" {
            languages.push(meta.language.clone());
        }
        if meta.domain != common::Domain::Unknown {
            domains.push(meta.domain);
        }
        if meta.genre != common::Genre::Unknown {
            genres.push(meta.genre);
        }
        if meta.era != common::Era::Unknown {
            eras.push(meta.era);
        }
    }

    languages.sort();
    languages.dedup();
    domains.sort();
    domains.dedup();
    genres.sort();
    genres.dedup();
    eras.sort();
    eras.dedup();

    common::DocScopeMetadata {
        documents: metadata,
        profile: common::DocScopeProfile {
            languages,
            domains,
            genres,
            eras,
        },
    }
}

pub fn build_redis_url(addr: &str, password: &str, db: i64) -> String {
    if password.is_empty() {
        format!("redis://{addr}/{db}")
    } else {
        format!("redis://:{password}@{addr}/{db}")
    }
}

pub fn is_remote_asset_reference(value: &str) -> bool {
    common::is_remote_url(value)
}

pub fn infer_mime_type_from_path(path: &str) -> Option<String> {
    common::infer_mime_type(path).map(|s| s.to_string())
}

pub fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            other => other,
        })
        .collect()
}
