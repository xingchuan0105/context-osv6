//! Cloud BYOK provider secrets domain (ADR-0010 PR7 §3.2).
//!
//! Secrets are stored **encrypted** at rest. API / list views expose only
//! fingerprint (last 4 + length) — never plaintext. Resolve is for outbound
//! LLM/embed/rerank calls only.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Secret purpose bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSecretPurpose {
    Llm,
    Embedding,
    Rerank,
}

impl ProviderSecretPurpose {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Llm => "llm",
            Self::Embedding => "embedding",
            Self::Rerank => "rerank",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, &'static str> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "llm" => Ok(Self::Llm),
            "embedding" | "embed" => Ok(Self::Embedding),
            "rerank" => Ok(Self::Rerank),
            _ => Err("purpose must be llm | embedding | rerank"),
        }
    }
}

impl std::fmt::Display for ProviderSecretPurpose {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Safe public view of a stored secret (no ciphertext / plaintext).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderSecretView {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub purpose: String,
    pub provider: String,
    pub base_url: Option<String>,
    pub model_hint: Option<String>,
    /// Display fingerprint: `{last4}:{length}` (e.g. `xYz1:51`).
    pub key_fingerprint: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Upsert request: plaintext key only crosses this boundary into the store adapter.
#[derive(Clone)]
pub struct UpsertProviderSecretInput {
    pub owner_user_id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub purpose: ProviderSecretPurpose,
    pub provider: String,
    pub base_url: Option<String>,
    pub model_hint: Option<String>,
    /// Plaintext API key. Adapters encrypt before persistence. Never log.
    pub api_key: String,
}

impl std::fmt::Debug for UpsertProviderSecretInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpsertProviderSecretInput")
            .field("owner_user_id", &self.owner_user_id)
            .field("workspace_id", &self.workspace_id)
            .field("purpose", &self.purpose)
            .field("provider", &self.provider)
            .field("base_url", &self.base_url)
            .field("model_hint", &self.model_hint)
            .field("api_key", &"[redacted]")
            .finish()
    }
}

/// Resolved secret for outbound provider calls (request path only).
#[derive(Clone)]
pub struct ResolvedProviderSecret {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub purpose: ProviderSecretPurpose,
    pub provider: String,
    pub base_url: Option<String>,
    pub model_hint: Option<String>,
    /// Plaintext API key for the outbound HTTP client. Never log / serialize to API.
    pub api_key: String,
}

impl std::fmt::Debug for ResolvedProviderSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedProviderSecret")
            .field("id", &self.id)
            .field("owner_user_id", &self.owner_user_id)
            .field("workspace_id", &self.workspace_id)
            .field("purpose", &self.purpose)
            .field("provider", &self.provider)
            .field("base_url", &self.base_url)
            .field("model_hint", &self.model_hint)
            .field("api_key", &"[redacted]")
            .finish()
    }
}

impl ResolvedProviderSecret {
    /// Build a single-route `avrag_llm::ModelProviderConfig` for outbound use
    /// (ADR-0010 G1/G4). BYOK secrets are OpenAI-compatible single-route
    /// endpoints — no multi-provider pool, no native-dialect routing, no
    /// request-side `dimensions` (SiliconFlow bge-m3 rejects the field).
    /// Returns `None` when `base_url` / `model_hint` / `api_key` are incomplete,
    /// so callers keep the platform-config path unchanged.
    pub fn to_llm_config(&self) -> Option<avrag_llm::ModelProviderConfig> {
        const DEFAULT_TIMEOUT_MS: u64 = 120_000;
        let base_url = self.base_url.as_deref()?.trim();
        let model = self.model_hint.as_deref()?.trim();
        if base_url.is_empty() || model.is_empty() || self.api_key.is_empty() {
            return None;
        }
        Some(avrag_llm::ModelProviderConfig {
            base_url: base_url.to_string(),
            api_key: self.api_key.clone(),
            model: model.to_string(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            api_style: Some(avrag_llm::ApiStyle::OpenAi),
            dimensions: None,
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        })
    }
}

/// Build display fingerprint: last 4 chars + total length. Never the full key.
pub fn key_fingerprint(api_key: &str) -> String {
    let chars: Vec<char> = api_key.chars().collect();
    let len = chars.len();
    let last4: String = if len == 0 {
        String::new()
    } else {
        chars[len.saturating_sub(4)..].iter().collect()
    };
    format!("{last4}:{len}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_last4_and_length() {
        assert_eq!(key_fingerprint("sk-abcdefghij"), "ghij:13");
        assert_eq!(key_fingerprint("ab"), "ab:2");
        assert_eq!(key_fingerprint(""), ":0");
    }

    #[test]
    fn purpose_parse() {
        assert_eq!(
            ProviderSecretPurpose::parse("LLM").unwrap(),
            ProviderSecretPurpose::Llm
        );
        assert_eq!(
            ProviderSecretPurpose::parse("embed").unwrap(),
            ProviderSecretPurpose::Embedding
        );
        assert!(ProviderSecretPurpose::parse("other").is_err());
    }

    fn dummy_secret() -> ResolvedProviderSecret {
        ResolvedProviderSecret {
            id: Uuid::nil(),
            owner_user_id: Uuid::nil(),
            workspace_id: None,
            purpose: ProviderSecretPurpose::Embedding,
            provider: "siliconflow".to_string(),
            base_url: Some("https://api.siliconflow.cn/v1".to_string()),
            model_hint: Some("BAAI/bge-m3".to_string()),
            api_key: "sk-e2e".to_string(),
        }
    }

    #[test]
    fn to_llm_config_builds_openai_single_route() {
        let cfg = dummy_secret().to_llm_config().expect("config");
        assert_eq!(cfg.base_url, "https://api.siliconflow.cn/v1");
        assert_eq!(cfg.model, "BAAI/bge-m3");
        assert_eq!(cfg.api_style, Some(avrag_llm::ApiStyle::OpenAi));
        assert_eq!(cfg.dimensions, None);
    }

    #[test]
    fn to_llm_config_requires_complete_secret() {
        let mut no_url = dummy_secret();
        no_url.base_url = None;
        assert!(no_url.to_llm_config().is_none());

        let mut no_model = dummy_secret();
        no_model.model_hint = None;
        assert!(no_model.to_llm_config().is_none());

        let mut no_key = dummy_secret();
        no_key.api_key = String::new();
        assert!(no_key.to_llm_config().is_none());
    }
}
