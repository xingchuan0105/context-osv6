use common::AppError;
use contracts::chat::{ChatRequest, MAX_TURN_ATTACHMENTS, MAX_TURN_CONTEXT_BYTES};

pub(crate) fn validate_turn_attachments(req: &ChatRequest) -> Result<(), AppError> {
    if req.attachments.is_empty() {
        return Ok(());
    }
    let rag_enabled = req
        .capabilities
        .as_ref()
        .map(|caps| caps.iter().any(|cap| cap == "rag"))
        .unwrap_or_else(|| req.agent_type.contains("rag"));
    if req.workspace_id.is_some()
        || req.source_type.is_some()
        || rag_enabled
        || super::chat::is_write_agent_type(&req.agent_type)
    {
        return Err(AppError::validation(
            "attachment_scope_invalid",
            "Turn attachments belong to personal chat.",
        ));
    }
    if req.attachments.len() > MAX_TURN_ATTACHMENTS
        || req.attachments.iter().any(|file| {
            file.filename.is_empty() || file.filename.len() > 255 || file.text.trim().is_empty()
        })
    {
        return Err(AppError::validation(
            "attachments_invalid",
            "A turn accepts up to five non-empty attachments.",
        ));
    }
    let bytes = req
        .attachments
        .iter()
        .try_fold(req.query.len(), |sum, file| {
            sum.checked_add(file.text.len())?
                .checked_add(file.filename.len())
        });
    if bytes.is_none_or(|bytes| bytes > MAX_TURN_CONTEXT_BYTES)
        || query_with_attachments(req).len() > MAX_TURN_CONTEXT_BYTES
    {
        return Err(AppError::validation(
            "attachment_context_too_large",
            "The question and extracted attachments exceed 64 KiB. Use smaller files.",
        ));
    }
    Ok(())
}

pub(crate) fn query_with_attachments(req: &ChatRequest) -> String {
    if req.attachments.is_empty() {
        return req.query.clone();
    }
    // Substitute data once: user content cannot become a template placeholder.
    include_str!("../../../prompts/templates/turn-attachments.md")
        .split("{query}")
        .map(|part| {
            part.replace(
                "{attachments_json}",
                &serde_json::to_string(&req.attachments).expect("attachment JSON"),
            )
        })
        .collect::<Vec<_>>()
        .join(&req.query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::chat::TurnAttachment;

    fn request() -> ChatRequest {
        serde_json::from_value(serde_json::json!({"query": "Compare the supplied columns."}))
            .unwrap()
    }

    #[test]
    fn attachment_context_is_request_local_and_does_not_mutate_history() {
        let mut req = request();
        req.attachments.push(TurnAttachment {
            filename: "table.xlsx".into(),
            text: "A | B\n2 | 3".into(),
        });
        assert!(validate_turn_attachments(&req).is_ok());
        assert!(query_with_attachments(&req).contains("2 | 3"));
        assert_eq!(req.query, "Compare the supplied columns.");
        assert!(req.messages.is_empty());
        assert!(!query_with_attachments(&request()).contains("2 | 3"));
    }

    #[test]
    fn over_budget_and_workspace_attachments_are_rejected_without_truncation() {
        let mut req = request();
        req.attachments.push(TurnAttachment {
            filename: "large.txt".into(),
            text: "x".repeat(MAX_TURN_CONTEXT_BYTES),
        });
        assert!(validate_turn_attachments(&req).is_err());
        assert_eq!(req.attachments[0].text.len(), MAX_TURN_CONTEXT_BYTES);
        req.attachments[0].text = "small".into();
        req.workspace_id = Some("workspace".into());
        assert!(validate_turn_attachments(&req).is_err());
    }
}
