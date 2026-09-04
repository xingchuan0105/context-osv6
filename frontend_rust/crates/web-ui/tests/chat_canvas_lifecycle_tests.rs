use contracts::chat::ChatEvent;
use web_ui::{ChatCanvasModel, MessageRole, TurnStatus};

fn done_payload(answer: &str) -> serde_json::Value {
    done_payload_with(answer, serde_json::json!([]), serde_json::json!([]))
}

fn done_payload_with(
    answer: &str,
    answer_blocks: serde_json::Value,
    citations: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "answer": answer,
        "answer_blocks": answer_blocks,
        "session_id": "fixture-session",
        "agent_type": "chat",
        "sources": [],
        "citations": citations,
        "trace": { "mode": "chat" },
        "degrade_trace": []
    })
}

#[test]
fn test_chat_canvas_personal_conversation_lifecycle() {
    let mut canvas = ChatCanvasModel::new();
    canvas.new_personal_chat(Some("quick_chat"));
    assert_eq!(canvas.manager().active.workspace_id, None);
    assert_eq!(canvas.manager().active.model_role, "quick_chat");

    let turn = canvas.prepare_user_turn("什么是 Context-OS？");
    let scope = turn.stream_scope;
    assert_eq!(turn.request.query, "什么是 Context-OS？");
    assert_eq!(turn.request.workspace_id, None);
    assert!(canvas.is_streaming());
    assert_eq!(canvas.manager().active.messages.len(), 1);
    assert_eq!(canvas.manager().active.messages[0].role, MessageRole::User);

    assert!(canvas.on_event(
        scope,
        ChatEvent::Start {
            request_id: "srv-req-101".to_string(),
            session_id: "sess-101".to_string(),
        },
    ));
    assert_eq!(
        canvas.live_turn().request_id.as_deref(),
        Some("srv-req-101")
    );

    canvas.on_event(
        scope,
        ChatEvent::AnswerStart {
            request_id: "srv-req-101".to_string(),
            session_id: "sess-101".to_string(),
            message_id: 1,
            agent_type: "chat".to_string(),
        },
    );
    canvas.on_event(
        scope,
        ChatEvent::Token {
            request_id: "srv-req-101".to_string(),
            message_id: 1,
            content: "Context-OS 是以知识库为核心的操作系统。".to_string(),
        },
    );
    canvas.on_event(
        scope,
        ChatEvent::Done {
            request_id: "srv-req-101".to_string(),
            session_id: "sess-101".to_string(),
            message_id: 1,
            payload: done_payload("Context-OS 是以知识库为核心的操作系统。"),
        },
    );

    assert!(!canvas.is_streaming());
    assert_eq!(canvas.manager().active.messages.len(), 2);
    let assistant_msg = &canvas.manager().active.messages[1];
    assert_eq!(assistant_msg.role, MessageRole::Assistant);
    assert_eq!(
        assistant_msg.content,
        "Context-OS 是以知识库为核心的操作系统。"
    );
    assert_eq!(
        canvas.manager().active.session_id.as_deref(),
        Some("sess-101")
    );
}

#[test]
fn test_mismatched_server_request_events_are_ignored() {
    let mut canvas = ChatCanvasModel::new();
    let turn = canvas.prepare_user_turn("当前新问题");
    let scope = turn.stream_scope;

    canvas.on_event(
        scope,
        ChatEvent::Start {
            request_id: "srv-req-new".to_string(),
            session_id: "sess-new".to_string(),
        },
    );
    assert!(!canvas.on_event(
        scope,
        ChatEvent::Token {
            request_id: "srv-req-old".to_string(),
            message_id: 99,
            content: "这是旧请求的迟到残片".to_string(),
        },
    ));

    assert!(canvas.live_turn().answer_text.is_empty());
    assert_eq!(
        canvas.manager().active.session_id.as_deref(),
        Some("sess-new")
    );
    assert!(canvas.on_event(
        scope,
        ChatEvent::Token {
            request_id: "srv-req-new".to_string(),
            message_id: 1,
            content: "合法内容".to_string(),
        },
    ));
    assert_eq!(canvas.live_turn().answer_text, "合法内容");
}

#[test]
fn test_late_start_cannot_capture_replacement_turn() {
    let mut canvas = ChatCanvasModel::new();
    let old_turn = canvas.prepare_user_turn("旧问题");
    let old_cancel = old_turn.cancellation.clone();
    let new_turn = canvas.prepare_user_turn("新问题");

    assert!(old_cancel.is_cancelled());
    assert!(!canvas.on_event(
        old_turn.stream_scope,
        ChatEvent::Start {
            request_id: "srv-old".to_string(),
            session_id: "sess-old".to_string(),
        },
    ));
    assert!(canvas.live_turn().request_id.is_none());

    assert!(canvas.on_event(
        new_turn.stream_scope,
        ChatEvent::Start {
            request_id: "srv-new".to_string(),
            session_id: "sess-new".to_string(),
        },
    ));
    assert_eq!(canvas.live_turn().request_id.as_deref(), Some("srv-new"));
}

#[test]
fn test_done_payload_serves_as_final_authority() {
    let mut canvas = ChatCanvasModel::new();
    let scope = canvas.prepare_user_turn("测试 Done 收束").stream_scope;
    canvas.on_event(
        scope,
        ChatEvent::Start {
            request_id: "srv-req-done".to_string(),
            session_id: "sess-done".to_string(),
        },
    );
    canvas.on_event(
        scope,
        ChatEvent::Token {
            request_id: "srv-req-done".to_string(),
            message_id: 1,
            content: "流式草稿文案...".to_string(),
        },
    );
    canvas.on_event(
        scope,
        ChatEvent::Done {
            request_id: "srv-req-done".to_string(),
            session_id: "sess-done".to_string(),
            message_id: 1,
            payload: done_payload("最终经审核修正后的权威文案。"),
        },
    );

    assert_eq!(canvas.manager().active.messages.len(), 2);
    assert_eq!(
        canvas.manager().active.messages[1].content,
        "最终经审核修正后的权威文案。"
    );
}

#[test]
fn test_done_empty_answer_uses_blocks_and_preserves_stream_citation_content() {
    let mut canvas = ChatCanvasModel::new();
    let scope = canvas.prepare_user_turn("结构化终答").stream_scope;
    canvas.on_event(
        scope,
        ChatEvent::Start {
            request_id: "srv-blocks".to_string(),
            session_id: "sess-blocks".to_string(),
        },
    );
    canvas.on_event(
        scope,
        ChatEvent::Token {
            request_id: "srv-blocks".to_string(),
            message_id: 4,
            content: "必须被清除的流式草稿".to_string(),
        },
    );
    canvas.on_event(
        scope,
        ChatEvent::Citations {
            request_id: "srv-blocks".to_string(),
            message_id: 4,
            citations: vec![serde_json::json!({
                "citation_id": 7,
                "doc_id": "doc-7",
                "doc_name": "文档",
                "score": 0.9,
                "content": "完整引用正文",
                "preview": "完整预览"
            })],
        },
    );
    canvas.on_event(
        scope,
        ChatEvent::Done {
            request_id: "srv-blocks".to_string(),
            session_id: "sess-blocks".to_string(),
            message_id: 4,
            payload: done_payload_with(
                "",
                serde_json::json!([
                    { "type": "text", "text": "来自答案块的终答", "citations": [] }
                ]),
                serde_json::json!([{
                    "citation_id": 7,
                    "doc_id": "doc-7",
                    "doc_name": "文档",
                    "score": 0.9,
                    "content": null,
                    "preview": null
                }]),
            ),
        },
    );

    let message = &canvas.manager().active.messages[1];
    assert_eq!(message.content, "来自答案块的终答");
    assert_eq!(message.answer_blocks.len(), 1);
    assert_eq!(message.citations[0]["content"], "完整引用正文");
    assert_eq!(message.citations[0]["preview"], "完整预览");
}

#[test]
fn test_done_empty_answer_and_blocks_clears_stream_draft() {
    let mut canvas = ChatCanvasModel::new();
    let scope = canvas.prepare_user_turn("空终答").stream_scope;
    canvas.on_event(
        scope,
        ChatEvent::Start {
            request_id: "srv-empty".to_string(),
            session_id: "sess-empty".to_string(),
        },
    );
    canvas.on_event(
        scope,
        ChatEvent::Token {
            request_id: "srv-empty".to_string(),
            message_id: 5,
            content: "不得保留".to_string(),
        },
    );
    canvas.on_event(
        scope,
        ChatEvent::Done {
            request_id: "srv-empty".to_string(),
            session_id: "sess-empty".to_string(),
            message_id: 5,
            payload: done_payload(""),
        },
    );

    assert!(canvas.manager().active.messages[1].content.is_empty());
}

#[test]
fn test_invalid_done_payload_is_an_explicit_error() {
    let mut canvas = ChatCanvasModel::new();
    let scope = canvas.prepare_user_turn("坏协议").stream_scope;
    canvas.on_event(
        scope,
        ChatEvent::Start {
            request_id: "srv-invalid".to_string(),
            session_id: "sess-invalid".to_string(),
        },
    );
    canvas.on_event(
        scope,
        ChatEvent::Done {
            request_id: "srv-invalid".to_string(),
            session_id: "sess-invalid".to_string(),
            message_id: 6,
            payload: serde_json::json!({ "answer": "缺少必需契约字段" }),
        },
    );

    assert!(matches!(
        canvas.live_turn().status,
        TurnStatus::Error { ref code, .. } if code == "invalid_done_payload"
    ));
    assert_eq!(canvas.manager().active.messages.len(), 1);
}

#[test]
fn test_switching_conversation_invalidates_stream_before_reducer() {
    let mut canvas = ChatCanvasModel::new();
    canvas.new_personal_chat(None);
    let turn = canvas.prepare_user_turn("正在个人会话提问");
    let cancel = turn.cancellation.clone();
    canvas.on_event(
        turn.stream_scope,
        ChatEvent::Start {
            request_id: "srv-req-mid".to_string(),
            session_id: "sess-personal-1".to_string(),
        },
    );

    canvas.switch_to_workspace("ws-finance", Some("sess-finance-1"));
    assert!(cancel.is_cancelled());
    assert!(!canvas.on_event(
        turn.stream_scope,
        ChatEvent::Token {
            request_id: "srv-req-mid".to_string(),
            message_id: 1,
            content: "旧回答内容".to_string(),
        },
    ));

    assert_eq!(
        canvas.manager().active.workspace_id.as_deref(),
        Some("ws-finance")
    );
    assert_eq!(
        canvas.manager().active.session_id.as_deref(),
        Some("sess-finance-1")
    );
    assert!(canvas.manager().active.messages.is_empty());
    assert_eq!(canvas.live_turn().status, TurnStatus::Idle);
    assert!(canvas.live_turn().answer_text.is_empty());
}

#[test]
fn test_switching_to_personal_session_invalidates_stream() {
    let mut canvas = ChatCanvasModel::new();
    let turn = canvas.prepare_user_turn("旧会话问题");

    canvas.switch_to_personal_session("sess-resumed");
    assert!(turn.cancellation.is_cancelled());
    assert_eq!(
        canvas.manager().active.session_id.as_deref(),
        Some("sess-resumed")
    );
    assert!(canvas.manager().active.workspace_id.is_none());
    assert!(canvas.manager().active.messages.is_empty());
    assert_eq!(canvas.live_turn().status, TurnStatus::Idle);
}

#[test]
fn test_error_only_stream_is_terminal() {
    let mut canvas = ChatCanvasModel::new();
    let scope = canvas.prepare_user_turn("预检失败").stream_scope;

    assert!(canvas.on_event(
        scope,
        ChatEvent::Error {
            request_id: "srv-preflight-error".to_string(),
            code: "session_not_found".to_string(),
            message: "Session not found".to_string(),
        },
    ));
    assert_eq!(
        canvas.live_turn().request_id.as_deref(),
        Some("srv-preflight-error")
    );
    assert!(matches!(
        canvas.live_turn().status,
        TurnStatus::Error { ref code, .. } if code == "session_not_found"
    ));
    assert!(!canvas.cancel());
    assert!(matches!(
        canvas.live_turn().status,
        TurnStatus::Error { .. }
    ));
}

#[test]
fn test_terminal_error_ignores_subsequent_events_and_cancel() {
    let mut canvas = ChatCanvasModel::new();
    let scope = canvas.prepare_user_turn("测试失败终态").stream_scope;
    canvas.on_event(
        scope,
        ChatEvent::Start {
            request_id: "srv-req-err".to_string(),
            session_id: "sess-err".to_string(),
        },
    );
    canvas.on_event(
        scope,
        ChatEvent::Token {
            request_id: "srv-req-err".to_string(),
            message_id: 1,
            content: "初步部分内容".to_string(),
        },
    );
    canvas.on_event(
        scope,
        ChatEvent::Error {
            request_id: "srv-req-err".to_string(),
            code: "rate_limited".to_string(),
            message: "Too many requests".to_string(),
        },
    );

    assert!(!canvas.on_event(
        scope,
        ChatEvent::Token {
            request_id: "srv-req-err".to_string(),
            message_id: 1,
            content: "终态后的非法数据".to_string(),
        },
    ));
    assert!(!canvas.cancel());
    assert_eq!(canvas.live_turn().answer_text, "初步部分内容");
    assert!(matches!(
        canvas.live_turn().status,
        TurnStatus::Error { .. }
    ));
}

#[test]
fn test_duplicate_done_events_do_not_duplicate_messages() {
    let mut canvas = ChatCanvasModel::new();
    let scope = canvas.prepare_user_turn("测试去重 Done").stream_scope;
    canvas.on_event(
        scope,
        ChatEvent::Start {
            request_id: "srv-req-dup".to_string(),
            session_id: "sess-dup".to_string(),
        },
    );
    assert!(canvas.on_event(
        scope,
        ChatEvent::Done {
            request_id: "srv-req-dup".to_string(),
            session_id: "sess-dup".to_string(),
            message_id: 1,
            payload: done_payload("唯一回答"),
        },
    ));
    assert!(!canvas.on_event(
        scope,
        ChatEvent::Done {
            request_id: "srv-req-dup".to_string(),
            session_id: "sess-dup".to_string(),
            message_id: 1,
            payload: done_payload("重复回答"),
        },
    ));
    assert_eq!(canvas.manager().active.messages.len(), 2);
}

#[test]
fn test_server_session_id_persists_across_turns() {
    let mut canvas = ChatCanvasModel::new();
    let turn1 = canvas.prepare_user_turn("轮次 1");
    assert_eq!(turn1.request.session_id, None);
    canvas.on_event(
        turn1.stream_scope,
        ChatEvent::Start {
            request_id: "srv-req-t1".to_string(),
            session_id: "sess-created-by-server".to_string(),
        },
    );
    canvas.on_event(
        turn1.stream_scope,
        ChatEvent::Done {
            request_id: "srv-req-t1".to_string(),
            session_id: "sess-created-by-server".to_string(),
            message_id: 1,
            payload: done_payload("第一轮"),
        },
    );

    let turn2 = canvas.prepare_user_turn("轮次 2 继续追问");
    assert_eq!(
        turn2.request.session_id.as_deref(),
        Some("sess-created-by-server")
    );
}

#[test]
fn test_cancel_and_retry_replace_the_stream_scope() {
    let mut canvas = ChatCanvasModel::new();
    let first = canvas.prepare_user_turn("需要重试的问题");
    assert!(canvas.cancel());
    assert!(first.cancellation.is_cancelled());

    let retry = canvas
        .retry_last()
        .expect("user message should be retryable");
    assert_ne!(first.stream_scope, retry.stream_scope);
    assert_eq!(retry.request.query, "需要重试的问题");
    assert_eq!(canvas.manager().active.messages.len(), 1);
    assert!(canvas.is_streaming());
}

#[test]
fn test_workspace_switch_preserves_model_role() {
    let mut canvas = ChatCanvasModel::new();
    canvas.new_personal_chat(Some("agent"));
    canvas.switch_to_workspace("ws-team", Some("sess-ws"));
    assert_eq!(canvas.manager().active.model_role, "agent");
}

#[test]
fn test_transport_error_enters_recoverable_error_state() {
    let mut canvas = ChatCanvasModel::new();
    let turn = canvas.prepare_user_turn("一次会失败的提问");
    assert!(canvas.on_transport_error(
        turn.stream_scope,
        web_sdk::TransportError::Unauthorized
    ));
    match &canvas.live_turn().status {
        TurnStatus::Error { code, message } => {
            assert_eq!(code, "unauthorized");
            assert!(message.contains("401"));
        }
        other => panic!("expected Error, got {other:?}"),
    }
    // transport 失败后流句柄已清理，可以立即重试
    let retry = canvas.retry_last().expect("user message retryable");
    assert!(canvas.is_streaming());
    assert_ne!(turn.stream_scope, retry.stream_scope);
}

#[test]
fn test_late_transport_error_from_old_scope_is_ignored() {
    let mut canvas = ChatCanvasModel::new();
    let stale = canvas.prepare_user_turn("第一轮");
    let current = canvas.prepare_user_turn("第二轮");
    assert!(!canvas.on_transport_error(
        stale.stream_scope,
        web_sdk::TransportError::Network("old connection reset".to_string())
    ));
    assert!(canvas.is_streaming());
    assert!(!matches!(
        canvas.live_turn().status,
        TurnStatus::Error { .. }
    ));
    assert!(canvas.on_transport_error(
        current.stream_scope,
        web_sdk::TransportError::RateLimited
    ));
    assert!(matches!(
        canvas.live_turn().status,
        TurnStatus::Error { .. }
    ));
}

#[test]
fn test_transport_error_does_not_override_terminal_state() {
    let mut canvas = ChatCanvasModel::new();
    let turn = canvas.prepare_user_turn("先完成的提问");
    canvas.on_event(
        turn.stream_scope,
        ChatEvent::Start {
            request_id: "srv-req-x".to_string(),
            session_id: "sess-x".to_string(),
        },
    );
    canvas.on_event(
        turn.stream_scope,
        ChatEvent::Done {
            request_id: "srv-req-x".to_string(),
            session_id: "sess-x".to_string(),
            message_id: 1,
            payload: done_payload("已定稿"),
        },
    );
    assert!(!canvas.on_transport_error(
        turn.stream_scope,
        web_sdk::TransportError::Interrupted("late abort".to_string())
    ));
    assert!(matches!(canvas.live_turn().status, TurnStatus::Done));
}

#[test]
fn test_quick_chat_request_contract_shape() {
    let mut canvas = ChatCanvasModel::new();
    let turn = canvas.prepare_user_turn("契约形态检查");
    assert_eq!(turn.request.workspace_id, None);
    assert_eq!(turn.request.session_id, None);
    assert_eq!(turn.request.agent_type, "chat");
    assert_eq!(turn.request.capabilities, Some(vec![]));
    assert!(turn.request.stream);
    let json = serde_json::to_value(&turn.request).unwrap();
    assert!(json.get("model_role").is_none());
    assert!(json.get("request_id").is_none());
}
