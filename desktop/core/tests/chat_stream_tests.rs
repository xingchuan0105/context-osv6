use contracts::chat::ChatEvent;
use desktop_core::{DesktopStreamError, decode_stream_chunks};

#[test]
fn test_multibyte_utf8_chinese_split_across_chunks_is_lossless() {
    // 构造一个包含中文的完整 Token 事件
    let chinese_text = "你好，世界！这是一段用于验证桌面端跨网络分块无损解码的中文内容。";
    let event_json = serde_json::json!({
        "event": "token",
        "request_id": "req-zh",
        "message_id": 1,
        "content": chinese_text,
    });
    let wire_frame = format!("event: token\ndata: {event_json}\n\n");
    let wire_bytes = wire_frame.as_bytes();

    // 验证即使按照极端的 1 字节、2 字节切片，也能无损解析且不产生任何损坏字符
    for chunk_size in [1, 2, 3, 5, 7, 11] {
        let chunks: Vec<&[u8]> = wire_bytes.chunks(chunk_size).collect();
        let mut decoded_events = Vec::new();
        decode_stream_chunks(chunks, |event| {
            decoded_events.push(event.clone());
            Ok(true)
        })
        .expect("should decode cleanly");

        assert_eq!(decoded_events.len(), 1);
        match &decoded_events[0] {
            ChatEvent::Token { content, .. } => {
                assert_eq!(content, chinese_text, "failed at chunk_size {chunk_size}");
                assert!(!content.contains('\u{FFFD}'), "must not contain replacement characters");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}

#[test]
fn test_crlf_and_multiple_events() {
    let raw = "event: start\r\ndata: {\"event\":\"start\",\"request_id\":\"r1\",\"session_id\":\"s1\"}\r\n\r\nevent: done\r\ndata: {\"event\":\"done\",\"request_id\":\"r1\",\"session_id\":\"s1\",\"message_id\":1,\"payload\":{\"answer\":\"完成\",\"answer_blocks\":[],\"session_id\":\"s1\",\"agent_type\":\"chat\",\"sources\":[],\"citations\":[],\"trace\":{\"mode\":\"chat\"},\"degrade_trace\":[]}}\r\n\r\n";
    let mut events = Vec::new();
    decode_stream_chunks([raw.as_bytes()], |event| {
        events.push(event.clone());
        Ok(true)
    })
    .expect("crlf should parse");

    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], ChatEvent::Start { .. }));
    assert!(matches!(events[1], ChatEvent::Done { .. }));
}

#[test]
fn test_bad_frame_returns_decoder_error() {
    let raw = "event: token\ndata: {not-valid-json\n\n";
    let err = decode_stream_chunks([raw.as_bytes()], |_| Ok(true)).expect_err("should fail");
    assert!(matches!(err, DesktopStreamError::Decoder(_)));
}

#[test]
fn test_cancellation_callback_stops_early() {
    let raw = "event: start\ndata: {\"event\":\"start\",\"request_id\":\"r1\",\"session_id\":\"s1\"}\n\nevent: token\ndata: {\"event\":\"token\",\"request_id\":\"r1\",\"message_id\":1,\"content\":\"token1\"}\n\n";
    let mut seen = Vec::new();
    decode_stream_chunks([raw.as_bytes()], |event| {
        seen.push(event.clone());
        Ok(false) // 触发停止
    })
    .expect("early stop is ok");

    assert_eq!(seen.len(), 1);
}
