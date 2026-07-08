use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, Instant};

use avrag_llm::{ApiStyle, ChatMessage, EmbeddingClient, LlmClient, ModelProviderConfig};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const LLM_CONFIG_FILENAME: &str = "llm-config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalLlmConfig {
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_ms: u64,
    #[serde(default)]
    pub enable_thinking: Option<bool>,
    #[serde(default)]
    pub enable_cache: Option<bool>,
    #[serde(default)]
    pub embedding: Option<LocalEmbeddingConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalEmbeddingConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    #[serde(default)]
    pub dimensions: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticStatus {
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticCheck {
    pub name: String,
    pub status: DiagnosticStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairSuggestion {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<RepairAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RepairAction {
    OpenUrl { url: String },
    UpdateConfig { patch: serde_json::Value },
    RunCommand { command: String },
    ShowGuide { guide_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub overall: DiagnosticStatus,
    pub checks: Vec<DiagnosticCheck>,
    pub suggestions: Vec<RepairSuggestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    pub message: String,
}

impl LocalLlmConfig {
    /// Bridge desktop config to `avrag-llm` provider settings.
    pub fn to_provider(&self) -> ModelProviderConfig {
        ModelProviderConfig {
            base_url: self.base_url.clone(),
            api_key: self.effective_api_key(),
            model: self.model.clone(),
            timeout_ms: self.timeout_ms,
            api_style: Some(self.inferred_api_style()),
            dimensions: self.embedding.as_ref().and_then(|e| e.dimensions),
            enable_thinking: self.enable_thinking,
            enable_cache: self.enable_cache,
            rpm_limit: None,
            tpm_limit: None,
        }
    }

    fn effective_api_key(&self) -> String {
        if !self.api_key.is_empty() {
            return self.api_key.clone();
        }
        if self.provider == "ollama" {
            return "ollama".to_string();
        }
        self.api_key.clone()
    }

    fn inferred_api_style(&self) -> ApiStyle {
        match self.provider.as_str() {
            "anthropic" | "google" | "gemini" | "ollama" | "custom" => ApiStyle::OpenAi,
            _ => ApiStyle::OpenAi,
        }
    }

    fn is_usable(&self) -> bool {
        !self.base_url.trim().is_empty()
            && !self.model.trim().is_empty()
            && (self.provider == "ollama" || !self.api_key.trim().is_empty())
    }
}

impl LocalEmbeddingConfig {
    pub fn to_provider(&self) -> ModelProviderConfig {
        ModelProviderConfig {
            base_url: self.base_url.clone(),
            api_key: if self.api_key.is_empty() {
                "local".to_string()
            } else {
                self.api_key.clone()
            },
            model: self.model.clone(),
            timeout_ms: 15_000,
            api_style: Some(ApiStyle::OpenAi),
            dimensions: self.dimensions,
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        }
    }
}

impl DiagnosticCheck {
    fn ok(name: impl Into<String>, message: impl Into<String>, latency_ms: Option<u64>) -> Self {
        Self {
            name: name.into(),
            status: DiagnosticStatus::Ok,
            latency_ms,
            message: message.into(),
        }
    }

    fn warning(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: DiagnosticStatus::Warning,
            latency_ms: None,
            message: message.into(),
        }
    }

    fn error(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: DiagnosticStatus::Error,
            latency_ms: None,
            message: message.into(),
        }
    }
}

impl RepairSuggestion {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            action: None,
        }
    }

    fn with_action(mut self, action: RepairAction) -> Self {
        self.action = Some(action);
        self
    }
}

fn llm_config_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(LLM_CONFIG_FILENAME)
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))
}

pub fn load_llm_config(app_data_dir: &Path) -> Result<Option<LocalLlmConfig>, String> {
    let path = llm_config_path(app_data_dir);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read LLM config: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("Failed to parse LLM config: {e}"))
}

pub fn save_llm_config(app_data_dir: &Path, config: &LocalLlmConfig) -> Result<(), String> {
    std::fs::create_dir_all(app_data_dir)
        .map_err(|e| format!("Failed to create app data dir: {e}"))?;
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize LLM config: {e}"))?;
    std::fs::write(llm_config_path(app_data_dir), json)
        .map_err(|e| format!("Failed to write LLM config: {e}"))
}

fn api_key_url(provider: &str) -> &'static str {
    match provider {
        "zhipu" => "https://open.bigmodel.cn/console/apikey",
        "anthropic" => "https://console.anthropic.com/settings/keys",
        "openai" => "https://platform.openai.com/api-keys",
        "deepseek" => "https://platform.deepseek.com/api_keys",
        "google" | "gemini" => "https://aistudio.google.com/apikey",
        "siliconflow" => "https://cloud.siliconflow.cn/account/ak",
        "dashscope" => "https://dashscope.console.aliyun.com/apiKey",
        "groq" => "https://console.groq.com/keys",
        "openrouter" => "https://openrouter.ai/keys",
        _ => "",
    }
}

fn apply_auth_headers(headers: &mut HeaderMap, provider: &str, api_key: &str) {
    if api_key.is_empty() {
        return;
    }

    match provider {
        "anthropic" => {
            insert_header(headers, "x-api-key", api_key);
            insert_header(headers, "anthropic-version", "2023-06-01");
        }
        "google" | "gemini" => insert_header(headers, "x-goog-api-key", api_key),
        _ => {
            if let Ok(value) = HeaderValue::from_str(&format!("Bearer {api_key}")) {
                headers.insert(reqwest::header::AUTHORIZATION, value);
            }
        }
    }
}

fn insert_header(headers: &mut HeaderMap, name: &str, value: &str) {
    if let (Ok(header_name), Ok(header_value)) = (
        HeaderName::from_str(name),
        HeaderValue::from_str(value),
    ) {
        headers.insert(header_name, header_value);
    }
}

fn models_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/models") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/models")
    }
}

async fn fetch_available_models(config: &LocalLlmConfig) -> Result<Vec<String>, String> {
    let url = models_url(&config.base_url);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(config.timeout_ms.max(5_000)))
        .build()
        .map_err(|e| e.to_string())?;

    let mut headers = HeaderMap::new();
    apply_auth_headers(&mut headers, &config.provider, &config.effective_api_key());

    let response = client
        .get(url)
        .headers(headers)
        .send()
        .await
        .map_err(|e| format!("Failed to list models: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "List models failed with status {}",
            response.status()
        ));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse models response: {e}"))?;

    let models = body
        .get("data")
        .and_then(|data| data.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("id")
                        .or_else(|| item.get("name"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(models)
}

async fn test_embedding(embedding: &LocalEmbeddingConfig) -> Result<(), String> {
    let client = EmbeddingClient::new(embedding.to_provider());
    client
        .embed(&["test"])
        .await
        .map_err(|e| format!("{e}"))
        .map(|_| ())
}

async fn run_chat_probe(config: &LocalLlmConfig) -> Result<(String, u64), String> {
    let started = Instant::now();
    let llm = LlmClient::new(config.to_provider());
    let response = llm
        .complete_with_max_tokens(&[ChatMessage::user("ping")], None, 1)
        .await
        .map_err(|e| e.to_string())?;
    Ok((response.model, started.elapsed().as_millis() as u64))
}

fn overall_status(checks: &[DiagnosticCheck]) -> DiagnosticStatus {
    if checks
        .iter()
        .any(|check| check.status == DiagnosticStatus::Error)
    {
        DiagnosticStatus::Error
    } else if checks
        .iter()
        .any(|check| check.status == DiagnosticStatus::Warning)
    {
        DiagnosticStatus::Warning
    } else {
        DiagnosticStatus::Ok
    }
}

async fn build_diagnostic_report(config: LocalLlmConfig) -> Result<DiagnosticReport, String> {
    let mut checks = Vec::new();
    let mut suggestions = Vec::new();

    if !config.is_usable() {
        checks.push(DiagnosticCheck::warning(
            "config",
            "LLM 配置不完整，请填写 base_url、model 和 API key（Ollama 除外）。",
        ));
        return Ok(DiagnosticReport {
            overall: overall_status(&checks),
            checks,
            suggestions,
        });
    }

    let url = url::Url::parse(&config.base_url)
        .map_err(|e| format!("Base URL 无效: {e}"))?;
    let host = url
        .host_str()
        .ok_or_else(|| "Base URL 缺少 host".to_string())?
        .to_string();
    let port = url.port_or_known_default().unwrap_or(443);

    // Step 1: DNS
    let dns_started = Instant::now();
    match tokio::net::lookup_host((host.as_str(), port)).await {
        Ok(_) => checks.push(DiagnosticCheck::ok(
            "dns",
            "DNS 解析成功",
            Some(dns_started.elapsed().as_millis() as u64),
        )),
        Err(e) => {
            checks.push(DiagnosticCheck::error("dns", format!("DNS 解析失败: {e}")));
            suggestions.push(RepairSuggestion::new(
                "check_dns",
                "请检查网络连接或 DNS 配置。如使用 VPN，请确保 DNS 正常解析。",
            ));
        }
    }

    // Step 2: TCP
    let tcp_started = Instant::now();
    match tokio::time::timeout(
        Duration::from_secs(5),
        tokio::net::TcpStream::connect((host.as_str(), port)),
    )
    .await
    {
        Ok(Ok(_)) => checks.push(DiagnosticCheck::ok(
            "tcp_connect",
            "TCP 连接成功",
            Some(tcp_started.elapsed().as_millis() as u64),
        )),
        Ok(Err(e)) => {
            checks.push(DiagnosticCheck::error(
                "tcp_connect",
                format!("无法连接: {e}"),
            ));
            if host == "localhost" || host == "127.0.0.1" {
                suggestions.push(
                    RepairSuggestion::new(
                        "start_ollama",
                        "本地服务不可达。请确认 Ollama 已启动。",
                    )
                    .with_action(RepairAction::RunCommand {
                        command: "ollama serve".to_string(),
                    }),
                );
            } else {
                suggestions.push(RepairSuggestion::new(
                    "check_network",
                    "无法连接服务器。请检查网络或代理设置。",
                ));
            }
        }
        Err(_) => {
            checks.push(DiagnosticCheck::error("tcp_connect", "连接超时（5s）"));
            suggestions.push(RepairSuggestion::new(
                "check_network",
                "连接超时。请检查网络或代理设置。",
            ));
        }
    }

    // Step 3 + 4: auth + model availability
    match run_chat_probe(&config).await {
        Ok((model, latency_ms)) => {
            checks.push(DiagnosticCheck::ok(
                "auth",
                "API key 有效",
                Some(latency_ms),
            ));
            checks.push(DiagnosticCheck::ok(
                "model_available",
                format!("模型 {} 可用", model),
                Some(latency_ms),
            ));
        }
        Err(err_str) => {
            let lower = err_str.to_ascii_lowercase();
            if lower.contains("401") || lower.contains("unauthorized") {
                checks.push(DiagnosticCheck::error("auth", "API key 无效或已过期"));
                let key_url = api_key_url(&config.provider);
                if key_url.is_empty() {
                    suggestions.push(RepairSuggestion::new(
                        "renew_api_key",
                        "API key 无效，请重新获取。",
                    ));
                } else {
                    suggestions.push(
                        RepairSuggestion::new("renew_api_key", "API key 无效，请重新获取。")
                            .with_action(RepairAction::OpenUrl {
                                url: key_url.to_string(),
                            }),
                    );
                }
            } else if lower.contains("403") || lower.contains("forbidden") {
                checks.push(DiagnosticCheck::error("auth", "无权限访问此模型"));
                suggestions.push(RepairSuggestion::new(
                    "check_plan",
                    "当前账号无权使用此模型，请检查订阅计划或更换模型。",
                ));
            } else if lower.contains("404") || lower.contains("model_not_found") {
                checks.push(DiagnosticCheck::error(
                    "model_available",
                    format!("模型 \"{}\" 不存在", config.model),
                ));
                if let Ok(models) = fetch_available_models(&config).await {
                    if let Some(first) = models.first() {
                        suggestions.push(
                            RepairSuggestion::new(
                                "switch_model",
                                format!("可用模型: {}", models.join(", ")),
                            )
                            .with_action(RepairAction::UpdateConfig {
                                patch: serde_json::json!({ "model": first }),
                            }),
                        );
                    }
                }
            } else if lower.contains("timeout") {
                checks.push(DiagnosticCheck::warning("auth", "请求超时"));
                suggestions.push(
                    RepairSuggestion::new("increase_timeout", "请求超时，可尝试增加超时时间。")
                        .with_action(RepairAction::UpdateConfig {
                            patch: serde_json::json!({ "timeout_ms": 60_000 }),
                        }),
                );
            } else {
                checks.push(DiagnosticCheck::error(
                    "auth",
                    format!("请求失败: {err_str}"),
                ));
            }
        }
    }

    // Step 5: embedding
    if let Some(embedding) = &config.embedding {
        match test_embedding(embedding).await {
            Ok(_) => checks.push(DiagnosticCheck::ok(
                "embedding",
                "Embedding 连接正常",
                None,
            )),
            Err(e) => {
                checks.push(DiagnosticCheck::warning(
                    "embedding",
                    format!("Embedding 异常: {e}"),
                ));
                suggestions.push(RepairSuggestion::new(
                    "use_local_bm25",
                    "Embedding 不可用，将使用本地 BM25 检索作为回退。",
                ));
            }
        }
    } else {
        checks.push(DiagnosticCheck::warning(
            "embedding",
            "Embedding 未配置，将使用本地 BM25 回退",
        ));
    }

    // Step 6: Coding Plan note (Zhipu)
    if config.provider == "zhipu" {
        checks.push(DiagnosticCheck::warning(
            "coding_plan",
            "智谱 Coding Plan 为按月订阅，无公开配额 API；请在智谱控制台查看用量与订阅状态。",
        ));
    } else {
        checks.push(DiagnosticCheck::ok(
            "coding_plan",
            "非 Coding Plan provider，跳过配额检查",
            None,
        ));
    }

    Ok(DiagnosticReport {
        overall: overall_status(&checks),
        checks,
        suggestions,
    })
}

#[tauri::command]
pub async fn get_llm_config(app: AppHandle) -> Result<Option<LocalLlmConfig>, String> {
    let app_data_dir = app_data_dir(&app)?;
    load_llm_config(&app_data_dir)
}

#[tauri::command]
pub async fn set_llm_config(app: AppHandle, config: LocalLlmConfig) -> Result<(), String> {
    let app_data_dir = app_data_dir(&app)?;
    save_llm_config(&app_data_dir, &config)
}

#[tauri::command]
pub async fn test_llm_connection(config: LocalLlmConfig) -> Result<TestResult, String> {
    if !config.is_usable() {
        return Ok(TestResult {
            ok: false,
            latency_ms: None,
            message: "配置不完整：请填写 base_url、model 和 API key（Ollama 除外）".to_string(),
        });
    }

    match run_chat_probe(&config).await {
        Ok((model, latency_ms)) => Ok(TestResult {
            ok: true,
            latency_ms: Some(latency_ms),
            message: format!("连接成功！模型 {model} 可用"),
        }),
        Err(err) => Ok(TestResult {
            ok: false,
            latency_ms: None,
            message: format!("连接失败: {err}"),
        }),
    }
}

#[tauri::command]
pub async fn diagnose_llm(config: LocalLlmConfig) -> Result<DiagnosticReport, String> {
    build_diagnostic_report(config).await
}

#[tauri::command]
pub async fn list_available_models(config: LocalLlmConfig) -> Result<Vec<String>, String> {
    if config.base_url.trim().is_empty() {
        return Err("base_url 不能为空".to_string());
    }
    fetch_available_models(&config).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_provider_uses_placeholder_key_for_ollama() {
        let config = LocalLlmConfig {
            provider: "ollama".to_string(),
            base_url: "http://localhost:11434/v1".to_string(),
            api_key: String::new(),
            model: "llama3.2".to_string(),
            timeout_ms: 30_000,
            enable_thinking: None,
            enable_cache: None,
            embedding: None,
        };

        let provider = config.to_provider();
        assert_eq!(provider.api_key, "ollama");
        assert!(provider.is_configured());
    }

    #[test]
    fn models_url_appends_models_path() {
        assert_eq!(
            models_url("https://api.openai.com/v1"),
            "https://api.openai.com/v1/models"
        );
    }
}
