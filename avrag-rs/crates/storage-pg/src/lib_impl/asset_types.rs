#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentAssetRow {
    pub asset_id: Uuid,
    pub org_id: Uuid,
    pub notebook_id: Uuid,
    pub document_id: Uuid,
    pub parse_run_id: Option<Uuid>,
    pub page: Option<i32>,
    pub asset_kind: String,
    pub storage_path: Option<String>,
    pub mime_type: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub caption: Option<String>,
    pub parser_backend: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultimodalChunkRow {
    pub chunk_id: Uuid,
    pub org_id: Uuid,
    pub notebook_id: Uuid,
    pub document_id: Uuid,
    pub parse_run_id: Option<Uuid>,
    pub asset_id: Option<Uuid>,
    pub page: Option<i32>,
    pub context_text: Option<String>,
    pub caption: Option<String>,
    pub normalized_text: String,
    pub parser_backend: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct StoreDocumentAssetParams {
    pub asset_id: Uuid,
    pub notebook_id: Uuid,
    pub document_id: Uuid,
    pub parse_run_id: Option<Uuid>,
    pub page: Option<i32>,
    pub asset_kind: String,
    pub storage_path: Option<String>,
    pub mime_type: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub caption: Option<String>,
    pub parser_backend: String,
}

#[derive(Debug, Clone)]
pub struct StoreMultimodalChunkParams {
    pub chunk_id: Uuid,
    pub notebook_id: Uuid,
    pub document_id: Uuid,
    pub parse_run_id: Option<Uuid>,
    pub asset_id: Option<Uuid>,
    pub page: Option<i32>,
    pub context_text: Option<String>,
    pub caption: Option<String>,
    pub normalized_text: String,
    pub parser_backend: String,
    pub metadata: serde_json::Value,
}
