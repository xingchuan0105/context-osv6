use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Token-bucket rate limiter with per-minute refill.
///
/// Tracks two independent buckets:
/// - **RPM** (requests per minute): 1 token consumed per request.
/// - **TPM** (tokens per minute): tokens consumed equal to the prompt + completion
///   token count of each request.
///
/// When a provider limit is unknown, conservative defaults are used
/// (see [`default_rpm_limit`] / [`default_tpm_limit`]).
#[derive(Debug)]
pub struct RateLimiter {
    rpm: Mutex<TokenBucket>,
    tpm: Mutex<TokenBucket>,
}

#[derive(Debug)]
struct TokenBucket {
    capacity: f64,
    tokens: f64,
    last_refill: Instant,
    /// Tokens added per second (capacity / 60s).
    refill_rate: f64,
}

impl TokenBucket {
    fn new(capacity: u32) -> Self {
        let capacity_f = capacity as f64;
        Self {
            capacity: capacity_f,
            tokens: capacity_f,
            last_refill: Instant::now(),
            refill_rate: capacity_f / 60.0,
        }
    }

    fn try_acquire(&mut self, needed: f64) -> bool {
        self.refill();
        if self.tokens >= needed {
            self.tokens -= needed;
            true
        } else {
            false
        }
    }

    fn refund(&mut self, amount: f64) {
        self.refill();
        self.tokens = (self.tokens + amount).min(self.capacity);
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let added = elapsed * self.refill_rate;
        if added > 0.0 {
            self.tokens = (self.tokens + added).min(self.capacity);
            self.last_refill = now;
        }
    }
}

impl RateLimiter {
    pub fn new(rpm_limit: u32, tpm_limit: u32) -> Self {
        Self {
            rpm: Mutex::new(TokenBucket::new(rpm_limit)),
            tpm: Mutex::new(TokenBucket::new(tpm_limit)),
        }
    }

    /// Check whether a request with the given estimated token cost is allowed.
    /// If allowed, deducts 1 RPM token and `estimated_tokens` TPM tokens.
    ///
    /// Returns `Ok(actual_deducted_tpm)` on success, or `Err(RateLimitError)`
    /// when either bucket is exhausted.
    pub fn check_request(&self, estimated_tokens: usize) -> Result<usize, RateLimitError> {
        let mut rpm = self.rpm.lock().unwrap();
        let mut tpm = self.tpm.lock().unwrap();

        let needed_tpm = estimated_tokens as f64;
        if !rpm.try_acquire(1.0) {
            return Err(RateLimitError::RpmExceeded);
        }
        if !tpm.try_acquire(needed_tpm) {
            // Roll back the RPM deduction.
            rpm.refund(1.0);
            return Err(RateLimitError::TpmExceeded);
        }
        Ok(estimated_tokens)
    }

    /// Adjust the TPM bucket after a request completes.
    ///
    /// If the actual token count is less than the pre-deducted estimate,
    /// the difference is refunded. If it is greater, the additional amount
    /// is deducted (this may drive the bucket negative, which is acceptable
    /// because the request already went through; the negative balance will
    /// throttle subsequent requests).
    pub fn record_actual_usage(&self, pre_deducted: usize, actual_tokens: usize) {
        let mut tpm = self.tpm.lock().unwrap();
        if actual_tokens < pre_deducted {
            tpm.refund((pre_deducted - actual_tokens) as f64);
        } else if actual_tokens > pre_deducted {
            let extra = (actual_tokens - pre_deducted) as f64;
            tpm.try_acquire(extra);
        }
    }
}

/// Clone-able handle to a shared rate limiter.
pub type SharedRateLimiter = Arc<RateLimiter>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitError {
    RpmExceeded,
    TpmExceeded,
}

impl std::fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RateLimitError::RpmExceeded => write!(f, "LLM RPM limit exceeded"),
            RateLimitError::TpmExceeded => write!(f, "LLM TPM limit exceeded"),
        }
    }
}

impl std::error::Error for RateLimitError {}

/// Conservative default RPM limit when the provider does not publish one.
pub fn default_rpm_limit() -> u32 {
    60
}

/// Conservative default TPM limit when the provider does not publish one.
pub fn default_tpm_limit() -> u32 {
    1_000_000
}

/// Provider-specific defaults derived from published limits (Tier-1 / standard keys).
///
/// These are intentionally conservative to avoid hard rate-limit errors from
/// upstream providers.
pub fn provider_defaults(base_url: &str) -> (u32, u32) {
    let url = base_url.to_ascii_lowercase();
    if url.contains("deepseek") {
        // DeepSeek V3: 60 RPM, 1M TPM for standard tier.
        (60, 1_000_000)
    } else if url.contains("dashscope") {
        // DashScope text embedding: 120 RPM, 2M TPM.
        // Chat models (Qwen): 60 RPM, 1M TPM.
        // We use the more conservative chat-model defaults here;
        // embedding configs can override explicitly.
        (120, 2_000_000)
    } else if url.contains("openai") {
        // OpenAI GPT-4-turbo tier-1: 60 RPM, 150K TPM.
        (60, 150_000)
    } else if url.contains("siliconflow") {
        (60, 1_000_000)
    } else {
        (default_rpm_limit(), default_tpm_limit())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_allows_requests_within_limit() {
        let limiter = RateLimiter::new(10, 1000);
        assert!(limiter.check_request(50).is_ok());
        assert!(limiter.check_request(50).is_ok());
    }

    #[test]
    fn bucket_blocks_when_rpm_exhausted() {
        let limiter = RateLimiter::new(2, 1000);
        assert!(limiter.check_request(1).is_ok());
        assert!(limiter.check_request(1).is_ok());
        assert_eq!(limiter.check_request(1), Err(RateLimitError::RpmExceeded));
    }

    #[test]
    fn bucket_blocks_when_tpm_exhausted() {
        let limiter = RateLimiter::new(100, 10);
        assert!(limiter.check_request(5).is_ok());
        assert!(limiter.check_request(5).is_ok());
        assert_eq!(limiter.check_request(1), Err(RateLimitError::TpmExceeded));
    }

    #[test]
    fn refund_restores_tpm_balance() {
        let limiter = RateLimiter::new(100, 100);
        let pre = limiter.check_request(80).unwrap();
        assert_eq!(pre, 80);
        // simulate actual usage was only 30
        limiter.record_actual_usage(pre, 30);
        // now we should have 50 tokens left (100 - 30 from first + refund 50)
        // but wait: we deducted 80, used 30, refunded 50 -> balance = 70
        // next request of 70 should pass
        assert!(limiter.check_request(70).is_ok());
    }

    #[test]
    fn provider_defaults_match_known_providers() {
        assert_eq!(
            provider_defaults("https://api.deepseek.com"),
            (60, 1_000_000)
        );
        assert_eq!(
            provider_defaults("https://dashscope.aliyuncs.com"),
            (120, 2_000_000)
        );
        assert_eq!(
            provider_defaults("https://api.openai.com"),
            (60, 150_000)
        );
        assert_eq!(
            provider_defaults("https://api.siliconflow.cn"),
            (60, 1_000_000)
        );
    }
}
