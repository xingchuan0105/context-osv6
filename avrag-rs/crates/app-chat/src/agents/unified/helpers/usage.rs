use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::runtime::AgentRunUsage;
use avrag_llm::LlmUsage;

pub fn merge_usage(existing: Option<&LlmUsage>, new: &LlmUsage) -> LlmUsage {
    match existing {
        Some(prev) => LlmUsage {
            provider: new.provider.clone(),
            model: new.model.clone(),
            prompt_tokens: prev.prompt_tokens.saturating_add(new.prompt_tokens),
            completion_tokens: prev.completion_tokens.saturating_add(new.completion_tokens),
            total_tokens: prev.total_tokens.saturating_add(new.total_tokens),
            cached_tokens: prev.cached_tokens.saturating_add(new.cached_tokens),
        },
        None => new.clone(),
    }
}

pub fn build_run_usage(usage: Option<&LlmUsage>, request_count: u64) -> Option<AgentRunUsage> {
    usage.map(|u| AgentRunUsage {
        provider: u.provider.clone(),
        model: u.model.clone(),
        prompt_tokens: u.prompt_tokens as u64,
        completion_tokens: u.completion_tokens as u64,
        total_tokens: u.total_tokens as u64,
        request_count,
        cached_tokens: u.cached_tokens as u64,
    })
}

pub fn run_usage_to_agent_usage(usage: &AgentRunUsage) -> crate::agents::events::AgentUsage {
    crate::agents::events::AgentUsage {
        provider: usage.provider.clone(),
        model: usage.model.clone(),
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        cached_tokens: usage.cached_tokens,
    }
}

pub async fn emit_usage(sink: &dyn AgentEventSink, usage: Option<&AgentRunUsage>) {
    if let Some(u) = usage {
        let _ = sink
            .emit(AgentEvent::Usage {
                provider: u.provider.clone(),
                model: u.model.clone(),
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
                request_count: u.request_count,
                metadata: Default::default(),
            })
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_usage(prompt: u32, completion: u32) -> LlmUsage {
        LlmUsage {
            provider: "test".to_string(),
            model: "m".to_string(),
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: prompt + completion,
            cached_tokens: 0,
        }
    }

    #[test]
    fn test_merge_usage_adds_tokens() {
        let a = make_usage(10, 5);
        let b = make_usage(3, 2);
        let merged = merge_usage(Some(&a), &b);
        assert_eq!(merged.prompt_tokens, 13);
        assert_eq!(merged.completion_tokens, 7);
        assert_eq!(merged.total_tokens, 20);
    }

    #[test]
    fn test_merge_usage_none_clones() {
        let b = make_usage(3, 2);
        let merged = merge_usage(None, &b);
        assert_eq!(merged.prompt_tokens, 3);
        assert_eq!(merged.completion_tokens, 2);
    }

    #[test]
    fn test_build_run_usage_maps_fields() {
        let u = make_usage(10, 5);
        let run = build_run_usage(Some(&u), 2).unwrap();
        assert_eq!(run.prompt_tokens, 10);
        assert_eq!(run.completion_tokens, 5);
        assert_eq!(run.total_tokens, 15);
        assert_eq!(run.request_count, 2);
    }

    #[test]
    fn test_build_run_usage_none_returns_none() {
        assert!(build_run_usage(None, 0).is_none());
    }
}
