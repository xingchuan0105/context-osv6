use std::collections::BTreeMap;

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::time::Duration;

use crate::ir::{DocumentIr, ParseWarning};

#[derive(Debug, Clone)]
pub struct OfficeParserServiceConfig {
    pub base_url: String,
    pub api_key: Option<String>,
    pub timeout_ms: u64,
}

impl OfficeParserServiceConfig {
    pub fn from_env() -> Option<Self> {
        let base_url = std::env::var("OFFICE_PARSER_BASE_URL").ok()?;
        if base_url.trim().is_empty() {
            return None;
        }

        let api_key = std::env::var("OFFICE_PARSER_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let timeout_ms = std::env::var("OFFICE_PARSER_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(30000);

        Some(Self {
            base_url,
            api_key,
            timeout_ms,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OfficeParserFormat {
    Doc,
    Docx,
    Xls,
    Xlsx,
    Ppt,
    Pptx,
}

impl OfficeParserFormat {
    fn endpoint_path(self) -> &'static str {
        match self {
            Self::Doc => "doc",
            Self::Docx => "docx",
            Self::Xls => "xls",
            Self::Xlsx => "xlsx",
            Self::Ppt => "ppt",
            Self::Pptx => "pptx",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OfficeParserCapabilities {
    pub formats: Vec<OfficeParserFormat>,
    pub backend_versions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OfficeParserHealthz {
    pub ok: bool,
    pub service: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct OfficeParserParseStats {
    pub duration_ms: u64,
    pub block_count: usize,
    pub asset_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OfficeParserParseResponse {
    pub document_ir: DocumentIr,
    pub warnings: Vec<ParseWarning>,
    pub stats: OfficeParserParseStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OfficeParserErrorBody {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct OfficeParserErrorEnvelope {
    pub error: OfficeParserErrorBody,
}

pub struct OfficeParserServiceClient {
    config: OfficeParserServiceConfig,
    client: Client,
}

impl OfficeParserServiceClient {
    pub fn new(config: OfficeParserServiceConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .expect("Failed to create office parser HTTP client");
        Self { config, client }
    }

    pub async fn healthz(&self) -> Result<OfficeParserHealthz> {
        let response = self
            .request(reqwest::Method::GET, "/v1/healthz")
            .send()
            .await
            .context("Failed to call office parser healthz")?;

        Self::decode_json_response(response)
            .await
            .context("Failed to decode office parser healthz response")
    }

    pub async fn capabilities(&self) -> Result<OfficeParserCapabilities> {
        let response = self
            .request(reqwest::Method::GET, "/v1/capabilities")
            .send()
            .await
            .context("Failed to call office parser capabilities")?;

        Self::decode_json_response(response)
            .await
            .context("Failed to decode office parser capabilities response")
    }

    pub async fn parse_doc(
        &self,
        bytes: &[u8],
        filename: &str,
        document_id: &str,
    ) -> Result<OfficeParserParseResponse> {
        self.parse(OfficeParserFormat::Doc, bytes, filename, document_id)
            .await
    }

    pub async fn parse_docx(
        &self,
        bytes: &[u8],
        filename: &str,
        document_id: &str,
    ) -> Result<OfficeParserParseResponse> {
        self.parse(OfficeParserFormat::Docx, bytes, filename, document_id)
            .await
    }

    pub async fn parse_xls(
        &self,
        bytes: &[u8],
        filename: &str,
        document_id: &str,
    ) -> Result<OfficeParserParseResponse> {
        self.parse(OfficeParserFormat::Xls, bytes, filename, document_id)
            .await
    }

    pub async fn parse_xlsx(
        &self,
        bytes: &[u8],
        filename: &str,
        document_id: &str,
    ) -> Result<OfficeParserParseResponse> {
        self.parse(OfficeParserFormat::Xlsx, bytes, filename, document_id)
            .await
    }

    pub async fn parse_ppt(
        &self,
        bytes: &[u8],
        filename: &str,
        document_id: &str,
    ) -> Result<OfficeParserParseResponse> {
        self.parse(OfficeParserFormat::Ppt, bytes, filename, document_id)
            .await
    }

    pub async fn parse_pptx(
        &self,
        bytes: &[u8],
        filename: &str,
        document_id: &str,
    ) -> Result<OfficeParserParseResponse> {
        self.parse(OfficeParserFormat::Pptx, bytes, filename, document_id)
            .await
    }

    pub async fn parse(
        &self,
        format: OfficeParserFormat,
        bytes: &[u8],
        filename: &str,
        document_id: &str,
    ) -> Result<OfficeParserParseResponse> {
        let path = format!("/v1/parse/{}", format.endpoint_path());
        let form = reqwest::multipart::Form::new()
            .part(
                "file",
                reqwest::multipart::Part::bytes(bytes.to_vec()).file_name(filename.to_string()),
            )
            .text("filename", filename.to_string())
            .text("document_id", document_id.to_string())
            .text("parse_profile", "default".to_string());

        let response = self
            .request(reqwest::Method::POST, &path)
            .multipart(form)
            .send()
            .await
            .with_context(|| format!("Failed to call office parser {}", format.endpoint_path()))?;

        Self::decode_json_response(response)
            .await
            .with_context(|| format!("Failed to decode office parser {}", format.endpoint_path()))
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.config.base_url.trim_end_matches('/'), path);
        let builder = self.client.request(method, url);
        if let Some(api_key) = &self.config.api_key {
            builder.header("Authorization", format!("Bearer {}", api_key))
        } else {
            builder
        }
    }

    async fn decode_json_response<T: for<'de> Deserialize<'de>>(
        response: reqwest::Response,
    ) -> Result<T> {
        if response.status().is_success() {
            return response
                .json()
                .await
                .context("Failed to decode office parser success payload");
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if let Ok(error) = serde_json::from_str::<OfficeParserErrorEnvelope>(&body) {
            anyhow::bail!(
                "office parser error {}: {} (retryable={})",
                error.error.code,
                error.error.message,
                error.error.retryable
            );
        }

        anyhow::bail!("office parser request failed: {} - {}", status, body);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_from_env_reads_base_url_and_timeout() {
        unsafe {
            std::env::set_var("OFFICE_PARSER_BASE_URL", "http://127.0.0.1:9090");
            std::env::set_var("OFFICE_PARSER_API_KEY", "secret");
            std::env::set_var("OFFICE_PARSER_TIMEOUT_MS", "45000");
        }

        let config = OfficeParserServiceConfig::from_env().expect("config should exist");
        assert_eq!(config.base_url, "http://127.0.0.1:9090");
        assert_eq!(config.api_key.as_deref(), Some("secret"));
        assert_eq!(config.timeout_ms, 45000);

        unsafe {
            std::env::remove_var("OFFICE_PARSER_BASE_URL");
            std::env::remove_var("OFFICE_PARSER_API_KEY");
            std::env::remove_var("OFFICE_PARSER_TIMEOUT_MS");
        }
    }

    #[test]
    fn config_from_env_returns_none_without_base_url() {
        unsafe {
            std::env::remove_var("OFFICE_PARSER_BASE_URL");
            std::env::remove_var("OFFICE_PARSER_API_KEY");
            std::env::remove_var("OFFICE_PARSER_TIMEOUT_MS");
        }

        let config = OfficeParserServiceConfig::from_env();
        assert!(config.is_none());
    }

    #[test]
    fn office_parser_format_maps_to_expected_path() {
        assert_eq!(OfficeParserFormat::Doc.endpoint_path(), "doc");
        assert_eq!(OfficeParserFormat::Docx.endpoint_path(), "docx");
        assert_eq!(OfficeParserFormat::Xls.endpoint_path(), "xls");
        assert_eq!(OfficeParserFormat::Xlsx.endpoint_path(), "xlsx");
        assert_eq!(OfficeParserFormat::Ppt.endpoint_path(), "ppt");
        assert_eq!(OfficeParserFormat::Pptx.endpoint_path(), "pptx");
    }

    #[test]
    fn parse_response_matches_contract_shape() {
        let payload = serde_json::json!({
            "document_ir": {
                "document_id": "doc-1",
                "title": "deck",
                "doc_type": "pptx",
                "primary_backend": "poi_pptx",
                "backend_version": null,
                "language": null,
                "metadata": {},
                "pages": [{
                    "page_number": 1,
                    "width": null,
                    "height": null,
                    "backend": "poi_pptx",
                    "text_char_count": 42,
                    "image_count": 1,
                    "metadata": {}
                }],
                "blocks": [{
                    "block_id": "slide-1-text",
                    "page": 1,
                    "block_type": "slide_text",
                    "modality": "text_only",
                    "text": "Agenda",
                    "summary_text": null,
                    "asset_refs": [],
                    "caption": null,
                    "section_path": [],
                    "source_locator": {
                        "page": 1,
                        "bbox": null,
                        "paragraph_index": null,
                        "table_index": null,
                        "sheet_name": null,
                        "row_range": null,
                        "col_range": null,
                        "slide_index": 1,
                        "shape_name": null
                    },
                    "parser_backend": "poi_pptx",
                    "metadata": {}
                }],
                "assets": [{
                    "asset_id": "asset-1",
                    "page": 1,
                    "asset_kind": "slide_render",
                    "storage_path": "temporary://slide-1.png",
                    "mime_type": "image/png",
                    "width": 1280,
                    "height": 720,
                    "parser_backend": "poi_pptx",
                    "metadata": {}
                }],
                "warnings": []
            },
            "warnings": [],
            "stats": {
                "duration_ms": 1320,
                "block_count": 1,
                "asset_count": 1
            }
        });

        let response: OfficeParserParseResponse =
            serde_json::from_value(payload).expect("response payload should deserialize");
        assert_eq!(response.document_ir.title, "deck");
        assert_eq!(response.document_ir.blocks.len(), 1);
        assert_eq!(response.document_ir.assets.len(), 1);
        assert_eq!(response.stats.duration_ms, 1320);
    }
}
