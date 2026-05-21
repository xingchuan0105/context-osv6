//! Structured error taxonomy for v5 agent layer.
//!
//! Replaces `AppError::Internal { code: "..." }` string-matching with a typed
//! enum so that strategies can make informed retry / degrade / abort decisions.
//!
//! Each variant carries enough context for telemetry, debugging, and user-facing
//! messages without leaking sensitive internals.

use crate::agents::capability::Permission;
use serde::{Deserialize, Serialize};

/// Classification of agent-layer failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentErrorKind {
    // ---------- Tool layer — retriable ----------
    /// Tool execution raised an unexpected error.
    ToolExecutionFailed { tool: String, reason: String },
    /// Tool did not complete within the allotted time.
    ToolTimeout { tool: String, timeout_ms: u64 },
    /// Tool was rate-limited by its provider.
    ToolRateLimited {
        tool: String,
        retry_after_ms: Option<u64>,
    },

    // ---------- Tool layer — non-retriable ----------
    /// Tool has been deprecated and should no longer be called.
    ToolDeprecated { tool: String },
    /// Tool arguments did not match the declared input schema.
    ToolSchemaMismatch {
        tool: String,
        expected: String,
        got: String,
    },
    /// Tool returned data that could not be parsed.
    ToolOutputMalformed { tool: String, raw: String },

    // ---------- Model layer — retriable ----------
    /// LLM provider returned a 5xx or connection error.
    ModelUnavailable { provider: String, model: String },
    /// LLM provider rate-limited the request.
    ModelRateLimited,

    // ---------- Model layer — non-retriable ----------
    /// Prompt exceeded the model's context window.
    ModelContextExceeded { used_tokens: u64, max_tokens: u64 },
    /// Model output did not conform to the expected format.
    ModelOutputInvalid { expected_schema: String, got: String },
    /// Model output failed JSON schema validation.
    ModelOutputSchemaMismatch {
        expected: String,
        got: serde_json::Value,
    },

    // ---------- Budget / resource — non-retriable ----------
    /// ReAct loop exhausted its iteration budget.
    BudgetExhausted { current: u8, max: u8 },
    /// Total prompt context (history + system + retrieval) exceeds hard limit.
    ContextWindowExceeded,

    // ---------- Permission — non-retriable ----------
    /// Caller lacks permission to invoke the tool.
    PermissionDenied { tool: String, required: Vec<Permission> },

    // ---------- External dependency — retriable ----------
    /// An external service (search, weather, etc.) failed.
    ExternalDependencyFailed { service: String, error: String },

    // ---------- Catch-all ----------
    /// Unclassified failure — should be refined over time.
    Unknown(String),
}

impl AgentErrorKind {
    /// Whether the error is transient and a retry may succeed.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            AgentErrorKind::ToolExecutionFailed { .. }
                | AgentErrorKind::ToolTimeout { .. }
                | AgentErrorKind::ToolRateLimited { .. }
                | AgentErrorKind::ModelUnavailable { .. }
                | AgentErrorKind::ModelRateLimited
                | AgentErrorKind::ExternalDependencyFailed { .. }
        )
    }

    /// Whether the failure can be degraded (skip the failing tool and continue).
    pub fn is_degradable(&self) -> bool {
        matches!(
            self,
            AgentErrorKind::ToolExecutionFailed { .. }
                | AgentErrorKind::ToolTimeout { .. }
                | AgentErrorKind::ToolSchemaMismatch { .. }
                | AgentErrorKind::ToolOutputMalformed { .. }
                | AgentErrorKind::ExternalDependencyFailed { .. }
        )
    }

    /// The minimum handling strategy a consumer must apply.
    pub fn minimum_strategy(&self) -> ErrorHandlingStrategy {
        match self {
            // Retriable errors — at least retry once.
            AgentErrorKind::ToolExecutionFailed { .. } => ErrorHandlingStrategy::Retry,
            AgentErrorKind::ToolTimeout { .. } => ErrorHandlingStrategy::Retry,
            AgentErrorKind::ToolRateLimited { .. } => ErrorHandlingStrategy::Retry,
            AgentErrorKind::ModelUnavailable { .. } => ErrorHandlingStrategy::Retry,
            AgentErrorKind::ModelRateLimited => ErrorHandlingStrategy::Retry,
            AgentErrorKind::ExternalDependencyFailed { .. } => ErrorHandlingStrategy::Retry,

            // Degradable but not retriable — skip and continue.
            AgentErrorKind::ToolSchemaMismatch { .. } => ErrorHandlingStrategy::Skip,
            AgentErrorKind::ToolOutputMalformed { .. } => ErrorHandlingStrategy::Skip,

            // Model format errors — try fallback (simpler prompt / different model).
            AgentErrorKind::ModelOutputInvalid { .. } => ErrorHandlingStrategy::Fallback,
            AgentErrorKind::ModelOutputSchemaMismatch { .. } => ErrorHandlingStrategy::Fallback,

            // Context exceeded — compress or truncate then retry.
            AgentErrorKind::ModelContextExceeded { .. } => ErrorHandlingStrategy::Fallback,

            // Budget exhausted — synthesise answer from accumulated state.
            AgentErrorKind::BudgetExhausted { .. } => ErrorHandlingStrategy::Fallback,

            // Permission errors — mask and continue without exposing details.
            AgentErrorKind::PermissionDenied { .. } => ErrorHandlingStrategy::MaskAndContinue,

            // Terminal errors — stop immediately.
            AgentErrorKind::ToolDeprecated { .. } => ErrorHandlingStrategy::Abort,
            AgentErrorKind::ContextWindowExceeded => ErrorHandlingStrategy::Abort,
            AgentErrorKind::Unknown(_) => ErrorHandlingStrategy::Abort,
        }
    }

    /// Convert to `AppError` for crossing the agent → service boundary.
    ///
    /// Maps permission errors to 403, rate-limit errors to 429, budget
    /// exhaustion to 400, and everything else to 500.
    pub fn to_app_error(&self) -> common::AppError {
        match self {
            AgentErrorKind::PermissionDenied { .. } => {
                common::AppError::forbidden("permission_denied", self.display_message())
            }
            AgentErrorKind::ToolRateLimited { retry_after_ms, .. } => {
                common::AppError::rate_limited(
                    "tool_rate_limited",
                    self.display_message(),
                    retry_after_ms.unwrap_or(60_000) / 1000,
                )
            }
            AgentErrorKind::ModelRateLimited => {
                common::AppError::rate_limited("model_rate_limited", self.display_message(), 60)
            }
            AgentErrorKind::BudgetExhausted { .. } => {
                common::AppError::validation("budget_exhausted", self.display_message())
            }
            AgentErrorKind::ToolDeprecated { .. }
            | AgentErrorKind::ToolSchemaMismatch { .. }
            | AgentErrorKind::ToolOutputMalformed { .. } => {
                common::AppError::validation("tool_error", self.display_message())
            }
            AgentErrorKind::ModelContextExceeded { .. }
            | AgentErrorKind::ModelOutputInvalid { .. }
            | AgentErrorKind::ModelOutputSchemaMismatch { .. } => {
                common::AppError::validation("model_error", self.display_message())
            }
            _ => common::AppError::internal(self.display_message()),
        }
    }

    /// Human-readable, non-sensitive message suitable for logs or debug traces.
    pub fn display_message(&self) -> String {
        match self {
            AgentErrorKind::ToolExecutionFailed { tool, reason } => {
                format!("tool '{}' execution failed: {}", tool, reason)
            }
            AgentErrorKind::ToolTimeout { tool, timeout_ms } => {
                format!("tool '{}' timed out after {} ms", tool, timeout_ms)
            }
            AgentErrorKind::ToolRateLimited { tool, .. } => {
                format!("tool '{}' rate limited", tool)
            }
            AgentErrorKind::ToolDeprecated { tool } => {
                format!("tool '{}' is deprecated", tool)
            }
            AgentErrorKind::ToolSchemaMismatch { tool, .. } => {
                format!("tool '{}' schema mismatch", tool)
            }
            AgentErrorKind::ToolOutputMalformed { tool, .. } => {
                format!("tool '{}' returned malformed output", tool)
            }
            AgentErrorKind::ModelUnavailable { provider, model } => {
                format!("model '{}' on provider '{}' unavailable", model, provider)
            }
            AgentErrorKind::ModelRateLimited => "model rate limited".to_string(),
            AgentErrorKind::ModelContextExceeded { used_tokens, max_tokens } => {
                format!("context exceeded: {} / {} tokens", used_tokens, max_tokens)
            }
            AgentErrorKind::ModelOutputInvalid { .. } => "model output invalid".to_string(),
            AgentErrorKind::ModelOutputSchemaMismatch { .. } => {
                "model output schema mismatch".to_string()
            }
            AgentErrorKind::BudgetExhausted { current, max } => {
                format!("budget exhausted: {} / {}", current, max)
            }
            AgentErrorKind::ContextWindowExceeded => "context window exceeded".to_string(),
            AgentErrorKind::PermissionDenied { tool, .. } => {
                format!("permission denied for tool '{}'", tool)
            }
            AgentErrorKind::ExternalDependencyFailed { service, .. } => {
                format!("external dependency '{}' failed", service)
            }
            AgentErrorKind::Unknown(msg) => format!("unknown error: {}", msg),
        }
    }
}

/// Minimum error-handling strategy required for a given failure kind.
///
/// Strategies are ordered from least to most severe:
/// Retry < Skip < Fallback < MaskAndContinue < Abort
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorHandlingStrategy {
    /// Retry the same operation with backoff.
    Retry,
    /// Skip the failing step and continue with remaining work.
    Skip,
    /// Use a simplified fallback (compressed prompt, single tool, etc.).
    Fallback,
    /// Mask sensitive details and return a generic safe response.
    MaskAndContinue,
    /// Stop immediately and return accumulated results or error.
    Abort,
}

impl ErrorHandlingStrategy {
    /// Whether this strategy is at least as strong as `other`.
    /// Abort >= MaskAndContinue >= Fallback >= Skip >= Retry
    pub fn is_at_least(&self, other: Self) -> bool {
        let rank = |s: &Self| match s {
            ErrorHandlingStrategy::Retry => 0,
            ErrorHandlingStrategy::Skip => 1,
            ErrorHandlingStrategy::Fallback => 2,
            ErrorHandlingStrategy::MaskAndContinue => 3,
            ErrorHandlingStrategy::Abort => 4,
        };
        rank(self) >= rank(&other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_execution_failed_is_retriable() {
        let e = AgentErrorKind::ToolExecutionFailed {
            tool: "web_search".to_string(),
            reason: "timeout".to_string(),
        };
        assert!(e.is_retriable());
        assert!(e.is_degradable());
        assert_eq!(e.minimum_strategy(), ErrorHandlingStrategy::Retry);
    }

    #[test]
    fn tool_deprecated_is_not_retriable() {
        let e = AgentErrorKind::ToolDeprecated {
            tool: "old_api".to_string(),
        };
        assert!(!e.is_retriable());
        assert!(!e.is_degradable());
        assert_eq!(e.minimum_strategy(), ErrorHandlingStrategy::Abort);
    }

    #[test]
    fn permission_denied_masks() {
        let e = AgentErrorKind::PermissionDenied {
            tool: "code_interpreter".to_string(),
            required: vec![Permission::CodeExecution],
        };
        assert!(!e.is_retriable());
        assert!(!e.is_degradable());
        assert_eq!(e.minimum_strategy(), ErrorHandlingStrategy::MaskAndContinue);
    }

    #[test]
    fn budget_exhausted_fallbacks() {
        let e = AgentErrorKind::BudgetExhausted { current: 4, max: 4 };
        assert!(!e.is_retriable());
        assert!(!e.is_degradable());
        assert_eq!(e.minimum_strategy(), ErrorHandlingStrategy::Fallback);
    }

    #[test]
    fn strategy_ranking() {
        assert!(ErrorHandlingStrategy::Abort.is_at_least(ErrorHandlingStrategy::Retry));
        assert!(ErrorHandlingStrategy::Abort.is_at_least(ErrorHandlingStrategy::Abort));
        assert!(!ErrorHandlingStrategy::Retry.is_at_least(ErrorHandlingStrategy::Skip));
        assert!(ErrorHandlingStrategy::Fallback.is_at_least(ErrorHandlingStrategy::Skip));
        assert!(
            ErrorHandlingStrategy::MaskAndContinue.is_at_least(ErrorHandlingStrategy::Fallback)
        );
    }

    #[test]
    fn serde_roundtrip() {
        let e = AgentErrorKind::ToolTimeout {
            tool: "web_search".to_string(),
            timeout_ms: 5000,
        };
        let json = serde_json::to_string(&e).unwrap();
        let parsed: AgentErrorKind = serde_json::from_str(&json).unwrap();
        assert_eq!(e, parsed);
    }

    #[test]
    fn serde_tagged_variant() {
        let json = r#"{"kind":"model_rate_limited"}"#;
        let parsed: AgentErrorKind = serde_json::from_str(json).unwrap();
        assert_eq!(parsed, AgentErrorKind::ModelRateLimited);
    }

    #[test]
    fn display_does_not_leak_raw_output() {
        let e = AgentErrorKind::ToolOutputMalformed {
            tool: "x".to_string(),
            raw: "secret_data".to_string(),
        };
        let msg = e.display_message();
        assert!(!msg.contains("secret_data"));
        assert!(msg.contains("malformed"));
    }
}
