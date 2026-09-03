use contracts::chat::ChatRequest;
use futures_util::StreamExt;
use web_sdk::{Cancellation, ChatTransport, FixtureTransport};
use web_ui::{ChatTurnState, TurnStatus, reduce_chat_event};

fn build_test_request(
    query: &str,
    session_id: Option<&str>,
    workspace_id: Option<&str>,
) -> ChatRequest {
    let mut req: ChatRequest = serde_json::from_value(serde_json::json!({
        "query": query,
        "stream": true,
    }))
    .expect("Minimal ChatRequest should parse cleanly");

    req.session_id = session_id.map(|s| s.to_string());
    req.workspace_id = workspace_id.map(|s| s.to_string());
    req
}

#[tokio::test]
async fn test_normal_long_stream_fixture() {
    let fixture_content = include_str!("../../../tests/fixtures/stream-normal-long.json");
    let transport =
        FixtureTransport::from_json_lines(fixture_content).expect("Fixture parsing should succeed");

    let req = build_test_request("测试长文本", Some("sess-100"), None);

    let mut stream = transport
        .stream_chat(req, Cancellation::new())
        .await
        .expect("Stream should initialize");

    let mut state = ChatTurnState::default();

    while let Some(event_res) = stream.next().await {
        let event = event_res.expect("Event should decode cleanly");
        reduce_chat_event(&mut state, event);
    }

    assert_eq!(state.status, TurnStatus::Done);
    assert_eq!(state.session_id.as_deref(), Some("sess-100"));
    assert_eq!(state.message_id, Some(1));
    assert!(state.answer_text.contains("系统架构设计方案"));
    assert!(state.answer_text.contains("web-sdk"));
    assert_eq!(state.citations.len(), 1);
    assert_eq!(state.citations[0]["id"], "cite-1");
}

#[tokio::test]
async fn test_tool_observation_channel_isolation() {
    let fixture_content = include_str!("../../../tests/fixtures/stream-tool-observation.json");
    let transport =
        FixtureTransport::from_json_lines(fixture_content).expect("Fixture parsing should succeed");

    let req = build_test_request("基于白皮书问答", Some("sess-200"), Some("ws-1"));

    let mut stream = transport
        .stream_chat(req, Cancellation::new())
        .await
        .expect("Stream should initialize");

    let mut state = ChatTurnState::default();

    while let Some(event_res) = stream.next().await {
        let event = event_res.expect("Event should decode cleanly");
        reduce_chat_event(&mut state, event);
    }

    assert_eq!(state.status, TurnStatus::Done);
    // 用户主气泡严格只包含模型自然语言输出
    assert_eq!(
        state.answer_text,
        "基于您上传的《架构白皮书》，工作区已成功挂载。"
    );
    // 思考摘要进入独立字段，未拼接进入 answer_text
    assert_eq!(
        state.reasoning_summary,
        "正在结合检索到的文档事实综合分析回答..."
    );
    // 进度与跟踪阶段进入对应集合，未污染 answer_text
    assert_eq!(state.activities.len(), 1);
    assert_eq!(state.activities[0].title, "检索工作区资料");
    assert_eq!(state.trace_stages, vec!["dense_search"]);
    assert_eq!(state.citations.len(), 1);
}

#[tokio::test]
async fn test_cancellation_and_error_fixture() {
    let fixture_content = include_str!("../../../tests/fixtures/stream-cancel-abort.json");
    let transport =
        FixtureTransport::from_json_lines(fixture_content).expect("Fixture parsing should succeed");

    let req = build_test_request("取消测试", Some("sess-300"), None);

    let mut stream = transport
        .stream_chat(req, Cancellation::new())
        .await
        .expect("Stream should initialize");

    let mut state = ChatTurnState::default();

    while let Some(event_res) = stream.next().await {
        let event = event_res.expect("Event should decode cleanly");
        reduce_chat_event(&mut state, event);
    }

    match state.status {
        TurnStatus::Error { code, message } => {
            assert_eq!(code, "client_cancelled");
            assert_eq!(message, "User clicked stop");
        }
        other => panic!("Expected Error status, got {:?}", other),
    }

    assert_eq!(state.answer_text, "正在生成初步构想...");
}
