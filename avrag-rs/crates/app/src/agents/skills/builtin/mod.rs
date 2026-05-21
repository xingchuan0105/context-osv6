//! Built-in atomic Skill components.
//!
//! To add a new skill:
//!   1. Create `builtin/your_skill.rs` and implement `SkillComponent`.
//!   2. Add `registry.register(Box::new(YourSkill));` below.

pub mod calculator;
pub mod code_interpreter;
pub mod weather_query;
pub mod web_search;

use super::SkillRegistry;

/// Register all built-in atomic skills into the given registry.
pub fn register_all(registry: &mut SkillRegistry) {
    registry.register(Box::new(calculator::CalculatorSkill));
    registry.register(Box::new(code_interpreter::CodeInterpreterSkill));
    registry.register(Box::new(weather_query::WeatherQuerySkill));
    registry.register(Box::new(web_search::WebSearchSkill));
}
