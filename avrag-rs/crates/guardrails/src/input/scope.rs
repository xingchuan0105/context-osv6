//! Scope enforcement guard.
//!
//! Validates that the user is only querying notebooks they have access to.
//! The actual access check requires database lookup; this guard handles
//! the case where a notebook_id is explicitly provided that doesn't match
//! the user's scope.

use crate::input::{InputGuard, InputGuardContext};
use common::{GuardResult, RiskLevel};

#[derive(Debug, Clone)]
pub struct ScopeGuard;

impl ScopeGuard {
    pub fn new() -> Self {
        Self
    }

    /// Scope check — validates notebook access based on doc_scope.
    ///
    /// If `notebook_id` is provided but not in `doc_scope`, this is a scope violation.
    /// The actual permission lookup (database check) happens elsewhere in the request
    /// pipeline; this guard enforces the client-provided scope contract.
    pub fn check(&self, ctx: &InputGuardContext<'_>) -> Option<GuardResult> {
        // If no notebook_id was specified, no scope violation is possible from this guard
        let notebook_id = ctx.notebook_id?;

        // If doc_scope is empty, user hasn't been granted any notebooks
        if ctx.doc_scope.is_empty() {
            return Some(GuardResult::block(
                "input:scope_violation",
                RiskLevel::High,
                "User has no notebooks in scope",
                ctx.trace_id.clone(),
                None,
            ));
        }

        // Convert doc_scope notebook IDs to strings for comparison
        let requested_notebook = notebook_id.to_string();
        let in_scope = ctx.doc_scope.iter().any(|id| {
            let normalized = id.trim_matches('-');
            normalized == requested_notebook
                || normalized == notebook_id.to_string().replace('-', "")
        });

        if !in_scope {
            return Some(GuardResult::block(
                "input:scope_violation",
                RiskLevel::Medium,
                format!(
                    "Requested notebook {} is not in user's authorized scope",
                    notebook_id
                ),
                ctx.trace_id.clone(),
                None,
            ));
        }

        None
    }
}

impl InputGuard for ScopeGuard {
    fn check(&self, ctx: &InputGuardContext<'_>) -> Option<GuardResult> {
        self.check(ctx)
    }

    fn name(&self) -> &'static str {
        "scope"
    }
}

impl Default for ScopeGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_ctx(
        query: &str,
        notebook_id: Option<Uuid>,
        doc_scope: Vec<String>,
    ) -> InputGuardContext<'_> {
        InputGuardContext {
            query,
            org_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            doc_scope: Box::leak(doc_scope.into_boxed_slice()),
            notebook_id,
            trace_id: Some("test-trace".into()),
        }
    }

    #[test]
    fn test_no_notebook_id_passed() {
        let guard = ScopeGuard::new();
        let ctx = make_ctx(
            "query",
            None,
            vec!["notebook-1".into(), "notebook-2".into()],
        );
        let result = guard.check(&ctx);
        assert!(result.is_none());
    }

    #[test]
    fn test_in_scope_passed() {
        let guard = ScopeGuard::new();
        let notebook_id = Uuid::new_v4();
        let ctx = make_ctx("query", Some(notebook_id), vec![notebook_id.to_string()]);
        let result = guard.check(&ctx);
        assert!(result.is_none());
    }

    #[test]
    fn test_out_of_scope_blocked() {
        let guard = ScopeGuard::new();
        let notebook_id = Uuid::new_v4();
        let ctx = make_ctx("query", Some(notebook_id), vec!["other-notebook".into()]);
        let result = guard.check(&ctx);
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(!r.passed);
        assert_eq!(r.guard_type, "input:scope_violation");
    }

    #[test]
    fn test_empty_scope_blocked() {
        let guard = ScopeGuard::new();
        let notebook_id = Uuid::new_v4();
        let ctx = make_ctx("query", Some(notebook_id), vec![]);
        let result = guard.check(&ctx);
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(!r.passed);
    }
}
