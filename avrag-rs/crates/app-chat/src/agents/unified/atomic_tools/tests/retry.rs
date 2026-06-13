use crate::agents::capability::RetryPolicy;
use crate::agents::unified::atomic_tools::dispatch::execute_with_retry;
use contracts::{ToolResult, ToolStatus};
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn test_retry_succeeds_on_second_attempt() {
    let counter = std::sync::Arc::new(AtomicUsize::new(0));
    let c = counter.clone();

    let policy = RetryPolicy {
        max_retries: 3,
        backoff_ms: 1,
        backoff_multiplier: 1.0,
        max_backoff_ms: 10,
        idempotent: true,
        idempotency_key_header: None,
    };

    let result = execute_with_retry(
        move || {
            let c = c.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    ToolResult {
                        tool: "x".to_string(),
                        version: "1.0".to_string(),
                        status: ToolStatus::Error,
                        data: Some(serde_json::json!({"error": "transient"})),
                        trace: None,
                    }
                } else {
                    ToolResult {
                        tool: "x".to_string(),
                        version: "1.0".to_string(),
                        status: ToolStatus::Ok,
                        data: Some(serde_json::json!({"ok": true})),
                        trace: None,
                    }
                }
            }
        },
        &policy,
    )
    .await;

    assert_eq!(result.status, ToolStatus::Ok);
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_non_idempotent_skips_retry() {
    let counter = std::sync::Arc::new(AtomicUsize::new(0));
    let c = counter.clone();

    let policy = RetryPolicy {
        max_retries: 3,
        backoff_ms: 1,
        backoff_multiplier: 1.0,
        max_backoff_ms: 10,
        idempotent: false, // non-idempotent
        idempotency_key_header: None,
    };

    let result = execute_with_retry(
        move || {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                ToolResult {
                    tool: "x".to_string(),
                    version: "1.0".to_string(),
                    status: ToolStatus::Error,
                    data: Some(serde_json::json!({"error": "boom"})),
                    trace: None,
                }
            }
        },
        &policy,
    )
    .await;

    assert_eq!(result.status, ToolStatus::Error);
    assert_eq!(counter.load(Ordering::SeqCst), 1); // no retry
}

#[tokio::test]
async fn test_not_found_is_terminal_no_retry() {
    let counter = std::sync::Arc::new(AtomicUsize::new(0));
    let c = counter.clone();

    let policy = RetryPolicy {
        max_retries: 3,
        backoff_ms: 1,
        backoff_multiplier: 1.0,
        max_backoff_ms: 10,
        idempotent: true,
        idempotency_key_header: None,
    };

    let result = execute_with_retry(
        move || {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                ToolResult {
                    tool: "x".to_string(),
                    version: "1.0".to_string(),
                    status: ToolStatus::NotFound,
                    data: None,
                    trace: None,
                }
            }
        },
        &policy,
    )
    .await;

    assert_eq!(result.status, ToolStatus::NotFound);
    assert_eq!(counter.load(Ordering::SeqCst), 1); // no retry for NotFound
}
