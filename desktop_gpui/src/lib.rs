use web_sdk::{ChatTurnState, FixtureTransport, reduce_chat_event};

/// 把 JSONL / `data:` 夹具收成一轮状态。GPUI 与测试共用，不解析第二套事件类型。
pub fn reduce_fixture_json_lines(json: &str) -> Result<ChatTurnState, serde_json::Error> {
    let transport = FixtureTransport::from_json_lines(json)?;
    let mut state = ChatTurnState::default();
    for event in transport.events() {
        reduce_chat_event(&mut state, event.clone());
    }
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use web_sdk::TurnStatus;

    #[test]
    fn fixture_long_stream_reaches_done() {
        let json = include_str!("../../frontend_rust/tests/fixtures/stream-normal-long.json");
        let state = reduce_fixture_json_lines(json).expect("fixture parses");
        assert_eq!(state.status, TurnStatus::Done);
        assert!(state.answer_text.contains("系统架构设计方案"));
        assert!(
            !state.answer_text.contains("operation_guide"),
            "host guide must not enter the user bubble"
        );
    }
}
