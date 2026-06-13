use super::{LlmResponse, LlmUsage};
use anyhow::Context;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct StreamChoiceDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    #[serde(default)]
    delta: Option<StreamChoiceDelta>,
}

#[derive(Debug, Deserialize, Default)]
struct PromptTokensDetails {
    #[serde(default)]
    cached_tokens: u32,
}

/// Provider usage block (OpenAI-compatible + DeepSeek cache fields).
#[derive(Debug, Deserialize, Default)]
pub(crate) struct ApiUsageRaw {
    pub(crate) prompt_tokens: u32,
    pub(crate) completion_tokens: u32,
    pub(crate) total_tokens: u32,
    #[serde(default)]
    cached_tokens: u32,
    #[serde(default)]
    prompt_cache_hit_tokens: u32,
    #[serde(default)]
    prompt_tokens_details: Option<PromptTokensDetails>,
}

impl ApiUsageRaw {
    pub(crate) fn from_token_counts(
        prompt_tokens: u32,
        completion_tokens: u32,
        total_tokens: u32,
        cached_tokens: u32,
    ) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cached_tokens,
            prompt_cache_hit_tokens: 0,
            prompt_tokens_details: None,
        }
    }

    pub(crate) fn prompt_tokens(&self) -> u32 {
        self.prompt_tokens
    }

    pub(crate) fn completion_tokens(&self) -> u32 {
        self.completion_tokens
    }

    pub(crate) fn total_tokens(&self) -> u32 {
        self.total_tokens
    }

    pub(crate) fn cached_token_count(&self) -> u32 {
        if self.cached_tokens > 0 {
            self.cached_tokens
        } else if self.prompt_cache_hit_tokens > 0 {
            self.prompt_cache_hit_tokens
        } else {
            self.prompt_tokens_details
                .as_ref()
                .map(|d| d.cached_tokens)
                .unwrap_or(0)
        }
    }

    pub(crate) fn to_llm_usage(&self, provider: String, model: String) -> LlmUsage {
        LlmUsage {
            prompt_tokens: self.prompt_tokens,
            completion_tokens: self.completion_tokens,
            total_tokens: self.total_tokens,
            provider,
            model,
            cached_tokens: self.cached_token_count(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<StreamChoice>,
    #[serde(default)]
    usage: Option<ApiUsageRaw>,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Debug)]
pub(crate) struct ChatCompletionStreamParser {
    buffer: Vec<u8>,
    data_lines: Vec<String>,
    accumulated_content: String,
    accumulated_reasoning: String,
    usage: Option<LlmUsage>,
    model: String,
    provider: String,
}

impl ChatCompletionStreamParser {
    pub(crate) fn new(provider: String, configured_model: String) -> Self {
        Self {
            buffer: Vec::new(),
            data_lines: Vec::new(),
            accumulated_content: String::new(),
            accumulated_reasoning: String::new(),
            usage: None,
            model: configured_model,
            provider,
        }
    }

    pub(crate) fn feed_chunk(
        &mut self,
        chunk: &[u8],
        on_content_delta: &mut impl FnMut(&str),
        on_reasoning_delta: &mut impl FnMut(&str),
    ) -> anyhow::Result<()> {
        self.buffer.extend_from_slice(chunk);

        while let Some(line) = self.take_line()? {
            if line.is_empty() {
                self.flush_event(on_content_delta, on_reasoning_delta)?;
                continue;
            }

            if line.starts_with(':') {
                continue;
            }

            if let Some(value) = line.strip_prefix("data:") {
                self.data_lines.push(value.trim_start().to_string());
            }
        }

        Ok(())
    }

    pub(crate) fn finish(
        mut self,
        on_content_delta: &mut impl FnMut(&str),
        on_reasoning_delta: &mut impl FnMut(&str),
    ) -> anyhow::Result<LlmResponse> {
        if !self.buffer.is_empty() {
            let line = String::from_utf8(std::mem::take(&mut self.buffer))
                .context("Failed to decode trailing chat completion stream line")?;
            let normalized = line.trim_end_matches('\r');
            if let Some(value) = normalized.strip_prefix("data:") {
                self.data_lines.push(value.trim_start().to_string());
            }
        }

        self.flush_event(on_content_delta, on_reasoning_delta)?;

        if self.accumulated_content.is_empty() {
            if self.accumulated_reasoning.is_empty() {
                anyhow::bail!("Chat completion stream finished without content");
            }
            self.accumulated_content = self.accumulated_reasoning.clone();
        }

        let reasoning_content = if self.accumulated_reasoning.is_empty() {
            None
        } else {
            Some(self.accumulated_reasoning.clone())
        };

        Ok(LlmResponse {
            content: self.accumulated_content,
            reasoning_content,
            usage: self.usage.unwrap_or_else(|| LlmUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
                provider: self.provider,
                model: self.model.clone(),
                cached_tokens: 0,
            }),
            model: self.model,
            tool_calls: None,
        })
    }

    fn take_line(&mut self) -> anyhow::Result<Option<String>> {
        let Some(index) = self.buffer.iter().position(|byte| *byte == b'\n') else {
            return Ok(None);
        };

        let mut raw_line = self.buffer.drain(..=index).collect::<Vec<_>>();
        raw_line.pop();
        if raw_line.last() == Some(&b'\r') {
            raw_line.pop();
        }

        let line =
            String::from_utf8(raw_line).context("Failed to decode chat completion stream line")?;
        Ok(Some(line))
    }

    fn flush_event(
        &mut self,
        on_content_delta: &mut impl FnMut(&str),
        on_reasoning_delta: &mut impl FnMut(&str),
    ) -> anyhow::Result<()> {
        if self.data_lines.is_empty() {
            return Ok(());
        }

        let payload = self.data_lines.join("\n");
        self.data_lines.clear();

        if payload.trim() == "[DONE]" {
            return Ok(());
        }

        let chunk: StreamChunk = serde_json::from_str(&payload).with_context(|| {
            format!("Failed to parse chat completion stream payload: {payload}")
        })?;

        if let Some(model) = chunk.model {
            self.model = model;
        }

        if let Some(usage) = chunk.usage {
            self.usage = Some(usage.to_llm_usage(self.provider.clone(), self.model.clone()));
        }

        for choice in chunk.choices {
            let Some(delta) = choice.delta else {
                continue;
            };
            if let Some(reasoning) = delta.reasoning_content {
                if !reasoning.is_empty() {
                    self.accumulated_reasoning.push_str(&reasoning);
                    on_reasoning_delta(&reasoning);
                }
            }
            let Some(content) = delta.content else {
                continue;
            };

            if content.is_empty() {
                continue;
            }

            self.accumulated_content.push_str(&content);
            on_content_delta(&content);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ApiUsageRaw, ChatCompletionStreamParser};

    #[test]
    fn chat_completion_stream_parser_accumulates_delta_content_and_usage() {
        let mut parser =
            ChatCompletionStreamParser::new("openai".to_string(), "gpt-test".to_string());
        let mut observed = String::new();

        parser
            .feed_chunk(
                br#"data: {"id":"chatcmpl-1","model":"gpt-stream","choices":[{"delta":{"content":"Hel"}}]}

"#,
                &mut |delta| observed.push_str(delta),
                &mut |_| {},
            )
            .unwrap();
        parser
            .feed_chunk(
                br#"data: {"choices":[{"delta":{"content":"lo"}}]}

data: {"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":3,"total_tokens":15}}

data: [DONE]

"#,
                &mut |delta| observed.push_str(delta),
                &mut |_| {},
            )
            .unwrap();

        let response = parser
            .finish(&mut |delta| observed.push_str(delta), &mut |_| {})
            .unwrap();

        assert_eq!(observed, "Hello");
        assert_eq!(response.content, "Hello");
        assert_eq!(response.model, "gpt-stream");
        assert_eq!(response.usage.prompt_tokens, 12);
        assert_eq!(response.usage.completion_tokens, 3);
        assert_eq!(response.usage.total_tokens, 15);
        assert_eq!(response.usage.provider, "openai");
    }

    #[test]
    fn chat_completion_stream_parser_handles_chunked_lines() {
        let mut parser =
            ChatCompletionStreamParser::new("openai".to_string(), "gpt-test".to_string());
        let mut observed = String::new();

        parser
            .feed_chunk(
                br#"data: {"choices":[{"delta":{"content":"A"#,
                &mut |delta| observed.push_str(delta),
                &mut |_| {},
            )
            .unwrap();
        parser
            .feed_chunk(
                br#"B"}}]}

data: [DONE]

"#,
                &mut |delta| observed.push_str(delta),
                &mut |_| {},
            )
            .unwrap();

        let response = parser
            .finish(&mut |delta| observed.push_str(delta), &mut |_| {})
            .unwrap();
        assert_eq!(observed, "AB");
        assert_eq!(response.content, "AB");
    }

    #[test]
    fn chat_completion_stream_parser_rejects_empty_content_stream() {
        let mut parser =
            ChatCompletionStreamParser::new("openai".to_string(), "gpt-test".to_string());

        parser
            .feed_chunk(
                br#"data: {"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":0,"total_tokens":12}}

data: [DONE]

"#,
                &mut |_delta| {},
                &mut |_| {},
            )
            .unwrap();

        let error = parser.finish(&mut |_delta| {}, &mut |_| {}).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("Chat completion stream finished without content")
        );
    }

    #[test]
    fn chat_completion_stream_parser_falls_back_to_reasoning_when_content_empty() {
        let mut parser =
            ChatCompletionStreamParser::new("deepseek".to_string(), "deepseek-chat".to_string());

        let mut reasoning_observed = String::new();
        parser
            .feed_chunk(
                br#"data: {"choices":[{"delta":{"reasoning_content":"Final answer from reasoning."}}]}

data: [DONE]

"#,
                &mut |_delta| {},
                &mut |delta| reasoning_observed.push_str(delta),
            )
            .unwrap();

        let response = parser
            .finish(&mut |_delta| {}, &mut |delta| {
                reasoning_observed.push_str(delta)
            })
            .unwrap();
        assert_eq!(reasoning_observed, "Final answer from reasoning.");
        assert_eq!(response.content, "Final answer from reasoning.");
        assert_eq!(
            response.reasoning_content.as_deref(),
            Some("Final answer from reasoning.")
        );
    }

    #[test]
    fn usage_parses_cached_tokens_from_provider_fields() {
        let raw: ApiUsageRaw = serde_json::from_str(
            r#"{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15,"prompt_cache_hit_tokens":8}"#,
        )
        .unwrap();
        assert_eq!(raw.cached_token_count(), 8);

        let raw2: ApiUsageRaw = serde_json::from_str(
            r#"{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15,"prompt_tokens_details":{"cached_tokens":3}}"#,
        )
        .unwrap();
        assert_eq!(raw2.cached_token_count(), 3);

        let usage = raw.to_llm_usage("deepseek".to_string(), "model".to_string());
        assert_eq!(usage.cached_tokens, 8);
    }
}
