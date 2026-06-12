use uuid::Uuid;

#[derive(Clone)]
pub struct StoredMultimodalChunk {
    pub chunk_id: Uuid,
    pub asset_id: Uuid,
    pub image_path: String,
    pub fusion_image_paths: Vec<String>,
    pub caption: Option<String>,
    pub context_text: String,
    pub page: Option<i64>,
    pub chunk_type: String,
    pub parser_backend: String,
    pub source_locator: Option<serde_json::Value>,
}

pub fn record_multimodal_degrade(outputs: &mut crate::ParseRunOutputs, reason: String) {
    outputs.multimodal_degrade_count += 1;
    outputs.multimodal_degrade_reasons.push(reason);
}
