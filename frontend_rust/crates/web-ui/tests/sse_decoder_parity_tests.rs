use contracts::chat::{ChatEvent, ChatRequest};
use futures_util::StreamExt;
use web_sdk::{ChatTransport, FixtureTransport, SseDecoder};
use web_ui::ChatCanvasModel;

fn quick_chat_request() -> ChatRequest {
    ChatRequest {
        query: "写一段 3000 字以上的流式系统说明".to_string(),
        workspace_id: None,
        session_id: None,
        agent_type: "chat".to_string(),
        capabilities: Some(vec![]),
        client_context: None,
        client_ip: None,
        source_type: None,
        source_token: None,
        doc_scope: vec![],
        messages: vec![],
        stream: true,
        debug: false,
        language: None,
        format_hint: None,
        turnstile_token: None,
    }
}

/// Gate B 收口：真实 SSE 分块 wire 经 SseDecoder 解码后，与 FixtureTransport
/// 直推同一事件序列进入 reducer，两者的终态必须完全一致。
#[tokio::test]
async fn chunked_wire_and_fixture_transport_reach_identical_terminal_state() {
    // 路径 A：字节分块 → SseDecoder → ChatEvent
    let chunks_json: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/stream-long-3000.chunks.json"
    ))
    .unwrap();
    let chunks: Vec<String> = chunks_json["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap().to_string())
        .collect();
    let mut decoder = SseDecoder::new();
    let mut decoded: Vec<ChatEvent> = Vec::new();
    for chunk in &chunks {
        decoded.extend(decoder.push(chunk.as_bytes()).unwrap());
    }
    decoded.extend(decoder.finish().unwrap());

    // 路径 B：FixtureTransport（events jsonl）→ ChatEvent
    let transport = FixtureTransport::from_json_lines(include_str!(
        "../../../tests/fixtures/stream-long-3000.events.jsonl"
    ))
    .unwrap();
    let cancellation = web_sdk::Cancellation::new();
    let mut stream = transport
        .stream_chat(quick_chat_request(), cancellation)
        .await
        .unwrap();
    let mut fixtured: Vec<ChatEvent> = Vec::new();
    while let Some(item) = stream.next().await {
        fixtured.push(item.unwrap());
    }

    assert_eq!(decoded, fixtured, "decoder 与 fixture 事件序列必须一致");

    let mut canvas_decoded = ChatCanvasModel::new();
    let turn = canvas_decoded.prepare_user_turn("写一段 3000 字以上的流式系统说明");
    for event in decoded {
        canvas_decoded.on_event(turn.stream_scope, event);
    }

    let mut canvas_fixtured = ChatCanvasModel::new();
    let turn = canvas_fixtured.prepare_user_turn("写一段 3000 字以上的流式系统说明");
    for event in fixtured {
        canvas_fixtured.on_event(turn.stream_scope, event);
    }

    assert_eq!(canvas_decoded.live_turn(), canvas_fixtured.live_turn());
    assert_eq!(
        canvas_decoded.manager().active.messages,
        canvas_fixtured.manager().active.messages
    );
    assert!(matches!(
        canvas_decoded.live_turn().status,
        web_ui::TurnStatus::Done
    ));
    assert!(canvas_decoded.live_turn().answer_text.chars().count() >= 3000);
}
