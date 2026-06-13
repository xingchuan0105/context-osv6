use super::ChatMessage;
use crate::ModelProviderConfig;

#[derive(Debug, Clone)]
pub(crate) struct ClientRateLimit {
    limiter: Option<crate::SharedRateLimiter>,
}

impl ClientRateLimit {
    pub(crate) fn from_config(config: &ModelProviderConfig) -> Self {
        let limiter = if config.is_configured() {
            let rpm = config.effective_rpm_limit();
            let tpm = config.effective_tpm_limit();
            Some(std::sync::Arc::new(crate::RateLimiter::new(rpm, tpm)))
        } else {
            None
        };
        Self { limiter }
    }

    pub(crate) fn estimate_input_tokens(&self, messages: &[ChatMessage]) -> usize {
        crate::count_chat_messages(messages)
    }

    pub(crate) fn check_rate_limit(&self, estimated_tokens: usize) -> anyhow::Result<usize> {
        if let Some(limiter) = &self.limiter {
            match limiter.check_request(estimated_tokens) {
                Ok(deducted) => Ok(deducted),
                Err(crate::RateLimitError::RpmExceeded) => {
                    anyhow::bail!("LLM rate limit exceeded: too many requests per minute")
                }
                Err(crate::RateLimitError::TpmExceeded) => {
                    anyhow::bail!("LLM rate limit exceeded: too many tokens per minute")
                }
            }
        } else {
            Ok(estimated_tokens)
        }
    }

    pub(crate) fn record_usage(&self, pre_deducted: usize, actual_tokens: usize) {
        if let Some(limiter) = &self.limiter {
            limiter.record_actual_usage(pre_deducted, actual_tokens);
        }
    }
}
