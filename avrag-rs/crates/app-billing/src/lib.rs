mod billing_context;
mod cost_events;

pub use billing_context::BillingContext;
pub use cost_events::{
    record_cost_event_if_available, record_external_search_cost_event_if_available,
    record_storage_cost_event_if_available, CostEventRecord,
};

/// Facade alias for quota and metering operations.
pub type BillingService = BillingContext;
