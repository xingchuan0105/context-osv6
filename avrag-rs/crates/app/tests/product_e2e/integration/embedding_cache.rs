//! Embedding Redis cache hit — second identical RAG query must not re-call mock embedding.

use std::time::Duration;

use crate::product_e2e::{DocumentStatus, TestContext, assertions::*};

#[tokio::test]
async fn identical_rag_query_hits_embedding_cache() {
    let mut ctx = TestContext::new_embedding_cache().await;

    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    let query = "What is antifragility?";
    let scope = [upload.document_id.clone()];

    let before = ctx.embedding_call_count();
    let first = ctx
        .chat(query, &upload.notebook_id, &scope)
        .await
        .unwrap();
    assert_http_ok(&first);
    let after_first = ctx.embedding_call_count();
    assert!(
        after_first > before,
        "first query should call mock embedding at least once"
    );

    let second = ctx
        .chat(query, &upload.notebook_id, &scope)
        .await
        .unwrap();
    assert_http_ok(&second);
    let after_second = ctx.embedding_call_count();
    let delta_first = after_first.saturating_sub(before);
    let delta_second = after_second.saturating_sub(after_first);

    assert!(
        delta_first > 0,
        "first query should call mock embedding at least once"
    );
    assert_eq!(
        delta_second, 0,
        "second identical query should not call mock embedding again \
         (delta_first={delta_first}, delta_second={delta_second}, total={after_second})"
    );
}
