use contracts::chat::ChatEvent;
use futures_util::{StreamExt, stream};
use web_sdk::{SseDecoder, TransportError, events_from_byte_stream};

fn frame(event: &str, data: &str) -> String {
    format!("event: {event}\ndata: {data}\n\n")
}

fn start_frame() -> String {
    frame("start", r#"{"request_id":"r1","session_id":"s1"}"#)
}

fn token_frame(content: &str) -> String {
    format!(
        "event: token\ndata: {}\n\n",
        serde_json::json!({"request_id": "r1", "message_id": 1, "content": content})
    )
}

fn done_frame() -> String {
    let payload = serde_json::json!({
        "request_id": "r1",
        "session_id": "s1",
        "message_id": 1,
        "payload": {
            "answer": "最终答案",
            "answer_blocks": [],
            "session_id": "s1",
            "agent_type": "chat",
            "sources": [],
            "citations": [],
            "trace": { "mode": "chat" },
            "degrade_trace": []
        }
    });
    frame("done", &payload.to_string())
}

fn error_frame() -> String {
    frame("error", r#"{"request_id":"r1","code":"boom","message":"失败"}"#)
}

fn feed_all(decoder: &mut SseDecoder, wire: &str) -> Result<Vec<ChatEvent>, TransportError> {
    let mut events = decoder.push(wire.as_bytes())?;
    events.extend(decoder.finish()?);
    Ok(events)
}

// 1. UTF-8 汉字跨 byte chunk：三字节字符被任意拆开后必须无损重组
#[test]
fn utf8_multibyte_split_across_chunks() {
    let wire = token_frame("汉字跨块传输");
    let bytes = wire.as_bytes();
    // 逐字节喂入，是最激进的跨块形态
    let mut decoder = SseDecoder::new();
    let mut events = Vec::new();
    for byte in bytes.chunks(1) {
        events.extend(decoder.push(byte).expect("single-byte pushes must not fail"));
    }
    events.extend(decoder.finish().unwrap());
    assert_eq!(events.len(), 1);
    match &events[0] {
        ChatEvent::Token { content, .. } => assert_eq!(content, "汉字跨块传输"),
        other => panic!("expected Token, got {other:?}"),
    }
}

// 每个可能的双切分点都必须得到相同事件序列（字节级穷举）
#[test]
fn every_two_way_byte_split_yields_same_events() {
    let wire = format!("{}{}{}", start_frame(), token_frame("中英 mixed 内容"), done_frame());
    let bytes = wire.as_bytes();
    let expected = feed_all(&mut SseDecoder::new(), &wire).unwrap();
    for split in 0..=bytes.len() {
        let mut decoder = SseDecoder::new();
        let mut events = decoder.push(&bytes[..split]).unwrap();
        events.extend(decoder.push(&bytes[split..]).unwrap());
        events.extend(decoder.finish().unwrap());
        assert_eq!(events, expected, "split at byte {split}");
    }
}

// 2. `\n` 与 `\r\n` 两种行尾都能正确 framing
#[test]
fn lf_and_crlf_line_endings() {
    let lf = format!("{}{}", start_frame(), token_frame("a"));
    let crlf = lf.replace('\n', "\r\n");
    assert_eq!(
        feed_all(&mut SseDecoder::new(), &lf).unwrap(),
        feed_all(&mut SseDecoder::new(), &crlf).unwrap()
    );
}

// 3. 一个事件的多行 data: 以 \n 拼接后解析
#[test]
fn multi_line_data_is_joined() {
    let wire = "event: token\ndata: {\"request_id\":\"r1\",\"message_id\":1,\ndata: \"content\":\"分段内容\"}\n\n";
    let events = feed_all(&mut SseDecoder::new(), wire).unwrap();
    match &events[0] {
        ChatEvent::Token { content, .. } => assert_eq!(content, "分段内容"),
        other => panic!("expected Token, got {other:?}"),
    }
}

// 4. comment / keepalive 不产生事件，也不冲刷已累积事件
#[test]
fn comments_and_keepalive_are_ignored() {
    let wire = format!(
        ": keep-alive\n\n{}{}: another comment\n\n{}",
        start_frame(),
        token_frame("x"),
        done_frame()
    );
    let events = feed_all(&mut SseDecoder::new(), &wire).unwrap();
    assert_eq!(events.len(), 3);
}

// 5. 一个 chunk 包含多个完整事件
#[test]
fn multiple_events_in_one_chunk() {
    let wire = format!("{}{}{}", start_frame(), token_frame("a"), token_frame("b"));
    let events = feed_all(&mut SseDecoder::new(), &wire).unwrap();
    assert_eq!(events.len(), 3);
}

// 6. 事件跨多个 chunk（含 event: 行被截断）
#[test]
fn event_spanning_many_chunks() {
    let wire = format!("{}{}", start_frame(), done_frame());
    let bytes = wire.as_bytes();
    let mut decoder = SseDecoder::new();
    let mut events = Vec::new();
    for chunk in bytes.chunks(7) {
        events.extend(decoder.push(chunk).unwrap());
    }
    events.extend(decoder.finish().unwrap());
    assert_eq!(events.len(), 2);
}

// 7. EOF 前没有最后空行：残留事件仍由 finish() 交付
#[test]
fn eof_flush_without_trailing_blank_line() {
    let wire = format!("{}{}", start_frame(), done_frame());
    let trimmed = wire.trim_end_matches('\n');
    let events = feed_all(&mut SseDecoder::new(), trimmed).unwrap();
    assert_eq!(events.len(), 2);
    assert!(matches!(events[1], ChatEvent::Done { .. }));
}

// 8a. 缺少 event: 字段 → Framing 错误，绝不静默跳过
#[test]
fn missing_event_field_is_framing_error() {
    let wire = "data: {\"request_id\":\"r1\"}\n\n";
    let result = feed_all(&mut SseDecoder::new(), wire);
    assert!(matches!(result, Err(TransportError::Framing(_))));
}

// 8b. 空 data: → typed error
#[test]
fn empty_data_is_error() {
    let wire = "event: token\ndata:\n\n";
    let result = feed_all(&mut SseDecoder::new(), wire);
    assert!(matches!(
        result,
        Err(TransportError::Framing(_)) | Err(TransportError::Serialization(_))
    ));
}

// 8c. 未知事件名 → Serialization（契约枚举 unknown variant），不进入成功路径
#[test]
fn unknown_event_is_typed_error() {
    let wire = frame("mystery_event", r#"{"request_id":"r1"}"#);
    let result = feed_all(&mut SseDecoder::new(), &wire);
    assert!(matches!(result, Err(TransportError::Serialization(_))));
}

// 8d. 坏 JSON → typed error，且后续合法事件不再被误当作成功流的一部分
#[test]
fn bad_json_is_typed_error() {
    let wire = frame("token", "{not-valid-json");
    let result = feed_all(&mut SseDecoder::new(), &wire);
    assert!(matches!(result, Err(TransportError::Framing(_))));
}

// 9. reader 中途失败：已交付事件保留，错误恰好出现一次，随后流终止
#[tokio::test]
async fn mid_stream_reader_failure_surfaces_once() {
    let chunks: Vec<Result<Vec<u8>, TransportError>> = vec![
        Ok(start_frame().into_bytes()),
        Err(TransportError::Interrupted("connection reset".into())),
        Ok(done_frame().into_bytes()), // 失败后的字节永远不应被消费
    ];
    let items: Vec<_> = events_from_byte_stream(stream::iter(chunks))
        .collect()
        .await;
    assert_eq!(items.len(), 2);
    assert!(matches!(items[0], Ok(ChatEvent::Start { .. })));
    assert!(matches!(
        &items[1],
        Err(TransportError::Interrupted(message)) if message == "connection reset"
    ));
}

// 9b. 坏帧在流中恰好产生一次错误并终止
#[tokio::test]
async fn bad_frame_terminates_stream_with_single_error() {
    let chunks: Vec<Result<Vec<u8>, TransportError>> = vec![
        Ok(start_frame().into_bytes()),
        Ok(frame("token", "{bad").into_bytes()),
        Ok(done_frame().into_bytes()),
    ];
    let items: Vec<_> = events_from_byte_stream(stream::iter(chunks))
        .collect()
        .await;
    assert_eq!(items.len(), 2);
    assert!(matches!(items[0], Ok(ChatEvent::Start { .. })));
    assert!(matches!(items[1], Err(TransportError::Framing(_))));
}

// 10. 正常 done 与单独 error 都能作为终态事件穿过 decoder
#[test]
fn done_and_error_terminal_events_decode() {
    let wire = format!("{}{}", start_frame(), done_frame());
    let events = feed_all(&mut SseDecoder::new(), &wire).unwrap();
    assert!(matches!(events.last(), Some(ChatEvent::Done { .. })));

    let error_only = error_frame();
    let events = feed_all(&mut SseDecoder::new(), &error_only).unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        ChatEvent::Error { code, message, .. } => {
            assert_eq!(code, "boom");
            assert_eq!(message, "失败");
        }
        other => panic!("expected Error, got {other:?}"),
    }
}

// 终态后迟到事件：decoder 如实交付（它不做状态裁决），reducer 负责拒绝。
// 这里证明 decoder 不吞掉迟到事件，拒绝行为由 web-ui reducer 测试锚定。
#[test]
fn late_events_after_terminal_are_delivered_to_reducer() {
    let wire = format!("{}{}{}", start_frame(), done_frame(), token_frame("迟到字节"));
    let events = feed_all(&mut SseDecoder::new(), &wire).unwrap();
    assert_eq!(events.len(), 3);
    assert!(matches!(events[2], ChatEvent::Token { .. }));
}

// 3000+ 字确定性长夹具：分块 wire 解码结果与 events jsonl 完全一致
#[test]
fn long_chunked_fixture_decodes_to_expected_events() {
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
    let mut decoded = Vec::new();
    for chunk in &chunks {
        decoded.extend(decoder.push(chunk.as_bytes()).unwrap());
    }
    decoded.extend(decoder.finish().unwrap());

    let expected: Vec<ChatEvent> = include_str!("../../../tests/fixtures/stream-long-3000.events.jsonl")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();

    assert_eq!(decoded.len(), expected.len());
    assert_eq!(decoded, expected);

    // 夹具性质：答案 ≥3000 字、含 activity/reasoning/citations/done 全链路
    let answer: String = decoded
        .iter()
        .filter_map(|e| match e {
            ChatEvent::Token { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();
    assert!(answer.chars().count() >= 3000, "answer too short");
    assert!(decoded.iter().any(|e| matches!(e, ChatEvent::Activity { .. })));
    assert!(decoded
        .iter()
        .any(|e| matches!(e, ChatEvent::ReasoningSummaryDelta { .. })));
    assert!(decoded.iter().any(|e| matches!(e, ChatEvent::Citations { .. })));
    assert!(matches!(decoded.last(), Some(ChatEvent::Done { .. })));
}
