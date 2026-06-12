use app_core::parse_uuid_or_app_error;
use app_documents::{infer_mime_type_from_path, is_remote_asset_reference};
use common::{AppError, CitationLookupResponse};

use crate::context::ChatContext;

// `app_core::CitationResolver` is not implemented here: its signature resolves by
// `(session_id, citation_id)` only, while HTTP handlers require `message_id` as well.

impl ChatContext {
    pub async fn lookup_citation(
        &self,
        session_id: &str,
        message_id: i64,
        citation_id: i64,
    ) -> Result<CitationLookupResponse, AppError> {
        if let Some(pg) = self.storage.chat_persistence() {
            let session_uuid =
                parse_uuid_or_app_error(session_id, "session_not_found", "session not found")?;
            let message = pg
                .get_message(&self.auth, session_uuid, message_id)
                .await
                ?
                .ok_or_else(|| AppError::not_found("message_not_found", "message not found"))?;
            let citation = message
                .citations
                .iter()
                .find(|citation| citation.citation_id == citation_id)
                .cloned()
                .ok_or_else(|| AppError::not_found("citation_not_found", "citation not found"))?;
            if let Some(chunk_id) = citation.chunk_id.as_deref() {
                let chunk_uuid =
                    parse_uuid_or_app_error(chunk_id, "chunk_not_found", "chunk not found")?;

                if let Some(multimodal) = pg
                    .get_multimodal_chunk_by_id(&self.auth, chunk_uuid)
                    .await
                    ?
                {
                    let asset = if let Some(asset_id) = multimodal.asset_id {
                        pg.get_document_asset_by_id(&self.auth, asset_id)
                            .await
                            ?
                    } else {
                        None
                    };
                    let caption = multimodal
                        .caption
                        .clone()
                        .or_else(|| asset.as_ref().and_then(|item| item.caption.clone()));
                    let page = multimodal.page.map(|value| value as usize).or_else(|| {
                        asset
                            .as_ref()
                            .and_then(|item| item.page.map(|value| value as usize))
                    });
                    let image_url = if let Some(asset) = asset.as_ref() {
                        self.documents
                            .resolve_citation_asset_url(&self.storage, asset)
                            .await
                    } else {
                        None
                    };

                    return Ok(CitationLookupResponse {
                        doc_name: Some(citation.doc_name),
                        content: multimodal
                            .context_text
                            .clone()
                            .or_else(|| Some(multimodal.normalized_text.clone())),
                        doc_id: Some(citation.doc_id),
                        chunk_id: Some(chunk_uuid.to_string()),
                        page,
                        chunk_type: Some("image_with_context".to_string()),
                        asset_id: multimodal.asset_id.map(|value| value.to_string()),
                        caption,
                        image_url,
                        parser_backend: Some(multimodal.parser_backend.clone()),
                        source_locator: multimodal
                            .metadata
                            .get("source_locator")
                            .cloned()
                            .filter(|value| !value.is_null()),
                    });
                }

                if let Some(chunk) = pg
                    .get_chunk_by_id(&self.auth, chunk_uuid)
                    .await
                    ?
                {
                    return Ok(CitationLookupResponse {
                        doc_name: Some(citation.doc_name),
                        content: Some(chunk.content),
                        doc_id: Some(citation.doc_id),
                        chunk_id: Some(chunk_uuid.to_string()),
                        page: chunk.page.map(|value| value as usize),
                        chunk_type: chunk
                            .metadata
                            .get("block_type")
                            .and_then(serde_json::Value::as_str)
                            .map(ToOwned::to_owned)
                            .or(Some("text".to_string())),
                        asset_id: None,
                        caption: None,
                        image_url: None,
                        parser_backend: chunk
                            .metadata
                            .get("parser_backend")
                            .and_then(serde_json::Value::as_str)
                            .map(ToOwned::to_owned),
                        source_locator: chunk
                            .metadata
                            .get("source_locator")
                            .cloned()
                            .filter(|value| !value.is_null()),
                    });
                }
            }
            let content = self
                .documents
                .get_document_content(&self.auth, &self.storage, &citation.doc_id)
                .await?;
            return Ok(CitationLookupResponse {
                doc_name: Some(citation.doc_name),
                content: Some(content.content),
                doc_id: Some(citation.doc_id),
                chunk_id: citation.chunk_id,
                page: citation.page,
                chunk_type: citation.chunk_type,
                asset_id: citation.asset_id,
                caption: citation.caption,
                image_url: citation.image_url,
                parser_backend: citation.parser_backend,
                source_locator: citation.source_locator,
            });
        }

        let state = self.storage.inner().read().await;
        let messages = state
            .messages
            .get(session_id)
            .ok_or_else(|| AppError::not_found("session_not_found", "session not found"))?;
        let session = state
            .sessions
            .get(session_id)
            .ok_or_else(|| AppError::not_found("session_not_found", "session not found"))?;
        if !self.memory_session_visible(&state, session) {
            return Err(AppError::not_found(
                "session_not_found",
                "session not found",
            ));
        }
        let message = messages
            .iter()
            .find(|message| message.id == message_id)
            .ok_or_else(|| AppError::not_found("message_not_found", "message not found"))?;
        let citation = message
            .citations
            .iter()
            .find(|citation| citation.citation_id == citation_id)
            .ok_or_else(|| AppError::not_found("citation_not_found", "citation not found"))?;
        let doc = state
            .documents
            .get(&citation.doc_id)
            .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
        Ok(CitationLookupResponse {
            doc_name: Some(citation.doc_name.clone()),
            content: Some(doc.content.clone()),
            doc_id: Some(citation.doc_id.clone()),
            chunk_id: citation.chunk_id.clone(),
            page: citation.page,
            chunk_type: citation.chunk_type.clone(),
            asset_id: citation.asset_id.clone(),
            caption: citation.caption.clone(),
            image_url: citation.image_url.clone(),
            parser_backend: citation.parser_backend.clone(),
            source_locator: citation.source_locator.clone(),
        })
    }

    pub async fn get_citation_asset(&self, asset_id: &str) -> Result<(Vec<u8>, String), AppError> {
        let pg_opt = self.storage.chat_persistence();
        let pg = pg_opt
            .as_ref()
            .ok_or_else(|| AppError::internal("postgres backend is not configured"))?;
        let asset_uuid = parse_uuid_or_app_error(asset_id, "asset_not_found", "asset not found")?;
        let asset = pg
            .get_document_asset_by_id(&self.auth, asset_uuid)
            .await
            ?
            .ok_or_else(|| AppError::not_found("asset_not_found", "asset not found"))?;
        let storage_path = asset
            .storage_path
            .clone()
            .ok_or_else(|| AppError::not_found("asset_not_found", "asset not found"))?;
        if is_remote_asset_reference(&storage_path) {
            return Err(AppError::validation(
                "asset_is_remote",
                "remote asset should be fetched via image_url",
            ));
        }

        let bytes = self
            .storage
            .object_store()
            .get(&storage_path)
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        let mime_type = asset
            .mime_type
            .clone()
            .or_else(|| infer_mime_type_from_path(&storage_path))
            .unwrap_or_else(|| "application/octet-stream".to_string());

        Ok((bytes, mime_type))
    }
}
