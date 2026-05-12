//! Input guards — run before the RAG pipeline.
//!
//! Current implementation uses regex-based pattern matching.
//! This is lightweight but can be bypassed with semantic variations.
//!
//! TODO: Evaluate LLM-based semantic guard for production hardening.
//! See: https://github.com/protectai/llm-guard or custom lightweight classifier.

mod privilege_escalation;
mod prompt_injection;
mod scope;

pub use privilege_escalation::PrivilegeEscalationGuard;
pub use prompt_injection::PromptInjectionGuard;
pub use scope::ScopeGuard;

use common::GuardResult;
use uuid::Uuid;

/// Context passed to all input guards.
#[derive(Debug, Clone)]
pub struct InputGuardContext<'a> {
    pub query: &'a str,
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub doc_scope: &'a [String],
    pub notebook_id: Option<Uuid>,
    pub trace_id: Option<String>,
}

/// Individual input guard — returns `None` if passed, `Some(GuardResult)` if blocked.
pub trait InputGuard: Send + Sync {
    fn check(&self, ctx: &InputGuardContext<'_>) -> Option<GuardResult>;
    fn name(&self) -> &'static str;
}

/// Pipeline of input guards — runs sequentially, returns first blocking result.
pub struct InputGuardPipeline {
    guards: Vec<Box<dyn InputGuard>>,
}

impl InputGuardPipeline {
    pub fn new() -> Self {
        let guards: Vec<Box<dyn InputGuard>> = vec![
            Box::new(PromptInjectionGuard::new()),
            Box::new(PrivilegeEscalationGuard::new()),
            Box::new(ScopeGuard::new()),
        ];
        Self { guards }
    }

    /// Run all guards. Returns `None` if all passed, or `Some(result)` for the first blocking guard.
    pub fn run(&self, ctx: &InputGuardContext<'_>) -> Option<GuardResult> {
        for guard in &self.guards {
            if let Some(result) = guard.check(ctx)
                && !result.passed {
                    return Some(result);
                }
        }
        None
    }
}

impl Default for InputGuardPipeline {
    fn default() -> Self {
        Self::new()
    }
}
