use avrag_llm::LlmClient;

#[derive(Clone)]
pub struct LlmContext {
    llm_client: Option<LlmClient>,
    memory_llm_client: Option<LlmClient>,
}

impl LlmContext {
    pub fn new(
        llm_client: Option<LlmClient>,
        memory_llm_client: Option<LlmClient>,
    ) -> Self {
        Self {
            llm_client,
            memory_llm_client,
        }
    }

    pub fn memory_llm_temperature(&self) -> Option<f32> {
        Some(0.2)
    }

    pub fn agent_llm_temperature(&self) -> Option<f32> {
        Some(0.2)
    }

    pub fn agent_client(&self) -> Option<&LlmClient> {
        self.llm_client.as_ref()
    }

    pub fn memory_client(&self) -> Option<&LlmClient> {
        self.memory_llm_client.as_ref().or(self.llm_client.as_ref())
    }
}
