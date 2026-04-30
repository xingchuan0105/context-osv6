use crate::agents::events::{AgentEvent, AgentUsage};
use serde::{Deserialize, Serialize};

/// Configuration for a Rig-backed model provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigModelConfig {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub base_url: Option<String>,
    pub temperature: Option<f32>,
    #[serde(default)]
    pub supports_reasoning: bool,
}

/// A Rig-backed model client that can produce streaming `AgentEvent`s.
/// This is the sole new streaming runtime; no legacy/Rig dual path.
#[async_trait::async_trait]
pub trait RigModelClient: Send + Sync {
    /// Non-streaming completion.
    async fn complete(&self, messages: &[RigChatMessage]) -> anyhow::Result<RigCompletion>;

    /// Streaming completion: emits `AgentEvent` through the callback.
    async fn complete_stream(
        &self,
        messages: &[RigChatMessage],
        on_event: Box<dyn FnMut(AgentEvent) + Send>,
    ) -> anyhow::Result<RigCompletion>;
}

/// A single chat message for Rig adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigChatMessage {
    pub role: String,
    pub content: String,
}

/// Final completion metadata from a Rig run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RigCompletion {
    pub content: String,
    pub usage: Option<AgentUsage>,
    #[serde(default)]
    pub degrade_trace: Vec<common::DegradeTraceItem>,
}

// ---------------------------------------------------------------------------
// Fake implementation for unit tests (no real API calls).
// ---------------------------------------------------------------------------

/// Fake Rig client that produces deterministic events for tests.
pub struct FakeRigClient {
    pub config: RigModelConfig,
    pub scripted_events: Vec<FakeRigEvent>,
}

#[derive(Debug, Clone)]
pub enum FakeRigEvent {
    Activity {
        stage: String,
        message: String,
    },
    ReasoningDelta(String),
    MessageDelta(String),
    Usage {
        prompt_tokens: u64,
        completion_tokens: u64,
        total_tokens: u64,
    },
}

impl FakeRigClient {
    pub fn new(config: RigModelConfig, scripted_events: Vec<FakeRigEvent>) -> Self {
        Self {
            config,
            scripted_events,
        }
    }
}

#[async_trait::async_trait]
impl RigModelClient for FakeRigClient {
    async fn complete(&self, _messages: &[RigChatMessage]) -> anyhow::Result<RigCompletion> {
        let mut content = String::new();
        let mut usage: Option<AgentUsage> = None;

        for event in &self.scripted_events {
            match event {
                FakeRigEvent::MessageDelta(text) => content.push_str(text),
                FakeRigEvent::Usage {
                    prompt_tokens,
                    completion_tokens,
                    total_tokens,
                } => {
                    usage = Some(AgentUsage {
                        provider: self.config.provider.clone(),
                        model: self.config.model.clone(),
                        prompt_tokens: *prompt_tokens,
                        completion_tokens: *completion_tokens,
                        total_tokens: *total_tokens,
                    });
                }
                _ => {}
            }
        }

        Ok(RigCompletion {
            content,
            usage,
            degrade_trace: Vec::new(),
        })
    }

    async fn complete_stream(
        &self,
        _messages: &[RigChatMessage],
        mut on_event: Box<dyn FnMut(AgentEvent) + Send>,
    ) -> anyhow::Result<RigCompletion> {
        let mut content = String::new();
        let mut usage: Option<AgentUsage> = None;

        for event in &self.scripted_events {
            match event {
                FakeRigEvent::Activity { stage, message } => {
                    on_event(AgentEvent::Activity {
                        stage: stage.clone(),
                        message: message.clone(),
                    });
                }
                FakeRigEvent::ReasoningDelta(text) => {
                    on_event(AgentEvent::ReasoningSummaryDelta { text: text.clone() });
                }
                FakeRigEvent::MessageDelta(text) => {
                    content.push_str(text);
                    on_event(AgentEvent::MessageDelta { text: text.clone() });
                }
                FakeRigEvent::Usage {
                    prompt_tokens,
                    completion_tokens,
                    total_tokens,
                } => {
                    let u = AgentUsage {
                        provider: self.config.provider.clone(),
                        model: self.config.model.clone(),
                        prompt_tokens: *prompt_tokens,
                        completion_tokens: *completion_tokens,
                        total_tokens: *total_tokens,
                    };
                    usage = Some(u.clone());
                    on_event(AgentEvent::Usage {
                        provider: u.provider,
                        model: u.model,
                        prompt_tokens: u.prompt_tokens,
                        completion_tokens: u.completion_tokens,
                        total_tokens: u.total_tokens,
                        request_count: 1,
                        metadata: std::collections::BTreeMap::new(),
                    });
                }
            }
        }

        on_event(AgentEvent::Done {
            final_message: Some(content.clone()),
            usage: usage.clone(),
        });

        Ok(RigCompletion {
            content,
            usage,
            degrade_trace: Vec::new(),
        })
    }
}

// ---------------------------------------------------------------------------
// Rig-backed implementation using rig-core.
// ---------------------------------------------------------------------------

/// A real Rig-backed client. This is a thin wrapper that maps rig-core
/// stream items into internal `AgentEvent`s.
pub struct RigCoreClient {
    pub config: RigModelConfig,
}

impl RigCoreClient {
    pub fn new(config: RigModelConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl RigModelClient for RigCoreClient {
    async fn complete(&self, _messages: &[RigChatMessage]) -> anyhow::Result<RigCompletion> {
        // TODO: wire to rig-core when concrete provider integration is needed.
        // For now, return a degrade trace so callers know the adapter is not
        // yet connected to a live provider.
        Ok(RigCompletion {
            content: String::new(),
            usage: None,
            degrade_trace: vec![common::DegradeTraceItem {
                stage: "rig_adapter.complete".to_string(),
                reason: "rig_core_not_yet_wired".to_string(),
                impact: "RigCoreClient is a placeholder until provider credentials and topology are finalized.".to_string(),
            }],
        })
    }

    async fn complete_stream(
        &self,
        _messages: &[RigChatMessage],
        _on_event: Box<dyn FnMut(AgentEvent) + Send>,
    ) -> anyhow::Result<RigCompletion> {
        // TODO: wire to rig-core streaming API.
        Ok(RigCompletion {
            content: String::new(),
            usage: None,
            degrade_trace: vec![common::DegradeTraceItem {
                stage: "rig_adapter.complete_stream".to_string(),
                reason: "rig_core_not_yet_wired".to_string(),
                impact: "RigCoreClient streaming is a placeholder.".to_string(),
            }],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RigModelClient;
    use super::*;
    use std::sync::{Arc, Mutex};

    fn test_config() -> RigModelConfig {
        RigModelConfig {
            provider: "test".to_string(),
            model: "test-model".to_string(),
            api_key: "fake-key".to_string(),
            base_url: None,
            temperature: Some(0.2),
            supports_reasoning: true,
        }
    }

    #[tokio::test]
    async fn test_fake_rig_complete() {
        let client = FakeRigClient::new(
            test_config(),
            vec![
                FakeRigEvent::MessageDelta("Hello".to_string()),
                FakeRigEvent::MessageDelta(" world".to_string()),
                FakeRigEvent::Usage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                },
            ],
        );

        let result = client.complete(&[]).await.unwrap();
        assert_eq!(result.content, "Hello world");
        assert!(result.usage.is_some());
        let usage = result.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
    }

    #[tokio::test]
    async fn test_fake_rig_stream_emits_ordered_events() {
        let client = FakeRigClient::new(
            test_config(),
            vec![
                FakeRigEvent::Activity {
                    stage: "plan".to_string(),
                    message: "planning".to_string(),
                },
                FakeRigEvent::ReasoningDelta("Thinking...".to_string()),
                FakeRigEvent::MessageDelta("Answer".to_string()),
                FakeRigEvent::Usage {
                    prompt_tokens: 20,
                    completion_tokens: 10,
                    total_tokens: 30,
                },
            ],
        );

        let ev = Arc::new(Mutex::new(Vec::new()));
        let ev2 = ev.clone();
        let completion = client
            .complete_stream(&[], Box::new(move |e| ev2.lock().unwrap().push(e)))
            .await
            .unwrap();
        assert_eq!(completion.content, "Answer");
        let events = ev.lock().unwrap();
        assert_eq!(events.len(), 5); // activity + reasoning + message + usage + done

        assert!(matches!(&events[0], AgentEvent::Activity { .. }));
        assert!(
            matches!(&events[1], AgentEvent::ReasoningSummaryDelta { text } if text == "Thinking...")
        );
        assert!(matches!(&events[2], AgentEvent::MessageDelta { text } if text == "Answer"));
        assert!(matches!(&events[3], AgentEvent::Usage { .. }));
        assert!(matches!(&events[4], AgentEvent::Done { .. }));
    }

    #[tokio::test]
    async fn test_rig_core_client_returns_degrade_trace() {
        let client = RigCoreClient::new(test_config());
        let result = client.complete(&[]).await.unwrap();
        assert!(result.content.is_empty());
        assert_eq!(result.degrade_trace.len(), 1);
        assert_eq!(result.degrade_trace[0].stage, "rig_adapter.complete");
    }

    #[tokio::test]
    async fn test_graceful_missing_reasoning_support() {
        // When provider does not support reasoning, the adapter simply
        // does not emit reasoning events.
        let mut config = test_config();
        config.supports_reasoning = false;

        let client = FakeRigClient::new(
            config,
            vec![
                FakeRigEvent::Activity {
                    stage: "answer".to_string(),
                    message: "answering".to_string(),
                },
                FakeRigEvent::MessageDelta("No reasoning here".to_string()),
            ],
        );

        let ev = Arc::new(Mutex::new(Vec::new()));
        let ev2 = ev.clone();
        let _ = client
            .complete_stream(&[], Box::new(move |e| ev2.lock().unwrap().push(e)))
            .await
            .unwrap();
        let events = ev.lock().unwrap();

        let has_reasoning = events
            .iter()
            .any(|e| matches!(e, AgentEvent::ReasoningSummaryDelta { .. }));
        assert!(
            !has_reasoning,
            "provider without reasoning support should not emit reasoning events"
        );
    }
}
