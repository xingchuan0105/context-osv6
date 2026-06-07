use crate::agents::events::{AgentEvent, AgentEventSink};
use super::config::ModeConfig;
use avrag_llm::{ChatMessage, LlmClient};
use common::AppError;
use tokio_util::sync::CancellationToken;

pub struct SynthesisPhase;

impl SynthesisPhase {
    pub async fn run(
        &self,
        llm: &LlmClient,
        base_prompt: &str,
        mode: &ModeConfig,
        messages: &[ChatMessage],
        disclosed_skills: &[crate::agents::capability::SkillMetadata],
        sink: &dyn AgentEventSink,
        cancel: &CancellationToken,
    ) -> Result<String, AppError> {
        if cancel.is_cancelled() {
            return Err(AppError::internal("synthesis cancelled"));
        }

        let skills_text = if disclosed_skills.is_empty() {
            String::new()
        } else {
            let mut text = String::from("\n\n<skills>\n");
            for skill in disclosed_skills {
                text.push_str(&format!("- {}: {}\n", skill.id, skill.description));
            }
            text.push_str("</skills>");
            text
        };

        let system_content = format!("{}{}", base_prompt, skills_text);
        let system_msg = ChatMessage::system(system_content);

        let mut synthesis_messages = vec![system_msg];
        // Filter out every system message from the ReAct history so the
        // synthesis prompt contains exactly one system instruction. This
        // is robust against fallback-injected system observations that
        // appear at arbitrary positions.
        for msg in messages {
            if msg.role != "system" {
                synthesis_messages.push(msg.clone());
            }
        }

        let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let temperature = mode.temperature.unwrap_or(0.7);
        let stream = llm.complete_stream(
            &synthesis_messages,
            Some(temperature),
            cancel.clone(),
            move |delta| {
                if !delta.is_empty() {
                    let _ = delta_tx.send(delta.to_string());
                }
            },
        );
        tokio::pin!(stream);

        let mut full_answer = String::new();

        let response = loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    return Err(AppError::internal("synthesis cancelled"));
                }
                delta = delta_rx.recv() => {
                    if let Some(delta) = delta {
                        full_answer.push_str(&delta);
                        let _ = sink.emit(AgentEvent::MessageDelta { text: delta }).await;
                    }
                }
                result = &mut stream => {
                    break result.map_err(|e| AppError::internal(format!("synthesis stream failed: {e}")))?;
                }
            }
        };

        while let Ok(delta) = delta_rx.try_recv() {
            full_answer.push_str(&delta);
            let _ = sink.emit(AgentEvent::MessageDelta { text: delta }).await;
        }

        let usage = crate::agents::events::AgentUsage {
            provider: response.usage.provider.clone(),
            model: response.model.clone(),
            prompt_tokens: response.usage.prompt_tokens as u64,
            completion_tokens: response.usage.completion_tokens as u64,
            total_tokens: response.usage.total_tokens as u64,
        };

        let _ = sink
            .emit(AgentEvent::Done {
                final_message: Some(full_answer.clone()),
                usage: Some(usage),
            })
            .await;

        Ok(full_answer)
    }
}
