//! TokenBudgetSimulator — offline token-consumption analysis for development.

mod report;
mod scenarios;
mod simulate;
mod types;

#[cfg(test)]
mod tests;

pub use report::print_report;
pub use scenarios::default_scenarios;
pub use simulate::{simulate_all, simulate_scenario};
pub use types::{Scenario, SimulationResult, StageEstimate};
