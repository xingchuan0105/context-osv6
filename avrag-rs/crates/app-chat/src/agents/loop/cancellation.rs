//! Cancellation + degradation primitives shared across the ReAct loop.
//!
//! Previously these lived in the `react_loop` skeleton module; that module's
//! scaffolding types (`LoopBudget`, `LoopDecision`, `NextStep`, `ReactStep`,
//! `StepOutcome`, `ReactContext`, `emit_retry_activity`) turned out to be dead
//! and were removed. The two symbols that *are* live — [`cancellation_error`]
//! and the [`DegradeReason`] re-export — were migrated here so their callers
//! (loop driver, synthesis, fallback, runtime) share a single home.

use common::AppError;

/// Re-export the degradation-reason enum so callers can write
/// `super::cancellation::DegradeReason` / `crate::agents::r#loop::cancellation::DegradeReason`
/// instead of reaching into `contracts::chat` directly. Behaviour is unchanged.
pub use contracts::chat::DegradeReason;

/// Construct a cancellation-shaped `AppError`. Uses HTTP 499 (client closed
/// request) so caller chains can distinguish cancellation from real failures.
pub(crate) fn cancellation_error() -> AppError {
    AppError::Internal {
        code: "request_cancelled",
        message: "request cancelled".to_string(),
        http_status: 499,
    }
}
