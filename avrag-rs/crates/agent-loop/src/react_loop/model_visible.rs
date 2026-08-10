//! Model-visible View — retrieve phase (architecture deepen W3 / review I4).
//!
//! One composition entry for LLM-boundary messages after system content:
//! history (clear + working-set) → optional budget_hint → optional query_card
//! → claim board. Order is fixed for prefix-cache stability.

use avrag_llm::ChatMessage;

use super::claim_notes::{self, ClaimNoteLine, MAX_CLAIM_NOTES};
use super::context_visibility::{
    self, HISTORY_FULL_RETRIEVAL_ROUNDS, WORKING_SET_CHAR_BUDGET,
};
use super::prompt_assets;
use super::query_card::QueryCard;

/// Inputs for retrieve-phase model-visible composition (excluding system).
pub struct RetrieveViewInputs<'a> {
    pub durable_messages: &'a [ChatMessage],
    pub budget_hint: &'a str,
    pub query_card: Option<&'a QueryCard>,
    pub claim_notes: &'a [ClaimNoteLine],
    /// Active EWS items (KEEP); injected before folded history.
    pub ews_items: &'a [crate::helpers::EwsItem],
    pub keep_recent: usize,
    pub char_budget: usize,
}

impl<'a> RetrieveViewInputs<'a> {
    pub fn defaults(
        durable_messages: &'a [ChatMessage],
        budget_hint: &'a str,
        query_card: Option<&'a QueryCard>,
        claim_notes: &'a [ClaimNoteLine],
    ) -> Self {
        Self {
            durable_messages,
            budget_hint,
            query_card,
            claim_notes,
            ews_items: &[],
            keep_recent: HISTORY_FULL_RETRIEVAL_ROUNDS,
            char_budget: WORKING_SET_CHAR_BUDGET,
        }
    }
}

/// Build non-system messages for one retrieve LLM call.
///
/// Order (fixed): **ews_active** → history view (near-K full + older folded) →
/// budget_hint → query_card → claim_notes.
pub fn build_retrieve_model_visible(inputs: RetrieveViewInputs<'_>) -> Vec<ChatMessage> {
    let mut out = Vec::new();
    if !inputs.ews_items.is_empty() {
        let block = crate::helpers::format_ews_active_block(inputs.ews_items);
        if !block.is_empty() {
            out.push(ChatMessage::user(block));
        }
    }
    out.extend(build_retrieve_history_view_with_budget(
        inputs.durable_messages,
        inputs.keep_recent,
        inputs.char_budget,
    ));
    if !inputs.budget_hint.is_empty() {
        out.push(ChatMessage::user(inputs.budget_hint.to_string()));
    }
    if let Some(card) = inputs.query_card {
        if let Some(block) = super::assembler::build_query_card_block(card) {
            out.push(ChatMessage::user(block));
        }
    }
    if !inputs.claim_notes.is_empty() {
        let lines = claim_notes::format_claim_note_lines(inputs.claim_notes);
        out.push(ChatMessage::user(prompt_assets::claim_notes(
            &lines,
            inputs.claim_notes.len(),
            MAX_CLAIM_NOTES,
        )));
    }
    out
}

/// Transform durable history into model-visible messages (no system rows).
pub fn build_retrieve_history_view(durable_messages: &[ChatMessage]) -> Vec<ChatMessage> {
    build_retrieve_history_view_with_budget(
        durable_messages,
        HISTORY_FULL_RETRIEVAL_ROUNDS,
        WORKING_SET_CHAR_BUDGET,
    )
}

pub fn build_retrieve_history_view_with_budget(
    durable_messages: &[ChatMessage],
    keep_recent: usize,
    char_budget: usize,
) -> Vec<ChatMessage> {
    let mut out = context_visibility::transform_messages_for_llm_with_budget(
        durable_messages,
        keep_recent,
        char_budget,
    );
    out.retain(|m| m.role != "system");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::react_loop::claim_notes::ClaimNoteLine;

    #[test]
    fn order_is_history_budget_claim() {
        let history = vec![ChatMessage::user(
            r##"{"chunks":[{"chunk_id":"1","text":"hello body long enough for retrieval","alias":"#1"}]}"##,
        )];
        let claims = vec![ClaimNoteLine {
            alias: "#1".into(),
            excerpt: "hello body".into(),
        }];
        let out = build_retrieve_model_visible(RetrieveViewInputs::defaults(
            &history,
            "budget-hint-here",
            None,
            &claims,
        ));
        assert!(out.len() >= 3, "history + budget + claims: {out:?}");
        // Last is claims (contains claim_notes marker or excerpt).
        let last = out.last().unwrap().content.clone();
        assert!(
            last.contains("hello body") || last.contains("claim"),
            "{last}"
        );
        // Budget appears before claims.
        let budget_idx = out
            .iter()
            .position(|m| m.content.contains("budget-hint-here"))
            .expect("budget");
        let claim_idx = out.len() - 1;
        assert!(budget_idx < claim_idx);
    }
}
