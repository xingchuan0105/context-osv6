use serde::Deserialize;
use tokio::time::Duration;

pub(crate) const DEFAULT_MINERU_BASE_URL: &str = "https://mineru.net/api/v4";
pub(crate) const DEFAULT_MINERU_TASK_TIMEOUT_ATTEMPTS: usize = 90;
pub(crate) const DEFAULT_POLL_INTERVAL_SECS: u64 = 2;
/// MinerU v4 `file-urls/batch` rejects requests above this count per minute.
pub(crate) const MINERU_V4_MAX_FILES_PER_UPLOAD_BATCH: usize = 50;
pub(crate) const MINERU_V4_UPLOAD_BATCH_COOLDOWN: Duration = Duration::from_secs(61);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MineruApiMode {
    LegacyV1Upload,
    ExtractV4,
}

#[derive(Debug, Clone)]
pub struct MineruConfig {
    pub base_url: String,
    pub api_key: String,
    pub timeout_ms: u64,
    pub(crate) api_mode: MineruApiMode,
}

impl MineruConfig {
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("MINERU_API_KEY").ok()?;
        if api_key.trim().is_empty() {
            return None;
        }

        let base_url = std::env::var("MINERU_BASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_MINERU_BASE_URL.to_string());

        let api_mode = std::env::var("MINERU_API_MODE")
            .ok()
            .and_then(|value| parse_api_mode(&value))
            .unwrap_or_else(|| infer_api_mode_from_base_url(&base_url));

        let timeout_ms = std::env::var("MINERU_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30000);

        Some(Self {
            base_url,
            api_key,
            timeout_ms,
            api_mode,
        })
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct LegacyUploadResponse {
    pub(crate) task_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LegacyTaskStatus {
    pub(crate) status: String,
    pub(crate) markdown_url: Option<String>,
    pub(crate) images: Option<Vec<super::figure::ImageInfo>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExtractV4Envelope<T> {
    pub(crate) code: i64,
    pub(crate) msg: String,
    pub(crate) data: Option<T>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExtractV4CreateTaskData {
    pub(crate) task_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExtractV4UploadUrlData {
    pub(crate) batch_id: String,
    pub(crate) file_urls: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExtractV4BatchData {
    pub(crate) extract_result: Vec<ExtractV4TaskData>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExtractV4TaskData {
    pub(crate) file_name: Option<String>,
    pub(crate) state: String,
    pub(crate) err_msg: Option<String>,
    pub(crate) full_zip_url: Option<String>,
}

pub(crate) fn infer_api_mode_from_base_url(base_url: &str) -> MineruApiMode {
    if base_url.trim().contains("/api/v4") {
        MineruApiMode::ExtractV4
    } else {
        MineruApiMode::LegacyV1Upload
    }
}

pub(crate) fn parse_api_mode(raw: &str) -> Option<MineruApiMode> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "legacy_v1_upload" | "legacy" | "v1" => Some(MineruApiMode::LegacyV1Upload),
        "extract_v4" | "v4" => Some(MineruApiMode::ExtractV4),
        _ => None,
    }
}
