pub mod anomaly;
pub mod events;
pub mod rollups;
pub mod service;

#[cfg(test)]
mod tests;

pub use anomaly::detect_request_burst;
pub use events::{CostEvent, CostEventName, ProductEvent, ProductEventName, ResultTag, Surface};
pub use rollups::{ActivationInputs, is_activated};
pub use service::AnalyticsService;
