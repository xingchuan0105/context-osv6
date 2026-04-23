#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentParseRunRow {
    pub run_id: Uuid,
    pub org_id: Uuid,
    pub notebook_id: Uuid,
    pub document_id: Uuid,
    pub status: String,
    pub backend_summary: serde_json::Value,
    pub duration_ms: Option<i64>,
    pub warnings_json: serde_json::Value,
    pub error_json: Option<serde_json::Value>,
    pub artifact_path: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct StoredDocumentBlock {
    pub block_id: String,
    pub parse_run_id: Option<Uuid>,
    pub page: Option<i32>,
    pub block_type: String,
    pub modality: String,
    pub text: String,
    pub summary_text: Option<String>,
    pub caption: Option<String>,
    pub asset_refs: serde_json::Value,
    pub section_path: serde_json::Value,
    pub source_locator_json: serde_json::Value,
    pub parser_backend: String,
    pub metadata_json: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct StoreDocumentChunkParams {
    pub parse_run_id: Option<Uuid>,
    pub page: Option<i32>,
    pub content: String,
    pub metadata: serde_json::Value,
}
