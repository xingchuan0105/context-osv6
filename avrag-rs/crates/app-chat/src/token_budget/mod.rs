//! TokenBudgetSimulator — offline token-consumption analysis for development.

mod types;
mod scenarios;
mod simulate;
mod report;

#[cfg(test)]
mod tests;

pub use types::{Scenario, SimulationResult, StageEstimate};
pub use scenarios::default_scenarios;
pub use simulate::{simulate_all, simulate_scenario};
pub use report::print_report;
