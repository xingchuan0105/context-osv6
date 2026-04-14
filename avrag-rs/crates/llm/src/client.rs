use crate::ModelProviderConfig;
use anyhow::Context;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone)]
pub struct LlmClient {
    config: ModelProviderConfig,
    client: reqwest::Client,
}

impl LlmClient {
    pub fn new(config: ModelProviderConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(config.timeout_ms))
            .build()
            .expect("reqwest client should build");
        Self { config, client }
    }

    /// Send a chat completion request
    pub async fn complete(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
    ) -> anyhow::Result<LlmResponse> {
        let started_at = std::time::Instant::now();
        let provider = self.config.provider_name();
        let configured_model = self.config.model.clone();
        if !self.config.is_configured() {
            telemetry::prometheus::observe_llm_call(
                "generic",
                &provider,
                &configured_model,
                "failure",
                started_at.elapsed().as_secs_f64() * 1000.0,
            );
            anyhow::bail!("LLM not configured");
        }

        let mut request_body = serde_json::json!({
            "model": self.config.model,
            "messages": messages
                .iter()
                .map(|m| serde_json::json!({
                    "role": m.role,
                    "content": m.content
                }))
                .collect::<Vec<_>>(),
        });

        if let Some(temp) = temperature {
            request_body["temperature"] = serde_json::json!(temp);
        }
        if let Some(enable_thinking) = self.config.enable_thinking {
            request_body["enable_thinking"] = serde_json::json!(enable_thinking);
        }

        let response = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                telemetry::prometheus::record_dependency_failure(&provider);
                telemetry::prometheus::observe_llm_call(
                    "generic",
                    &provider,
                    &configured_model,
                    "failure",
                    started_at.elapsed().as_secs_f64() * 1000.0,
                );
                return Err(error).context("Failed to send chat completion request");
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            telemetry::prometheus::record_dependency_failure(&provider);
            telemetry::prometheus::observe_llm_call(
                "generic",
                &provider,
                &configured_model,
                "failure",
                started_at.elapsed().as_secs_f64() * 1000.0,
            );
            anyhow::bail!("Chat completion API error {}: {}", status, body);
        }

        #[derive(serde::Deserialize)]
        struct Choice {
            message: ResponseMessage,
        }

        #[derive(serde::Deserialize)]
        struct ResponseMessage {
            content: String,
        }

        #[derive(serde::Deserialize)]
        struct Usage {
            prompt_tokens: u32,
            completion_tokens: u32,
            total_tokens: u32,
        }

        #[derive(serde::Deserialize)]
        struct CompletionResponse {
            choices: Vec<Choice>,
            usage: Usage,
            model: String,
        }

        let resp = response.json().await;
        let resp: CompletionResponse = match resp {
            Ok(resp) => resp,
            Err(error) => {
                telemetry::prometheus::record_dependency_failure(&provider);
                telemetry::prometheus::observe_llm_call(
                    "generic",
                    &provider,
                    &configured_model,
                    "failure",
                    started_at.elapsed().as_secs_f64() * 1000.0,
                );
                return Err(error).context("Failed to parse chat completion response");
            }
        };

        let content = resp
            .choices
            .first()
            .context("No choices in response")?
            .message
            .content
            .clone();
        telemetry::prometheus::observe_llm_call(
            "generic",
            &provider,
            &resp.model,
            "success",
            started_at.elapsed().as_secs_f64() * 1000.0,
        );

        Ok(LlmResponse {
            content,
            usage: LlmUsage {
                prompt_tokens: resp.usage.prompt_tokens,
                completion_tokens: resp.usage.completion_tokens,
                total_tokens: resp.usage.total_tokens,
                provider: self.config.provider_name(),
                model: resp.model.clone(),
            },
            model: resp.model,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
    pub usage: LlmUsage,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
}

impl LlmUsage {
    pub fn zeroed() -> Self {
        Self {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            provider: String::new(),
            model: String::new(),
        }
    }

    pub fn accumulate(&mut self, other: &LlmUsage) {
        self.prompt_tokens += other.prompt_tokens;
        self.completion_tokens += other.completion_tokens;
        self.total_tokens += other.total_tokens;
        if self.provider.is_empty() && !other.provider.is_empty() {
            self.provider = other.provider.clone();
        }
        if self.model.is_empty() && !other.model.is_empty() {
            self.model = other.model.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LlmUsage;

    #[test]
    fn llm_usage_accumulate_preserves_provider_and_model() {
        let mut total = LlmUsage::zeroed();
        total.accumulate(&LlmUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
            provider: "dmxapi".to_string(),
            model: "gemini-test".to_string(),
        });

        assert_eq!(total.total_tokens, 30);
        assert_eq!(total.provider, "dmxapi");
        assert_eq!(total.model, "gemini-test");
    }
}
