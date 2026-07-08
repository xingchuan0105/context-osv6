//! Explicit policy for how the document pipeline treats non-fatal failures.
//!
//! Previously the pipeline had inconsistent error handling: TOC/profile writes
//! logged and continued, while asset/chunk writes failed the whole pipeline,
//! with no codified rule for which is which. This enum makes the decision
//! explicit at each callsite.

/// How a non-fatal pipeline step failure should be treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegradationPolicy {
    /// The step is optional; log the error and continue with a degraded result.
    Tolerate,
    /// The step is required; propagate the error and fail the pipeline.
    Fatal,
}

impl DegradationPolicy {
    /// Apply this policy to a `Result`. `Tolerate` logs (at warn level, with
    /// context) and returns `Ok(default)`; `Fatal` returns the error unchanged.
    pub fn apply<T: Default, E: std::fmt::Display>(
        self,
        result: Result<T, E>,
        context: &str,
    ) -> Result<T, E> {
        match self {
            Self::Tolerate => match result {
                Ok(v) => Ok(v),
                Err(e) => {
                    tracing::warn!(context, error = %e, "pipeline step degraded");
                    Ok(T::default())
                }
            },
            Self::Fatal => result,
        }
    }
}
