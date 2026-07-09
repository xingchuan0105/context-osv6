use anyhow::{anyhow, Context, Result};
use avrag_llm::{ApiStyle, ChatMessage, LlmClient, ModelProviderConfig};

/// Thin wrapper over [`LlmClient`] for HeavyTail writer stages.
#[derive(Debug, Clone)]
pub struct WriterLlm {
    client: LlmClient,
}

impl WriterLlm {
    /// Build from `AGENT_LLM_*` environment variables (loads repo `.env` when present).
    pub fn from_env() -> Result<Self> {
        load_repo_dotenv();
        let config = agent_llm_config_from_env()?;
        Ok(Self {
            client: LlmClient::new(config).with_feature("heavytail_writer"),
        })
    }

    /// Wrap an existing client (tests / orchestrator).
    pub fn from_client(client: LlmClient) -> Self {
        Self {
            client: client.with_feature("heavytail_writer"),
        }
    }

    /// Tag LLM calls for metering (`ChatUsageRecord.feature` / `.stage`).
    pub fn with_phase(&self, phase: &str) -> Self {
        Self {
            client: self
                .client
                .clone()
                .with_feature(format!("write:{phase}"))
                .with_stage(phase),
        }
    }

    pub async fn prose(&self, system: &str, user: &str, temp: f32) -> Result<(String, u32)> {
        let messages = vec![ChatMessage::system(system), ChatMessage::user(user)];
        let response = self
            .client
            .complete(&messages, Some(temp))
            .await
            .context("writer prose completion failed")?;
        Ok((response.content, response.usage.total_tokens))
    }

    /// Tool-calling completion for WriteRefine ReAct rounds.
    pub async fn complete_with_tools(
        &self,
        messages: &[ChatMessage],
        tools: &[contracts::ToolSpec],
        temperature: f32,
    ) -> Result<(avrag_llm::LlmResponse, u32)> {
        let response = self
            .client
            .complete_with_tools(messages, tools, Some(temperature))
            .await
            .context("writer tool completion failed")?;
        let tokens = response.usage.total_tokens;
        Ok((response, tokens))
    }

    /// `complete_json_mode` with one reparse-retry: on parse failure the error is appended
    /// to the user prompt and the model is called once more.
    pub async fn json<T: serde::de::DeserializeOwned>(
        &self,
        system: &str,
        user: &str,
    ) -> Result<(T, u32)> {
        let temperature = agent_llm_temperature();
        let messages = vec![ChatMessage::system(system), ChatMessage::user(user)];
        let response = self
            .client
            .complete_json_mode(&messages, Some(temperature))
            .await
            .context("writer json completion failed")?;
        let mut total_tokens = response.usage.total_tokens;

        match parse_json_response::<T>(&response.content) {
            Ok(value) => Ok((value, total_tokens)),
            Err(first_err) => {
                let repair_user = format!(
                    "{user}\n\nYour previous response was not valid JSON. Parse error: {first_err}\n\
                     Return ONLY valid JSON matching the requested schema."
                );
                let repair_messages = vec![
                    ChatMessage::system(system),
                    ChatMessage::assistant(&response.content),
                    ChatMessage::user(repair_user),
                ];
                let repaired = self
                    .client
                    .complete_json_mode(&repair_messages, Some(temperature))
                    .await
                    .context("writer json repair completion failed")?;
                total_tokens += repaired.usage.total_tokens;
                let value = parse_json_response(&repaired.content)
                    .context("writer json parse failed after repair retry")?;
                Ok((value, total_tokens))
            }
        }
    }
}

fn load_repo_dotenv() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let env_path = repo_root.join(".env");
    if env_path.exists() {
        let _ = dotenvy::from_path(&env_path);
    }
}

fn agent_llm_config_from_env() -> Result<ModelProviderConfig> {
    let base_url = env_required("AGENT_LLM_BASE_URL")?;
    let api_key = env_required("AGENT_LLM_API_KEY")?;
    let model = env_required("AGENT_LLM_MODEL")?;

    Ok(ModelProviderConfig {
        base_url,
        api_key,
        model,
        timeout_ms: env_u64("AGENT_LLM_TIMEOUT_MS", 180_000),
        api_style: env_optional("AGENT_LLM_API_STYLE").and_then(|s| ApiStyle::from_config_str(&s)),
        dimensions: None,
        enable_thinking: env_bool_optional("AGENT_LLM_ENABLE_THINKING"),
        enable_cache: env_bool_optional("AGENT_LLM_ENABLE_CACHE"),
        rpm_limit: env_u32_optional("AGENT_LLM_RPM_LIMIT"),
        tpm_limit: env_u32_optional("AGENT_LLM_TPM_LIMIT"),
    })
}

fn agent_llm_temperature() -> f32 {
    env_f32("AGENT_LLM_TEMPERATURE", 0.2)
}

fn env_required(key: &str) -> Result<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("missing or empty env var {key}"))
}

fn env_optional(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(default)
}

fn env_f32(key: &str, default: f32) -> f32 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(default)
}

fn env_u32_optional(key: &str) -> Option<u32> {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse().ok())
}

fn env_bool_optional(key: &str) -> Option<bool> {
    std::env::var(key).ok().map(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn parse_json_response<T: serde::de::DeserializeOwned>(raw: &str) -> Result<T> {
    let trimmed = raw.trim();
    if let Ok(value) = serde_json::from_str(trimmed) {
        return Ok(value);
    }
    if let Some(json) = extract_json_object(trimmed) {
        return serde_json::from_str(&json).map_err(|e| anyhow!("{e}"));
    }
    serde_json::from_str(trimmed).map_err(|e| anyhow!("{e}"))
}

fn extract_json_object(raw: &str) -> Option<String> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    (start <= end).then(|| raw[start..=end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent_llm_configured() -> bool {
        load_repo_dotenv();
        ["AGENT_LLM_BASE_URL", "AGENT_LLM_API_KEY", "AGENT_LLM_MODEL"]
            .into_iter()
            .all(|key| {
                std::env::var(key)
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false)
            })
    }

    #[test]
    fn parse_json_object_extracts_braced_payload() {
        let raw = "Here is the json:\n{\"ok\":true}\nThanks.";
        let parsed: serde_json::Value =
            parse_json_response(raw).expect("should parse braced JSON object");
        assert_eq!(parsed["ok"], true);
    }

    #[tokio::test]
    #[ignore = "requires live AGENT_LLM API; run with --ignored --nocapture"]
    async fn writer_llm_prose_smoke() {
        if !agent_llm_configured() {
            eprintln!("SKIP: AGENT_LLM_* not configured in .env");
            return;
        }

        let llm = WriterLlm::from_env().expect("from_env");
        let (out, _tokens) = llm
            .prose(
                "You are a terse assistant.",
                "Reply with exactly the word OK and nothing else.",
                0.0,
            )
            .await
            .expect("prose call");

        assert!(
            out.trim().eq_ignore_ascii_case("ok"),
            "unexpected prose response: {out:?}"
        );
    }
}
