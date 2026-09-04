use contracts::chat::ChatRequest;
use futures_util::StreamExt;
use web_sdk::{
    BrowserHttpTransport, Cancellation, ChatTransport, FixtureTransport, TransportError,
};

#[tokio::test]
async fn test_fixture_stream_observes_cancellation() {
    let fixture_content = include_str!("../../../tests/fixtures/stream-normal-long.json");
    let transport = FixtureTransport::from_json_lines(fixture_content).unwrap();
    let cancel = Cancellation::new();
    let mut stream = transport
        .stream_chat(
            serde_json::from_value(serde_json::json!({"query": "x", "stream": true})).unwrap(),
            cancel.clone(),
        )
        .await
        .unwrap();

    let first = stream.next().await;
    assert!(first.is_some());
    cancel.cancel();

    let second = stream.next().await;
    assert!(matches!(second, Some(Err(TransportError::Cancelled))));
}

#[tokio::test]
async fn test_native_browser_adapter_is_unavailable() {
    let request: ChatRequest =
        serde_json::from_value(serde_json::json!({"query": "x", "stream": true})).unwrap();

    let browser = BrowserHttpTransport::new("http://localhost:3001", None);
    let browser_result = browser
        .stream_chat(request, Cancellation::new())
        .await;
    assert!(matches!(
        browser_result,
        Err(TransportError::Unavailable(_))
    ));
}
