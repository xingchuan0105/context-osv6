use std::sync::Arc;

use avrag_chatmemory::ChatMemory;
use avrag_guardrails::GuardPipeline;
use avrag_rag_core::RagRuntime;

use crate::agents::service::UnifiedAgentService;

#[derive(Clone)]
pub struct OrchestratorContext {
    agent_service: Option<Arc<UnifiedAgentService>>,
    chatmemory: Option<Arc<ChatMemory>>,
    guard_pipeline: Arc<GuardPipeline>,
    rag_runtime: Option<Arc<RagRuntime>>,
}

impl OrchestratorContext {
    pub fn new(
        agent_service: Option<Arc<UnifiedAgentService>>,
        chatmemory: Option<Arc<ChatMemory>>,
        guard_pipeline: Arc<GuardPipeline>,
        rag_runtime: Option<Arc<RagRuntime>>,
    ) -> Self {
        Self {
            agent_service,
            chatmemory,
            guard_pipeline,
            rag_runtime,
        }
    }

    pub fn agent_service(&self) -> Option<Arc<UnifiedAgentService>> {
        self.agent_service.clone()
    }

    pub fn set_agent_service(&mut self, service: UnifiedAgentService) {
        self.agent_service = Some(Arc::new(service));
    }

    pub fn chatmemory(&self) -> Option<&Arc<ChatMemory>> {
        self.chatmemory.as_ref()
    }

    pub fn guard_pipeline(&self) -> &Arc<GuardPipeline> {
        &self.guard_pipeline
    }

    pub fn rag_runtime(&self) -> Option<&Arc<RagRuntime>> {
        self.rag_runtime.as_ref()
    }
}
