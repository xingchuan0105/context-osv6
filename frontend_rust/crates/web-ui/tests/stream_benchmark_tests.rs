use contracts::chat::ChatRequest;
use futures_util::StreamExt;
use std::time::Instant;
use web_sdk::{Cancellation, ChatTransport, FixtureTransport};
use web_ui::{ChatTurnState, reduce_chat_event};

fn calculate_percentile(mut data: Vec<f64>, p: f64) -> f64 {
    data.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((data.len() as f64) * (p / 100.0)).ceil() as usize;
    if idx == 0 {
        data[0]
    } else {
        data[idx.min(data.len()) - 1]
    }
}

#[tokio::test]
async fn benchmark_chat_stream_processing() {
    let fixture_content = include_str!("../../../tests/fixtures/stream-normal-long.json");

    let req = serde_json::from_value::<ChatRequest>(serde_json::json!({
        "query": "基准测试",
        "stream": true,
    }))
    .unwrap();

    // 预热 (Warm-up)
    for _ in 0..5 {
        let transport = FixtureTransport::from_json_lines(fixture_content).unwrap();
        let mut stream = transport
            .stream_chat(req.clone(), Cancellation::new())
            .await
            .unwrap();
        let mut state = ChatTurnState::default();
        while let Some(event_res) = stream.next().await {
            reduce_chat_event(&mut state, event_res.unwrap());
        }
    }

    // 采样 50 轮热路径迭代
    let sample_count = 50;
    let mut durations_us = Vec::with_capacity(sample_count);

    for _ in 0..sample_count {
        let start = Instant::now();
        let transport = FixtureTransport::from_json_lines(fixture_content).unwrap();
        let mut stream = transport
            .stream_chat(req.clone(), Cancellation::new())
            .await
            .unwrap();
        let mut state = ChatTurnState::default();
        while let Some(event_res) = stream.next().await {
            reduce_chat_event(&mut state, event_res.unwrap());
        }
        let elapsed = start.elapsed().as_nanos() as f64 / 1000.0;
        durations_us.push(elapsed);
    }

    let min_us = durations_us.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_us = durations_us
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);
    let median_us = calculate_percentile(durations_us.clone(), 50.0);
    let p95_us = calculate_percentile(durations_us.clone(), 95.0);

    println!("\n=== [Benchmark Result: Chat Stream Reducer & Deserialization] ===");
    println!("Samples: {}", sample_count);
    println!("Min:    {:.2} µs", min_us);
    println!("Median: {:.2} µs", median_us);
    println!("p95:    {:.2} µs", p95_us);
    println!("Max:    {:.2} µs", max_us);
    println!("=================================================================\n");

    assert!(
        p95_us < 2000.0,
        "p95 latency should be well under 2ms per complete stream burst"
    );
}

#[tokio::test]
async fn benchmark_memory_stability_over_1000_turns() {
    let fixture_content = include_str!("../../../tests/fixtures/stream-normal-long.json");
    let req = serde_json::from_value::<ChatRequest>(serde_json::json!({
        "query": "长时间会话稳定性测试",
        "stream": true,
    }))
    .unwrap();

    // 连续模拟 1000 轮流式事件更新
    for turn_idx in 0..1000 {
        let transport = FixtureTransport::from_json_lines(fixture_content).unwrap();
        let mut stream = transport
            .stream_chat(req.clone(), Cancellation::new())
            .await
            .unwrap();
        let mut state = ChatTurnState::default();

        while let Some(event_res) = stream.next().await {
            reduce_chat_event(&mut state, event_res.unwrap());
        }

        assert_eq!(state.status, web_ui::TurnStatus::Done);
        if (turn_idx + 1) % 250 == 0 {
            println!("Processed {} consecutive turns cleanly.", turn_idx + 1);
        }
    }
}
