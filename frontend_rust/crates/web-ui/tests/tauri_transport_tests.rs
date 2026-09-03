use contracts::chat::ChatRequest;
use futures_util::StreamExt;
use web_sdk::{
    BrowserHttpTransport, Cancellation, ChatTransport, FixtureTransport, TauriIpcTransport,
    TransportError,
};
use web_ui::{ChatTurnState, reduce_chat_event};

#[tokio::test]
async fn test_fixture_and_tauri_mock_reduce_to_the_same_state() {
    let fixture_content = include_str!("../../../tests/fixtures/stream-tool-observation.json");

    let req: ChatRequest = serde_json::from_value(serde_json::json!({
        "query": "测试两端等价性",
        "stream": true,
    }))
    .unwrap();

    // 1. 运行协议 fixture 基线
    let fixture_transport = FixtureTransport::from_json_lines(fixture_content).unwrap();
    let mut fixture_stream = fixture_transport
        .stream_chat(req.clone(), Cancellation::new())
        .await
        .unwrap();
    let mut fixture_state = ChatTurnState::default();
    let mut parsed_events = Vec::new();
    while let Some(ev_res) = fixture_stream.next().await {
        let ev = ev_res.unwrap();
        parsed_events.push(ev.clone());
        reduce_chat_event(&mut fixture_state, ev);
    }

    // 2. 运行 Tauri adapter 的确定性 mock seam
    let tauri_transport = TauriIpcTransport::with_mock_events(parsed_events);
    let mut tauri_stream = tauri_transport
        .stream_chat(req, Cancellation::new())
        .await
        .unwrap();
    let mut tauri_state = ChatTurnState::default();
    while let Some(ev_res) = tauri_stream.next().await {
        let ev = ev_res.unwrap();
        reduce_chat_event(&mut tauri_state, ev);
    }

    // 3. 严格对等断言 (Parity Check)
    assert_eq!(fixture_state.status, tauri_state.status);
    assert_eq!(fixture_state.answer_text, tauri_state.answer_text);
    assert_eq!(
        fixture_state.reasoning_summary,
        tauri_state.reasoning_summary
    );
    assert_eq!(fixture_state.citations, tauri_state.citations);
    assert_eq!(fixture_state.activities, tauri_state.activities);
    assert_eq!(fixture_state.trace_stages, tauri_state.trace_stages);
}

#[tokio::test]
async fn test_tauri_mock_stream_observes_cancellation() {
    let fixture_content = include_str!("../../../tests/fixtures/stream-normal-long.json");
    let web_transport = FixtureTransport::from_json_lines(fixture_content).unwrap();
    let mut stream = web_transport
        .stream_chat(
            serde_json::from_value(serde_json::json!({"query":"x","stream":true})).unwrap(),
            Cancellation::new(),
        )
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Some(ev) = stream.next().await {
        events.push(ev.unwrap());
    }

    let tauri = TauriIpcTransport::with_mock_events(events);
    let cancel = Cancellation::new();
    let mut tauri_stream = tauri
        .stream_chat(
            serde_json::from_value(serde_json::json!({"query":"x","stream":true})).unwrap(),
            cancel.clone(),
        )
        .await
        .unwrap();

    // 读一条后主动 cancel
    let first = tauri_stream.next().await;
    assert!(first.is_some());
    cancel.cancel();

    let second = tauri_stream.next().await;
    assert!(matches!(
        second,
        Some(Err(web_sdk::TransportError::Cancelled))
    ));
}

#[tokio::test]
async fn test_unimplemented_real_adapters_fail_explicitly() {
    let request: ChatRequest =
        serde_json::from_value(serde_json::json!({"query":"x","stream":true})).unwrap();

    let browser = BrowserHttpTransport::new("http://localhost:3001", None);
    let browser_result = browser
        .stream_chat(request.clone(), Cancellation::new())
        .await;
    assert!(matches!(
        browser_result,
        Err(TransportError::Unavailable(_))
    ));

    let tauri_result = TauriIpcTransport::new()
        .stream_chat(request, Cancellation::new())
        .await;
    assert!(matches!(tauri_result, Err(TransportError::Unavailable(_))));
}
