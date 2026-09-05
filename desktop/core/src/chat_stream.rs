use contracts::chat::ChatEvent;
use thiserror::Error;
use web_sdk::SseDecoder;

#[derive(Debug, Error)]
pub enum DesktopStreamError {
    #[error("HTTP client error: {0}")]
    HttpClient(String),
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
    #[error("Upstream API error ({status}): {message}")]
    UpstreamApi { status: u16, message: String },
    #[error("Framing or decoder error: {0}")]
    Decoder(String),
    #[error("IPC or emitter callback failed: {0}")]
    Callback(String),
}

/// 接收来自本地 API 的 SSE 流，并通过 SseDecoder 无损解析分块事件
pub async fn stream_chat_sse<F, C>(
    api_base_url: &str,
    request_body: &serde_json::Value,
    token: Option<&str>,
    is_cancelled: C,
    mut emit: F,
) -> Result<(), DesktopStreamError>
where
    C: Fn() -> bool + Send + Sync + 'static,
    F: FnMut(&ChatEvent) -> Result<bool, DesktopStreamError> + Send + 'static,
{
    let base = api_base_url.trim_end_matches('/');
    let url = format!("{base}/api/v1/chat");

    let mut body = request_body.clone();
    if let Some(obj) = body.as_object_mut() {
        obj.insert("stream".to_string(), serde_json::json!(true));
    }

    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| DesktopStreamError::HttpClient(format!("build client: {e}")))?;

    let mut req = client
        .post(&url)
        .header(reqwest::header::ACCEPT, "text/event-stream")
        .header(reqwest::header::CONTENT_TYPE, "application/json");

    if let Some(t) = token.filter(|t| !t.trim().is_empty()) {
        req = req.bearer_auth(t.trim());
    }
    req = req.json(&body);

    let resp = req.send().await.map_err(|e| {
        if e.is_connect() {
            DesktopStreamError::ServiceUnavailable(format!(
                "Local product API not reachable at {base} ({e})"
            ))
        } else {
            DesktopStreamError::HttpClient(format!("chat request to {url} failed: {e}"))
        }
    })?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        let message = extract_error_message(status.as_u16(), &text);
        return Err(DesktopStreamError::UpstreamApi {
            status: status.as_u16(),
            message,
        });
    }

    let mut resp = resp;
    let mut decoder = SseDecoder::new();

    loop {
        if is_cancelled() {
            return Ok(());
        }

        match resp.chunk().await {
            Ok(Some(chunk)) => {
                let events = decoder
                    .push(&chunk)
                    .map_err(|e| DesktopStreamError::Decoder(format!("{e}")))?;
                for event in events {
                    if !emit(&event)? {
                        return Ok(());
                    }
                }
            }
            Ok(None) => break,
            Err(e) => {
                return Err(DesktopStreamError::HttpClient(format!(
                    "chat stream read failed: {e}"
                )));
            }
        }
    }

    let remaining = decoder
        .finish()
        .map_err(|e| DesktopStreamError::Decoder(format!("{e}")))?;
    for event in remaining {
        if !emit(&event)? {
            return Ok(());
        }
    }

    Ok(())
}

/// 直接将字节分块流通过 SseDecoder 解码，供测试与本地回放使用
pub fn decode_stream_chunks<I, B, F>(chunks: I, mut emit: F) -> Result<(), DesktopStreamError>
where
    I: IntoIterator<Item = B>,
    B: AsRef<[u8]>,
    F: FnMut(&ChatEvent) -> Result<bool, DesktopStreamError>,
{
    let mut decoder = SseDecoder::new();
    for chunk in chunks {
        let events = decoder
            .push(chunk.as_ref())
            .map_err(|e| DesktopStreamError::Decoder(format!("{e}")))?;
        for event in events {
            if !emit(&event)? {
                return Ok(());
            }
        }
    }
    let remaining = decoder
        .finish()
        .map_err(|e| DesktopStreamError::Decoder(format!("{e}")))?;
    for event in remaining {
        if !emit(&event)? {
            return Ok(());
        }
    }
    Ok(())
}

fn extract_error_message(status: u16, text: &str) -> String {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(msg) = value
            .get("message")
            .or_else(|| value.get("error"))
            .and_then(|m| m.as_str())
        {
            return msg.to_string();
        }
    }
    if text.trim().is_empty() {
        format!("Chat failed (HTTP {status})")
    } else {
        text.to_string()
    }
}
