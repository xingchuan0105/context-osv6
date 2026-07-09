use crate::runtime::AgentRunUsage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReActIterationRecord {
    pub iteration: u8,
    pub disclosed_skills: Vec<String>,
    pub action_type: String,
    pub observation_preview: String,
    pub llm_usage: Option<AgentRunUsage>,
    pub elapsed_ms: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub exit_reason: String,
}
