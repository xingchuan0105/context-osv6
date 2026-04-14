fn map_document_asset(row: PgRow) -> Result<DocumentAssetRow, PgStorageError> {
    Ok(DocumentAssetRow {
        asset_id: row.get("asset_id"),
        org_id: row.get("org_id"),
        notebook_id: row.get("notebook_id"),
        document_id: row.get("document_id"),
        page: row.get("page"),
        asset_kind: row.get("asset_kind"),
        storage_path: row.get("storage_path"),
        mime_type: row.get("mime_type"),
        width: row.get("width"),
        height: row.get("height"),
        caption: row.get("caption"),
        parser_backend: row.get("parser_backend"),
        created_at: row.get("created_at"),
    })
}

fn map_multimodal_chunk(row: PgRow) -> Result<MultimodalChunkRow, PgStorageError> {
    Ok(MultimodalChunkRow {
        chunk_id: row.get("chunk_id"),
        org_id: row.get("org_id"),
        notebook_id: row.get("notebook_id"),
        document_id: row.get("document_id"),
        asset_id: row.get("asset_id"),
        page: row.get("page"),
        context_text: row.get("context_text"),
        caption: row.get("caption"),
        normalized_text: row.get("normalized_text"),
        parser_backend: row.get("parser_backend"),
        metadata: row.get("metadata"),
        created_at: row.get("created_at"),
    })
}
