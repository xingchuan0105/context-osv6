//! Compensating-action tracking for the document ingestion pipeline.
//!
//! The pipeline writes to three independent storage systems (Postgres, object
//! store, Milvus) across 12+ sequential operations. True cross-system atomicity
//! would require two-phase commit or a saga orchestrator — neither is practical
//! here. Instead, this module implements a **compensating-transaction log**:
//! each successful side-effect pushes a closure that can undo it; on fatal
//! pipeline failure, the log is drained in reverse order, running each
//! compensation to restore the document to a clean pre-ingestion state.
//!
//! This turns "partial failure leaves half-applied state" into "partial failure
//! triggers best-effort rollback" — the failure is still not atomic, but the
//! blast radius is bounded and observable.

/// A queued compensating action (rollback step).
pub(crate) struct CompensatingAction {
    description: &'static str,
    action: Box<
        dyn FnOnce() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send,
    >,
}

impl std::fmt::Debug for CompensatingAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompensatingAction")
            .field("description", &self.description)
            .finish()
    }
}

/// Accumulator for compensating actions. Push after each successful
/// side-effect; call [`rollback`](Self::rollback) on pipeline failure.
#[derive(Default)]
pub(crate) struct PipelineCompensation {
    actions: Vec<CompensatingAction>,
}

impl PipelineCompensation {
    /// Queue a rollback step. Call this **after** a side-effect succeeds.
    pub(crate) fn push<F, Fut>(&mut self, description: &'static str, f: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        self.actions.push(CompensatingAction {
            description,
            action: Box::new(move || Box::pin(f())),
        });
    }

    /// Drain and execute all compensating actions in reverse order (LIFO).
    /// Each compensation is best-effort — failures are logged but do not
    /// abort the rollback of remaining steps.
    pub(crate) async fn rollback(mut self) {
        while let Some(action) = self.actions.pop() {
            tracing::warn!(
                description = action.description,
                "pipeline compensation: rolling back side-effect"
            );
            (action.action)().await;
        }
    }

    /// Number of queued compensating actions.
    #[allow(dead_code)]
    pub(crate) fn len(&self) -> usize {
        self.actions.len()
    }

    #[allow(dead_code)]
    pub(crate) fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}
