//! RAG plan/evaluation prompt helpers.
#![cfg_attr(not(test), allow(dead_code, unused_imports))]

mod citations;
mod internal;
mod search_eval;
mod strategy_eval;
mod types;

pub use citations::{answer_context, extract_referenced_chunk_ids};
pub use types::*;

pub(crate) use search_eval::{
    build_search_strategy_evaluation_prompt, parse_search_strategy_evaluation,
};
pub(crate) use strategy_eval::{
    build_rag_strategy_evaluation_prompt, parse_rag_strategy_evaluation,
};

#[cfg(test)]
mod tests;
