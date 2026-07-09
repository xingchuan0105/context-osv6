mod billing_context;
mod cost_events;
mod usage_observer_impl;

pub use billing_context::BillingContext;
pub use cost_events::{
    CostEventRecord, record_cost_event_if_available,
    record_external_search_cost_event_if_available, record_storage_cost_event_if_available,
};
pub use usage_observer_impl::PgUsageObserver;

/// Facade alias for quota and metering operations.
pub type BillingService = BillingContext;
