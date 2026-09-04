pub mod anomaly;
pub mod events;
pub mod service;

#[cfg(test)]
mod tests;

pub use anomaly::detect_request_burst;
pub use events::{CostEvent, CostEventName, ProductEvent, ProductEventName, ResultTag, Surface};
pub use service::AnalyticsService;
