//! Live end-to-end verification of the OpenAI Responses protocol against
//! DeepSeek's `/v1/responses` endpoint.
//!
//! Exercises the full route path (`build_route_from_config` with
//! `ApiStyle::OpenAiResponses`, non-streaming request/response mapping) and
//! the streaming event path.
//!
//! Run with:
//!   cargo test -p avrag-llm -- --ignored --nocapture responses_live
//!
//! Prerequisites:
//!   - `.env` with `AGENT_LLM_API_KEY` set
//!   - `AGENT_LLM_MODEL` must be `deepseek-v4-flash` (the only model that
//!     currently supports Responses; `deepseek-v4-pro` arrives 2026-08)

use avrag_llm::route::{ReqwestTransport, build_route_from_config};
use avrag_llm::{
    ApiStyle, ChatMessage, GenerationOptions, LlmEvent, LlmRequest, LlmResponse,
    ModelProviderConfig, ToolDefinition,
};
use futures::StreamExt;
use std::sync::Arc;

fn env_config() -> Option<ModelProviderConfig> {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let env_path = repo_root.join(".env");
    if env_path.exists() {
        dotenvy::from_path(&env_path).ok();
    }
    let api_key = std::env::var("AGENT_LLM_API_KEY").ok()?;
    if api_key.is_empty() {
        return None;
    }
    Some(ModelProviderConfig {
        base_url: std::env::var("AGENT_LLM_BASE_URL")
            .unwrap_or_else(|_| "https://api.deepseek.com".to_string()),
        api_key,
        model: std::env::var("AGENT_LLM_MODEL").unwrap_or_else(|_| "deepseek-v4-flash".to_string()),
        timeout_ms: 60_000,
        api_style: Some(ApiStyle::OpenAiResponses),
        dimensions: None,
        enable_thinking: Some(false),
        enable_cache: None,
        rpm_limit: None,
        tpm_limit: None,
    })
}

#[tokio::test]
#[ignore = "requires live AGENT_LLM API key; run with --ignored --nocapture"]
async fn responses_live_non_streaming() {
    let Some(config) = env_config() else {
        eprintln!("SKIP: AGENT_LLM_API_KEY not set");
        return;
    };
    let http = reqwest::Client::new();
    let route = build_route_from_config(&config, Arc::new(ReqwestTransport::new(http)));
    assert_eq!(route.protocol_id(), "openai_responses");

    let request = LlmRequest::new(vec![ChatMessage::user("Reply with exactly: OK")], config)
        .with_options(GenerationOptions {
            temperature: Some(0.0),
            max_tokens: None,
            stream: false,
            json_mode: false,
        });
    let response = route
        .generate(request)
        .await
        .expect("live responses request should succeed");
    println!(
        "non-streaming model={} content={:?} usage={}",
        response.model, response.content, response.usage.total_tokens
    );
    assert!(!response.content.is_empty());
}

#[tokio::test]
#[ignore = "requires live AGENT_LLM API key; run with --ignored --nocapture"]
async fn responses_live_streaming() {
    let Some(config) = env_config() else {
        eprintln!("SKIP: AGENT_LLM_API_KEY not set");
        return;
    };
    let http = reqwest::Client::new();
    let route = build_route_from_config(&config, Arc::new(ReqwestTransport::new(http)));
    assert_eq!(route.protocol_id(), "openai_responses");

    let request = LlmRequest::new(vec![ChatMessage::user("Count from 1 to 3.")], config)
        .with_options(GenerationOptions {
            temperature: Some(0.0),
            max_tokens: None,
            stream: true,
            json_mode: false,
        });
    let mut stream = route.stream(request);
    let mut content = String::new();
    while let Some(event) = stream.next().await {
        match event.expect("stream event should be ok") {
            LlmEvent::TextDelta { text, .. } => content.push_str(&text),
            LlmEvent::Finish { .. } => {}
            LlmEvent::ProviderError { message, .. } => panic!("provider error: {message}"),
            _ => {}
        }
    }
    println!("streaming content={content:?}");
    assert!(
        content.contains('1') && content.contains('2') && content.contains('3'),
        "expected digits 1-3, got {content:?}"
    );
}

/// Full generate path with tools: DeepSeek should emit a `function_call`
/// output item that maps back to `LlmResponse.tool_calls`.
#[tokio::test]
#[ignore = "requires live AGENT_LLM API key; run with --ignored --nocapture"]
async fn responses_live_tool_call() {
    let Some(config) = env_config() else {
        eprintln!("SKIP: AGENT_LLM_API_KEY not set");
        return;
    };
    let http = reqwest::Client::new();
    let route = build_route_from_config(&config, Arc::new(ReqwestTransport::new(http)));

    let request = LlmRequest::new(
        vec![ChatMessage::user(
            "What is the weather in Beijing? Use get_weather.",
        )],
        config,
    )
    .with_options(GenerationOptions {
        temperature: Some(0.0),
        max_tokens: None,
        stream: false,
        json_mode: false,
    })
    .with_tools(vec![ToolDefinition {
        name: "get_weather".to_string(),
        description: "Get the current weather for a city".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": { "city": { "type": "string" } },
            "required": ["city"]
        }),
    }]);
    let response: LlmResponse = route
        .generate(request)
        .await
        .expect("live responses tool-call request should succeed");
    println!(
        "tool-call model={} content={:?} calls={:?}",
        response.model, response.content, response.tool_calls
    );
    let calls = response.tool_calls.expect("expected a tool call");
    assert_eq!(calls[0].tool, "get_weather");
    assert_eq!(calls[0].args["city"], "Beijing");
}
