//! Built-in atomic Skill components.
//!
//! To add a new skill:
//!   1. Create `builtin/your_skill.rs` and implement `SkillComponent`.
//!   2. Add `registry.register(Box::new(YourSkill));` below.
//!
//! Write refine tools live in `write_refine` but are **not** registered here —
//! they are Write control-ring tools (ADR-0007), disclosed via local ToolSpec only.

pub mod calculator;
pub mod code_interpreter;
pub mod conversation_history;
pub mod weather_query;
pub mod web_fetch;
pub mod web_search;
pub mod write_refine;

use super::SkillRegistry;

/// Register ReAct-executable built-in skills (excludes write_refine_*).
pub fn register_all(registry: &mut SkillRegistry) {
    registry.register(Box::new(calculator::CalculatorSkill));
    registry.register(Box::new(code_interpreter::CodeInterpreterSkill));
    registry.register(Box::new(conversation_history::ConversationHistoryLoad));
    registry.register(Box::new(conversation_history::UserProfileLoad));
    registry.register(Box::new(weather_query::WeatherQuerySkill));
    registry.register(Box::new(web_fetch::WebFetchSkill));
    registry.register(Box::new(web_search::WebSearchSkill));
}
