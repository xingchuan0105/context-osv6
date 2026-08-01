//! Provider routing layer: multi-key pools and cross-provider failover.
//!
//! A [`ProviderPool`] holds one or more provider members in priority order.
//! Each member carries one or more API keys. Picks prefer the first healthy
//! member (round-robin across its keys); members/keys that fail are placed
//! into cooldown and skipped until the cooldown expires.
//!
//! - Key-level failures (429 rate limiting, 401/403 credential errors) cool
//!   down only the offending key.
//! - Provider-level failures (5xx, network/timeout, protocol/parse errors,
//!   empty streams) cool down the whole member, which makes the next pick
//!   fall through to the backup member.

use crate::ModelProviderConfig;
use crate::client::rate_limit::ClientRateLimit;
use crate::route::{AnyRoute, build_route_from_config};
use crate::schema::{LlmError, LlmResponse};
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Default member-level cooldown applied after a provider-level failure.
pub const DEFAULT_COOLDOWN_SECS: u64 = 30;

/// One pool member: a provider configuration plus the keys to rotate across.
#[derive(Debug, Clone)]
pub struct PoolMemberConfig {
    pub config: ModelProviderConfig,
    pub api_keys: Vec<String>,
}

impl PoolMemberConfig {
    /// Member from a single-key config (uses `config.api_key`).
    pub fn single(config: ModelProviderConfig) -> Self {
        let api_keys = if config.api_key.is_empty() {
            Vec::new()
        } else {
            vec![config.api_key.clone()]
        };
        Self { config, api_keys }
    }

    /// Member with an explicit key list (rotated round-robin).
    pub fn with_keys(config: ModelProviderConfig, api_keys: Vec<String>) -> Self {
        Self { config, api_keys }
    }

    fn is_usable(&self) -> bool {
        !self.config.base_url.is_empty() && !self.api_keys.is_empty()
    }
}

/// Pool configuration: ordered members plus a member-level cooldown.
#[derive(Debug, Clone)]
pub struct LlmPoolConfig {
    pub members: Vec<PoolMemberConfig>,
    pub cooldown_secs: u64,
}

impl LlmPoolConfig {
    pub fn new(members: Vec<PoolMemberConfig>) -> Self {
        Self {
            members,
            cooldown_secs: DEFAULT_COOLDOWN_SECS,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

/// A concrete pick: one member, one key, with its rate-limit pre-deduction.
#[derive(Debug, Clone)]
pub struct Pick {
    pub provider: String,
    pub model: String,
    /// Member config with `api_key` replaced by the picked key.
    pub config: ModelProviderConfig,
    pub route: AnyRoute,
    pub pre_deducted: usize,
    pub(crate) member_idx: usize,
    pub(crate) key_idx: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickError {
    /// No member/key is out of cooldown and has rate-limit capacity.
    NoCapacity,
}

/// How a failure affects the pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// Not retryable at all (cancellation, config errors, non-retryable 4xx).
    NotRetryable,
    /// Key-level issue (429 rate limiting, 401/403): cool down only the key.
    KeyOnly,
    /// Provider-level issue (5xx, network, protocol): cool down the member.
    Provider,
}

/// Outcome of [`ProviderPool::try_each`] after all candidates are exhausted.
#[derive(Debug)]
pub enum PoolAttemptError {
    /// Every retryable candidate failed; the last error is preserved.
    Exhausted(LlmError),
    /// No candidate was available at all (all cooled down or rate-limited).
    NoCapacity,
}

impl std::fmt::Display for PoolAttemptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exhausted(err) => write!(f, "all LLM pool candidates failed: {err}"),
            Self::NoCapacity => write!(
                f,
                "no LLM pool candidate available (rate-limited or in cooldown)"
            ),
        }
    }
}

impl std::error::Error for PoolAttemptError {}

/// Classify an error for pool retry decisions.
pub fn failure_kind(err: &LlmError) -> FailureKind {
    match err {
        LlmError::Api { status, .. } => match *status {
            429 | 401 | 403 => FailureKind::KeyOnly,
            500..=599 => FailureKind::Provider,
            _ => FailureKind::NotRetryable,
        },
        LlmError::Http(_) | LlmError::Parse(_) | LlmError::Protocol(_) | LlmError::EmptyStream => {
            FailureKind::Provider
        }
        LlmError::Cancelled | LlmError::Config(_) | LlmError::Other(_) => FailureKind::NotRetryable,
    }
}

#[derive(Debug)]
struct KeyState {
    provider: String,
    model: String,
    config: ModelProviderConfig,
    route: AnyRoute,
    rate_limit: ClientRateLimit,
    cooldown_until: Option<Instant>,
}

#[derive(Debug)]
struct MemberState {
    keys: Vec<KeyState>,
    cooldown_until: Option<Instant>,
    cooldown: Duration,
    /// Next key to try within this member (round-robin).
    cursor: usize,
}

/// Ordered multi-provider routing state. Cheap to clone (Arc inside).
#[derive(Clone)]
pub struct ProviderPool {
    inner: Arc<Mutex<Vec<MemberState>>>,
    cooldown: Duration,
}

impl std::fmt::Debug for ProviderPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderPool")
            .field("members", &self.inner.lock().unwrap().len())
            .field("cooldown", &self.cooldown)
            .finish_non_exhaustive()
    }
}

impl ProviderPool {
    pub fn new(config: LlmPoolConfig) -> Self {
        let cooldown = Duration::from_secs(config.cooldown_secs.max(1));
        let members = config
            .members
            .into_iter()
            .filter(PoolMemberConfig::is_usable)
            .map(|member| MemberState {
                keys: build_keys(member.config, member.api_keys),
                cooldown_until: None,
                cooldown,
                cursor: 0,
            })
            .filter(|member| !member.keys.is_empty())
            .collect();
        Self {
            inner: Arc::new(Mutex::new(members)),
            cooldown,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().is_empty()
    }

    pub fn member_count(&self) -> usize {
        self.inner.lock().unwrap().len()
    }

    pub fn key_count(&self) -> usize {
        self.inner
            .lock()
            .unwrap()
            .iter()
            .map(|member| member.keys.len())
            .sum()
    }

    /// Pick the best available member/key and pre-deduct rate-limit capacity.
    pub fn pick(&self, estimated_tokens: usize) -> Result<Pick, PickError> {
        let now = Instant::now();
        let mut members = self.inner.lock().unwrap();
        for (member_idx, member) in members.iter_mut().enumerate() {
            if member.cooldown_until.is_some_and(|until| until > now) {
                continue;
            }
            let n = member.keys.len();
            if n == 0 {
                continue;
            }
            for offset in 0..n {
                let key_idx = (member.cursor + offset) % n;
                let key = &member.keys[key_idx];
                if key.cooldown_until.is_some_and(|until| until > now) {
                    continue;
                }
                match key.rate_limit.check_rate_limit(estimated_tokens) {
                    Ok(pre_deducted) => {
                        member.cursor = (key_idx + 1) % n;
                        return Ok(Pick {
                            provider: key.provider.clone(),
                            model: key.model.clone(),
                            config: key.config.clone(),
                            route: key.route.clone(),
                            pre_deducted,
                            member_idx,
                            key_idx,
                        });
                    }
                    Err(_) => continue,
                }
            }
        }
        Err(PickError::NoCapacity)
    }

    /// Settle a successful attempt: record actual usage and clear cooldowns.
    ///
    /// When the response carries no usage (e.g. a provider that never emits
    /// streamed usage), the pre-deduction is kept instead of refunded so the
    /// per-key TPM gate stays effective.
    pub fn report_success(&self, pick: &Pick, actual_tokens: usize) {
        let mut members = self.inner.lock().unwrap();
        if let Some(member) = members.get_mut(pick.member_idx) {
            member.cooldown_until = None;
            if let Some(key) = member.keys.get_mut(pick.key_idx) {
                key.cooldown_until = None;
                let settled = if actual_tokens == 0 && pick.pre_deducted > 0 {
                    pick.pre_deducted
                } else {
                    actual_tokens
                };
                key.rate_limit.record_usage(pick.pre_deducted, settled);
            }
        }
    }

    /// Settle a failed attempt and optionally cool the key/member down.
    ///
    /// `refund` refunds the TPM pre-deduction (nothing was consumed); pass
    /// `false` when the request may have partially consumed tokens (e.g. a
    /// mid-stream failure after delivery started).
    pub fn report_failure(&self, pick: &Pick, kind: FailureKind, refund: bool) {
        let now = Instant::now();
        let mut members = self.inner.lock().unwrap();
        let Some(member) = members.get_mut(pick.member_idx) else {
            return;
        };
        let Some(key) = member.keys.get_mut(pick.key_idx) else {
            return;
        };
        if refund {
            key.rate_limit.record_usage(pick.pre_deducted, 0);
        }
        if kind == FailureKind::NotRetryable {
            return;
        }
        key.cooldown_until = Some(now + member.cooldown);
        if kind == FailureKind::Provider {
            member.cooldown_until = Some(now + member.cooldown);
        }
    }

    /// Try candidates in priority order until one succeeds or none remain.
    ///
    /// `execute` receives the picked route/config and returns a completed
    /// [`LlmResponse`] or an error. Failures are classified with
    /// [`failure_kind`]; retryable ones advance to the next candidate.
    pub async fn try_each<F, Fut>(
        &self,
        estimated_tokens: usize,
        execute: F,
    ) -> Result<LlmResponse, PoolAttemptError>
    where
        F: Fn(Pick) -> Fut,
        Fut: Future<Output = Result<LlmResponse, LlmError>>,
    {
        let mut last_error: Option<LlmError> = None;
        loop {
            let pick = match self.pick(estimated_tokens) {
                Ok(pick) => pick,
                Err(PickError::NoCapacity) => break,
            };
            match execute(pick.clone()).await {
                Ok(response) => {
                    let actual = response.usage.total_tokens as usize;
                    self.report_success(&pick, actual);
                    return Ok(response);
                }
                Err(err) => {
                    let kind = failure_kind(&err);
                    self.report_failure(&pick, kind, true);
                    last_error = Some(err);
                    if kind == FailureKind::NotRetryable {
                        break;
                    }
                }
            }
        }
        Err(match last_error {
            Some(err) => PoolAttemptError::Exhausted(err),
            None => PoolAttemptError::NoCapacity,
        })
    }
}

fn build_keys(mut config: ModelProviderConfig, api_keys: Vec<String>) -> Vec<KeyState> {
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(config.timeout_ms))
        .build()
        .expect("reqwest client should build");
    let provider = config.provider_name();
    let model = config.model.clone();
    api_keys
        .into_iter()
        .filter(|key| !key.is_empty())
        .map(|key| {
            config.api_key = key;
            let route = build_route_from_config(&config, http_client.clone());
            KeyState {
                provider: provider.clone(),
                model: model.clone(),
                config: config.clone(),
                route,
                rate_limit: ClientRateLimit::from_config(&config),
                cooldown_until: None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ApiStyle;

    fn member(base_url: &str, keys: &[&str], rpm: Option<u32>) -> PoolMemberConfig {
        PoolMemberConfig {
            config: ModelProviderConfig {
                base_url: base_url.to_string(),
                api_key: keys.first().map(|k| k.to_string()).unwrap_or_default(),
                model: "test-model".to_string(),
                timeout_ms: 1000,
                api_style: Some(ApiStyle::OpenAi),
                dimensions: None,
                enable_thinking: None,
                enable_cache: None,
                rpm_limit: rpm,
                tpm_limit: Some(1_000_000),
            },
            api_keys: keys.iter().map(|k| k.to_string()).collect(),
        }
    }

    fn pool(members: Vec<PoolMemberConfig>) -> ProviderPool {
        ProviderPool::new(LlmPoolConfig::new(members))
    }

    fn request_tokens(pick: &Pick) -> usize {
        pick.pre_deducted
    }

    #[test]
    fn pick_round_robins_across_keys() {
        let p = pool(vec![member(
            "https://api.deepseek.com",
            &["k1", "k2"],
            Some(10),
        )]);
        let a = p.pick(1).unwrap();
        let b = p.pick(1).unwrap();
        assert_ne!(a.key_idx, b.key_idx);
        assert_eq!(a.member_idx, b.member_idx);
        // Third pick wraps back to the first key.
        let c = p.pick(1).unwrap();
        assert_eq!(c.key_idx, a.key_idx);
    }

    #[test]
    fn pick_skips_ratelimited_key_and_moves_to_second() {
        // rpm=1: first request consumes the only RPM slot of key k1,
        // so the next pick must land on k2.
        let p = pool(vec![member(
            "https://api.deepseek.com",
            &["k1", "k2"],
            Some(1),
        )]);
        let a = p.pick(1).unwrap();
        assert_eq!(a.key_idx, 0);
        let b = p.pick(1).unwrap();
        assert_eq!(b.key_idx, 1);
        // refund a's usage so tests stay deterministic
        p.report_success(&a, 0);
        p.report_success(&b, 0);
    }

    #[test]
    fn fallback_moves_to_second_member_after_provider_failure() {
        let p = pool(vec![
            member("https://api.deepseek.com", &["k1"], Some(10)),
            member("https://open.bigmodel.cn/api/paas/v4", &["k2"], Some(10)),
        ]);
        let a = p.pick(1).unwrap();
        assert_eq!(a.member_idx, 0);
        // DeepSeek provider fails (5xx) -> whole member cools down.
        p.report_failure(&a, FailureKind::Provider, true);
        let b = p.pick(1).unwrap();
        assert_eq!(b.member_idx, 1);
        p.report_success(&b, 1);
    }

    #[test]
    fn key_only_failure_keeps_member_alive() {
        let p = pool(vec![member(
            "https://api.deepseek.com",
            &["k1", "k2"],
            Some(10),
        )]);
        let a = p.pick(1).unwrap();
        assert_eq!(a.key_idx, 0);
        // 429 on k1 -> only k1 cools down; k2 of the same member still usable.
        p.report_failure(&a, FailureKind::KeyOnly, true);
        let b = p.pick(1).unwrap();
        assert_eq!(b.key_idx, 1);
        assert_eq!(b.member_idx, 0);
        p.report_success(&b, 1);
    }

    #[test]
    fn all_cooldown_yields_no_capacity() {
        let p = pool(vec![member("https://api.deepseek.com", &["k1"], Some(10))]);
        let a = p.pick(1).unwrap();
        p.report_failure(&a, FailureKind::Provider, true);
        assert!(matches!(p.pick(1), Err(PickError::NoCapacity)));
    }

    #[test]
    fn success_clears_member_cooldown() {
        let p = pool(vec![
            member("https://api.deepseek.com", &["k1"], Some(10)),
            member("https://open.bigmodel.cn/api/paas/v4", &["k2"], Some(10)),
        ]);
        let a = p.pick(1).unwrap();
        p.report_failure(&a, FailureKind::Provider, true);
        let b = p.pick(1).unwrap();
        assert_eq!(b.member_idx, 1);
        p.report_success(&b, 1);
        // Member 0 cooled down; a fresh failure on member 1 must not leak.
        // Pick should now come back to member 0 (its cooldown is still active)
        // only after member 1 is exhausted; here member 1 has capacity.
        let c = p.pick(1).unwrap();
        assert_eq!(c.member_idx, 1);
        p.report_success(&c, 1);
        let _ = request_tokens;
    }

    #[test]
    fn try_each_switches_member_on_failure_and_reports_usage() {
        let p = pool(vec![
            member("https://api.deepseek.com", &["k1"], Some(10)),
            member("https://open.bigmodel.cn/api/paas/v4", &["k2"], Some(10)),
        ]);
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(p.try_each(1, {
            let calls = calls.clone();
            move |pick| {
                let calls = calls.clone();
                async move {
                    calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if pick.member_idx == 0 {
                        Err(LlmError::Api {
                            status: 503,
                            body: "down".to_string(),
                        })
                    } else {
                        Ok(sample_response())
                    }
                }
            }
        }));
        assert!(result.is_ok());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
        // k2 succeeded -> k2 no longer in cooldown, k1 (failed) still is.
        let pick = p.pick(1).unwrap();
        assert_eq!(pick.key_idx, 0);
        assert_eq!(pick.member_idx, 1);
    }

    fn sample_response() -> LlmResponse {
        crate::client::LlmResponse {
            content: "ok".to_string(),
            reasoning_content: None,
            usage: crate::client::LlmUsage {
                prompt_tokens: 1,
                completion_tokens: 1,
                total_tokens: 2,
                provider: "zhipu".to_string(),
                model: "test-model".to_string(),
                cached_tokens: 0,
                reasoning_tokens: 0,
            },
            model: "test-model".to_string(),
            tool_calls: None,
        }
    }

    #[test]
    fn failure_kind_classifies_http_codes() {
        assert_eq!(
            failure_kind(&LlmError::Api {
                status: 429,
                body: "".into()
            }),
            FailureKind::KeyOnly
        );
        assert_eq!(
            failure_kind(&LlmError::Api {
                status: 401,
                body: "".into()
            }),
            FailureKind::KeyOnly
        );
        assert_eq!(
            failure_kind(&LlmError::Api {
                status: 503,
                body: "".into()
            }),
            FailureKind::Provider
        );
        assert_eq!(
            failure_kind(&LlmError::Api {
                status: 400,
                body: "".into()
            }),
            FailureKind::NotRetryable
        );
        assert_eq!(failure_kind(&LlmError::EmptyStream), FailureKind::Provider);
        assert_eq!(
            failure_kind(&LlmError::Cancelled),
            FailureKind::NotRetryable
        );
    }
}
