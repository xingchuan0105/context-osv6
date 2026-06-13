//! RAG plan/evaluation prompt helpers.
#![cfg_attr(not(test), allow(dead_code, unused_imports))]

mod citations;
mod internal;
mod plan;
mod search_eval;
mod strategy_eval;
mod types;

pub use citations::{answer_context, extract_referenced_chunk_ids};
pub use types::*;

pub(crate) use plan::{
    build_rag_plan_user_prompt, execute_plan_request_to_tool_calls, fallback_execute_plan_request,
    normalize_execute_plan_request, parse_rag_plan_decision, plan_strategy_to_tool_calls,
};
pub(crate) use search_eval::{
    build_search_strategy_evaluation_prompt, parse_search_strategy_evaluation,
};
pub(crate) use strategy_eval::{
    build_rag_strategy_evaluation_prompt, parse_rag_strategy_evaluation,
};

#[cfg(test)]
mod tests;
