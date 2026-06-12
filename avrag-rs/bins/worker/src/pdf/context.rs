use std::sync::Arc;

use avrag_llm::LlmClient;
use ingestion::parser::PdfRendererServiceClient;

pub struct PdfParseContext {
    pub pdf_renderer_client: Option<PdfRendererServiceClient>,
    pub ingestion_llm: Option<Arc<LlmClient>>,
}

impl PdfParseContext {
    pub fn new(
        pdf_renderer_client: Option<PdfRendererServiceClient>,
        ingestion_llm: Option<Arc<LlmClient>>,
    ) -> Self {
        Self {
            pdf_renderer_client,
            ingestion_llm,
        }
    }
}
