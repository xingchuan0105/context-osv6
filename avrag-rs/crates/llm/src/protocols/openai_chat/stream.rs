//! Legacy stream parser used by non-Protocol code paths.
use super::types::*;
use crate::route::SseFramer;
use crate::schema::LlmResponse;
use crate::schema::LlmUsage;

#[derive(Debug)]
pub(crate) struct ChatCompletionStreamParser {
    framer: SseFramer,
    accumulated_content: String,
    accumulated_reasoning: String,
    usage: Option<LlmUsage>,
    model: String,
    provider: String,
}

impl ChatCompletionStreamParser {
    pub(crate) fn new(provider: String, configured_model: String) -> Self {
        Self {
            framer: SseFramer::new(),
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
        let frames = self
            .framer
            .feed_chunk(chunk)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        for frame in frames {
            self.process_frame(&frame, on_content_delta, on_reasoning_delta)?;
        }
        Ok(())
    }

    pub(crate) fn finish(
        mut self,
        on_content_delta: &mut impl FnMut(&str),
        on_reasoning_delta: &mut impl FnMut(&str),
    ) -> anyhow::Result<LlmResponse> {
        let frames = self
            .framer
            .finish()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        for frame in frames {
            self.process_frame(&frame, on_content_delta, on_reasoning_delta)?;
        }

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

    fn process_frame(
        &mut self,
        payload: &str,
        on_content_delta: &mut impl FnMut(&str),
        on_reasoning_delta: &mut impl FnMut(&str),
    ) -> anyhow::Result<()> {
        if payload.trim() == "[DONE]" {
            return Ok(());
        }

        let chunk: StreamChunk = serde_json::from_str(payload).map_err(|error| {
            anyhow::anyhow!("Failed to parse chat completion stream payload: {payload}: {error}")
        })?;

        if let Some(model) = chunk.model {
            self.model = model;
        }

        if let Some(usage) = chunk.usage {
            self.usage = Some(usage.to_llm_usage(self.provider.clone(), self.model.clone()));
        }

        for choice in chunk.choices {
            if let Some(message) = choice.message {
                apply_message_to_accumulators(
                    &message,
                    &mut self.accumulated_content,
                    &mut self.accumulated_reasoning,
                    on_content_delta,
                    on_reasoning_delta,
                );
                continue;
            }

            let Some(delta) = choice.delta else {
                continue;
            };
            apply_delta_to_accumulators(
                &delta,
                &mut self.accumulated_content,
                &mut self.accumulated_reasoning,
                on_content_delta,
                on_reasoning_delta,
            );
        }

        Ok(())
    }
}

