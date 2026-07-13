//! Real-LLM multi-turn RAG E2E regression tests.
//!
//! Run:
//!   cargo test -p app --test product_e2e llm_real::multi_turn -- --ignored --test-threads=1 --nocapture

use crate::product_e2e::{
    assertions::{assert_answer_substantive, assert_has_citations},
    fixtures::shared_standard_doc_real_llm,
    llm_real::{
        chat_with_retry, chat_with_session_retry, merge_llm_real_extra, non_blocking_degrade,
    },
};

/// Turn 1: document-grounded RAG. Turn 2: follow-up in same session references Taleb.
/// Reuses cold-ingested standard doc (same corpus as rag_real / format_real).
#[tokio::test]
#[ignore = "requires real LLM API key; run with --ignored --test-threads=1"]
async fn real_llm_multi_turn_rag_follow_up_remembers_context() {
    super::require_nightly_suite();
    let (ctx, upload) = shared_standard_doc_real_llm().await;

    let doc_scope = vec![upload.document_id.clone()];

    let turn1 = chat_with_retry(
        &ctx,
        "What is antifragility?",
        &upload.workspace_id,
        &doc_scope,
    )
    .await;
    let resp1 = &turn1.resp;

    assert_has_citations(resp1);
    assert_answer_substantive(resp1, 50);
    let blocking1: Vec<_> = resp1
        .degrade_trace
        .iter()
        .filter(|item| !non_blocking_degrade(item))
        .collect();
    assert!(
        blocking1.is_empty(),
        "turn 1 blocking degrade_trace: {:?}",
        blocking1
    );

    let session_id = resp1.session_id.clone();
    let turn2 = chat_with_session_retry(
        &ctx,
        "Who wrote the book about it?",
        &upload.workspace_id,
        &doc_scope,
        &session_id,
    )
    .await;
    let resp2 = &turn2.resp;

    assert_answer_substantive(resp2, 20);
    let answer_lower = resp2.answer.to_lowercase();
    assert!(
        answer_lower.contains("taleb"),
        "expected turn-2 answer to mention Taleb, got: {}",
        resp2.answer.chars().take(200).collect::<String>()
    );
    let blocking2: Vec<_> = resp2
        .degrade_trace
        .iter()
        .filter(|item| !non_blocking_degrade(item))
        .collect();
    assert!(
        blocking2.is_empty(),
        "turn 2 blocking degrade_trace: {:?}",
        blocking2
    );

    // ADR-0010: server-side query normalization removed; no resolved_query
    // DB write-back to assert. The turn-2 answer mentioning "taleb" above
    // is now the sole proof that the LLM resolved the anaphora on its own.

    let out_dir = ctx.llm_real_artifact_dir("real_llm_multi_turn_rag_follow_up_remembers_context");
    let _ = std::fs::create_dir_all(&out_dir);
    let _ = std::fs::write(
        out_dir.join("turn1_reasoning_summary.txt"),
        &turn1.reasoning.summary,
    );
    let _ = std::fs::write(
        out_dir.join("turn2_reasoning_summary.txt"),
        &turn2.reasoning.summary,
    );

    ctx.save_llm_artifact(
        "real_llm_multi_turn_rag_follow_up_remembers_context",
        resp2,
        merge_llm_real_extra(
            &turn2,
            Some(serde_json::json!({
                "document_id": upload.document_id,
                "session_id": session_id,
                "turn1_answer_len": resp1.answer.len(),
                "turn1_reasoning_delta_count": turn1.reasoning.delta_count,
                "turn1_reasoning_summary_chars": turn1.reasoning.summary.chars().count(),
                "turn1_trace_reasoning_count": turn1.reasoning.trace_reasoning.len(),
                "turn1_prompt_snapshot_count": turn1.reasoning.prompt_snapshots.len(),
                "turn2_reasoning_delta_count": turn2.reasoning.delta_count,
                "turn2_reasoning_summary_chars": turn2.reasoning.summary.chars().count(),
                "turn2_trace_reasoning_count": turn2.reasoning.trace_reasoning.len(),
                "turn2_prompt_snapshot_count": turn2.reasoning.prompt_snapshots.len(),
            })),
        ),
        Some(turn2.reasoning),
    );
}
