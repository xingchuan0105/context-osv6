//! Judge LLM client (design §4.1).
//!
//! The judge is always DeepSeek V4 Flash (or an explicit `JUDGE_LLM_MODEL`
//! override) — never the agent Pro model — with thinking disabled and
//! temperature forced to 0. Credentials resolve silently from the environment:
//! base URL / API key fall back `JUDGE_LLM_*` → `MEMORY_LLM_*` →
//! `AGENT_LLM_*`; the model falls back `JUDGE_LLM_MODEL` → `MEMORY_LLM_MODEL`
//! → the default constant (`AGENT_LLM_MODEL` is intentionally NOT consulted,
//! so a Pro agent model can never leak into the judge seat).

/// Default judge model (design §4.1): DeepSeek V4 Flash.
pub const DEFAULT_JUDGE_MODEL: &str = "deepseek-v4-flash";

/// Default judge timeout when `JUDGE_LLM_TIMEOUT_MS` is unset (design §4.1).
const DEFAULT_TIMEOUT_MS: u64 = 60_000;

/// Judge sampling temperature: forced to 0 (design §4.1).
/// `ModelProviderConfig` carries no temperature field — `avrag_llm::LlmClient`
/// takes temperature per call — so every judge call must go through
/// `JudgeClient::complete`, which pins this value.
pub const JUDGE_TEMPERATURE: f32 = 0.0;

/// Resolved judge configuration (pure data; see `JudgeConfig::resolve`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgeConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_ms: u64,
}

impl JudgeConfig {
    /// Resolve from an env-var lookup. Empty/whitespace values count as unset.
    ///
    /// Factored as a pure function so unit tests inject a map closure instead
    /// of mutating process env.
    pub fn resolve(get: impl Fn(&str) -> Option<String>) -> anyhow::Result<Self> {
        let first = |names: &[&str]| {
            names
                .iter()
                .filter_map(|n| get(n))
                .map(|v| v.trim().to_string())
                .find(|v| !v.is_empty())
        };
        let base_url = first(&[
            "JUDGE_LLM_BASE_URL",
            "MEMORY_LLM_BASE_URL",
            "AGENT_LLM_BASE_URL",
        ])
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no judge base URL: set JUDGE_LLM_BASE_URL \
                 (or MEMORY_LLM_BASE_URL / AGENT_LLM_BASE_URL)"
            )
        })?;
        let api_key = first(&[
            "JUDGE_LLM_API_KEY",
            "MEMORY_LLM_API_KEY",
            "AGENT_LLM_API_KEY",
        ])
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no judge API key: set JUDGE_LLM_API_KEY \
                 (or MEMORY_LLM_API_KEY / AGENT_LLM_API_KEY)"
            )
        })?;
        // Model chain stops at MEMORY_LLM_MODEL: never inherit the agent model.
        let model = first(&["JUDGE_LLM_MODEL", "MEMORY_LLM_MODEL"])
            .unwrap_or_else(|| DEFAULT_JUDGE_MODEL.to_string());
        let timeout_ms = match first(&["JUDGE_LLM_TIMEOUT_MS"]) {
            Some(raw) => raw
                .parse::<u64>()
                .map_err(|_| anyhow::anyhow!("invalid JUDGE_LLM_TIMEOUT_MS: {raw:?}"))?,
            None => DEFAULT_TIMEOUT_MS,
        };
        Ok(Self {
            base_url,
            api_key,
            model,
            timeout_ms,
        })
    }
}

/// LLM client for the v2 judge. Thin wrapper over `avrag_llm::LlmClient`.
pub struct JudgeClient {
    llm: avrag_llm::LlmClient,
    model: String,
}

impl JudgeClient {
    /// Build from a resolved config. Logs the resolved judge model so a
    /// misconfigured Pro judge is visible in the run log (design §11).
    pub fn new(config: JudgeConfig) -> Self {
        eprintln!(
            "[rag_eval_v2] judge model = {} (timeout {} ms)",
            config.model, config.timeout_ms
        );
        let llm = avrag_llm::LlmClient::new(avrag_llm::ModelProviderConfig {
            base_url: config.base_url,
            api_key: config.api_key,
            model: config.model.clone(),
            timeout_ms: config.timeout_ms,
            api_style: None,
            dimensions: None,
            enable_thinking: Some(false),
            enable_cache: Some(false),
            rpm_limit: None,
            tpm_limit: None,
        });
        Self {
            llm,
            model: config.model,
        }
    }

    /// Resolve from process env (`JudgeConfig::resolve` over `std::env::var`).
    pub fn from_env() -> anyhow::Result<Self> {
        let config = JudgeConfig::resolve(|k| std::env::var(k).ok())?;
        Ok(Self::new(config))
    }

    /// The resolved judge model name (for reports / logs).
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Judge completion — temperature is always pinned to `JUDGE_TEMPERATURE`;
    /// callers cannot override it.
    pub async fn complete(
        &self,
        messages: &[avrag_llm::ChatMessage],
    ) -> anyhow::Result<avrag_llm::LlmResponse> {
        self.llm.complete(messages, Some(JUDGE_TEMPERATURE)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| map.get(k).cloned()
    }

    #[test]
    fn judge_wins_over_memory_wins_over_agent() {
        let c = JudgeConfig::resolve(env(&[
            ("JUDGE_LLM_BASE_URL", "http://judge"),
            ("MEMORY_LLM_BASE_URL", "http://memory"),
            ("AGENT_LLM_BASE_URL", "http://agent"),
            ("JUDGE_LLM_API_KEY", "judge-key"),
            ("MEMORY_LLM_API_KEY", "memory-key"),
            ("AGENT_LLM_API_KEY", "agent-key"),
        ]))
        .unwrap();
        assert_eq!(c.base_url, "http://judge");
        assert_eq!(c.api_key, "judge-key");

        let c = JudgeConfig::resolve(env(&[
            ("MEMORY_LLM_BASE_URL", "http://memory"),
            ("AGENT_LLM_BASE_URL", "http://agent"),
            ("MEMORY_LLM_API_KEY", "memory-key"),
            ("AGENT_LLM_API_KEY", "agent-key"),
        ]))
        .unwrap();
        assert_eq!(c.base_url, "http://memory");
        assert_eq!(c.api_key, "memory-key");

        let c = JudgeConfig::resolve(env(&[
            ("AGENT_LLM_BASE_URL", "http://agent"),
            ("AGENT_LLM_API_KEY", "agent-key"),
        ]))
        .unwrap();
        assert_eq!(c.base_url, "http://agent");
        assert_eq!(c.api_key, "agent-key");
    }

    #[test]
    fn model_defaults_to_flash_and_never_inherits_agent_model() {
        // AGENT_LLM_MODEL must NOT leak in even when it is the only model set.
        let c = JudgeConfig::resolve(env(&[
            ("AGENT_LLM_BASE_URL", "http://agent"),
            ("AGENT_LLM_API_KEY", "k"),
            ("AGENT_LLM_MODEL", "deepseek-v4-pro"),
        ]))
        .unwrap();
        assert_eq!(c.model, DEFAULT_JUDGE_MODEL);

        let c = JudgeConfig::resolve(env(&[
            ("AGENT_LLM_BASE_URL", "http://agent"),
            ("AGENT_LLM_API_KEY", "k"),
            ("MEMORY_LLM_MODEL", "mem-flash"),
        ]))
        .unwrap();
        assert_eq!(c.model, "mem-flash");

        let c = JudgeConfig::resolve(env(&[
            ("AGENT_LLM_BASE_URL", "http://agent"),
            ("AGENT_LLM_API_KEY", "k"),
            ("MEMORY_LLM_MODEL", "mem-flash"),
            ("JUDGE_LLM_MODEL", "judge-flash"),
        ]))
        .unwrap();
        assert_eq!(c.model, "judge-flash");
    }

    #[test]
    fn timeout_defaults_and_parses() {
        let c = JudgeConfig::resolve(env(&[
            ("AGENT_LLM_BASE_URL", "http://agent"),
            ("AGENT_LLM_API_KEY", "k"),
        ]))
        .unwrap();
        assert_eq!(c.timeout_ms, 60_000);

        let c = JudgeConfig::resolve(env(&[
            ("AGENT_LLM_BASE_URL", "http://agent"),
            ("AGENT_LLM_API_KEY", "k"),
            ("JUDGE_LLM_TIMEOUT_MS", "120000"),
        ]))
        .unwrap();
        assert_eq!(c.timeout_ms, 120_000);

        let err = JudgeConfig::resolve(env(&[
            ("AGENT_LLM_BASE_URL", "http://agent"),
            ("AGENT_LLM_API_KEY", "k"),
            ("JUDGE_LLM_TIMEOUT_MS", "soon"),
        ]))
        .unwrap_err();
        assert!(err.to_string().contains("JUDGE_LLM_TIMEOUT_MS"));
    }

    #[test]
    fn missing_credentials_error() {
        let err = JudgeConfig::resolve(env(&[])).unwrap_err();
        assert!(err.to_string().contains("JUDGE_LLM_BASE_URL"));

        let err = JudgeConfig::resolve(env(&[("AGENT_LLM_BASE_URL", "http://agent")])).unwrap_err();
        assert!(err.to_string().contains("JUDGE_LLM_API_KEY"));
    }
}
