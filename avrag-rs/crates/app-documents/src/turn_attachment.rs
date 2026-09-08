use common::AppError;
use contracts::chat::{MAX_TURN_ATTACHMENT_BYTES, MAX_TURN_CONTEXT_BYTES, TurnAttachment};
use ingestion::parser::{
    AnydocConfig, LiteparsePdfConfig, LocalParseKind, MarkitdownConfig, PaddleOcrClient,
    PaddleOcrConfig, ParsePlan, ParseRouter, is_scanned_markdown, run_anydoc, run_liteparse_pdf,
    run_markitdown,
};

use crate::DocumentContext;

impl DocumentContext {
    /// Parse-only path: no document record, object-store copy, task queue or index.
    pub async fn parse_turn_attachment(
        &self,
        auth: &contracts::auth_runtime::AuthContext,
        billing: &app_billing::BillingContext,
        filename: &str,
        mime_type: &str,
        bytes: &[u8],
    ) -> Result<TurnAttachment, AppError> {
        if filename.is_empty()
            || filename.len() > 255
            || filename.contains(['/', '\\'])
            || filename.chars().any(char::is_control)
        {
            return Err(AppError::validation(
                "attachment_filename_invalid",
                "Invalid attachment filename.",
            ));
        }
        if bytes.is_empty() || bytes.len() > MAX_TURN_ATTACHMENT_BYTES {
            return Err(AppError::validation(
                "attachment_size_invalid",
                "Attachments must be non-empty and at most 20 MiB.",
            ));
        }
        let route = ParseRouter::route(bytes, filename, mime_type)
            .map_err(|error| AppError::validation(error.code(), error.to_string()))?;
        // Reuse the parser configuration, including subprocess timeouts and cleanup.
        // OCR is a platform service; local parsing does not consume storage/index quota.
        let text = match route.plan {
            ParsePlan::Local(plan) => match plan.kind {
                LocalParseKind::Anydoc => {
                    run_anydoc(bytes, filename, &AnydocConfig::from_env()).await
                }
                LocalParseKind::Markitdown => {
                    run_markitdown(bytes, filename, &MarkitdownConfig::from_env()).await
                }
                LocalParseKind::LiteparseV2Pdf => {
                    let parsed =
                        run_liteparse_pdf(bytes, filename, &LiteparsePdfConfig::from_env()).await;
                    match parsed {
                        Ok(markdown) if is_scanned_markdown(&markdown) => {
                            billing.ensure_payer_has_wallet_balance(auth).await?;
                            let config = PaddleOcrConfig::from_env().map_err(parse_error)?;
                            PaddleOcrClient::new(config)
                                .ocr_pdf_bytes(bytes, 1)
                                .await
                                .map(|pages| {
                                    pages
                                        .into_iter()
                                        .map(|page| page.text)
                                        .collect::<Vec<_>>()
                                        .join("\n\n")
                                })
                                .map_err(anyhow::Error::from)
                        }
                        result => result,
                    }
                }
            },
            ParsePlan::External(_) => {
                billing.ensure_payer_has_wallet_balance(auth).await?;
                let config = PaddleOcrConfig::from_env().map_err(parse_error)?;
                PaddleOcrClient::new(config)
                    .ocr_image_bytes(bytes, filename)
                    .await
                    .map(|page| page.text)
                    .map_err(anyhow::Error::from)
            }
        }
        .map_err(parse_error)?;
        if text.trim().is_empty() {
            return Err(AppError::validation(
                "attachment_empty",
                "No readable text was extracted from the attachment.",
            ));
        }
        if text.len() > MAX_TURN_CONTEXT_BYTES {
            return Err(AppError::validation(
                "attachment_context_too_large",
                "Extracted attachment exceeds the 64 KiB turn-context limit; use a smaller file.",
            ));
        }
        Ok(TurnAttachment {
            filename: filename.to_string(),
            text,
        })
    }
}

fn parse_error(error: anyhow::Error) -> AppError {
    tracing::warn!(error = %error, "turn attachment parsing failed");
    AppError::validation(
        "attachment_parse_failed",
        "Attachment parsing failed. The file has not been indexed; it can be retried.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::auth_runtime::{AuthContext, SubjectKind, UserId};

    #[tokio::test]
    async fn rejects_invalid_files_before_using_any_service() {
        let auth = AuthContext::new(UserId::new(uuid::Uuid::new_v4()), SubjectKind::User);
        let billing = app_billing::BillingContext::new(None, "observe".into());
        let documents = DocumentContext::new();
        for (name, bytes) in [
            ("../data.xlsx", b"x".as_slice()),
            ("empty.txt", b"".as_slice()),
            ("archive.zip", b"x".as_slice()),
        ] {
            assert!(
                documents
                    .parse_turn_attachment(&auth, &billing, name, "", bytes)
                    .await
                    .is_err()
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires the configured anydoc executable"]
    async fn parses_real_office_files_without_storage_or_index_services() {
        let auth = AuthContext::new(UserId::new(uuid::Uuid::new_v4()), SubjectKind::User);
        let billing = app_billing::BillingContext::new(None, "observe".into());
        let fixtures: [(&str, &[u8]); 3] = [
            (
                "smoke.xlsx",
                include_bytes!("../../ingestion/tests/fixtures/smoke.xlsx"),
            ),
            (
                "mini.pptx",
                include_bytes!("../../app/tests/product_e2e/fixtures/phase0-mini.pptx"),
            ),
            (
                "mini.docx",
                include_bytes!("../../app/tests/product_e2e/fixtures/phase0-mini.docx"),
            ),
        ];
        for (filename, bytes) in fixtures {
            let attachment = DocumentContext::new()
                .parse_turn_attachment(&auth, &billing, filename, "", bytes)
                .await
                .unwrap_or_else(|error| panic!("{filename}: {error}"));
            assert_eq!(attachment.filename, filename);
            assert!(!attachment.text.trim().is_empty());
            if filename == "mini.pptx" {
                assert!(attachment.text.contains("Phase0 mini pptx ingest probe"));
            }
        }
    }
}
