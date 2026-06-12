/// Risk level for tools and skills.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum RiskLevel {
    /// No external dependencies, no sensitive data access.
    #[default]
    Low,
    /// Accesses user data or internal systems.
    Medium,
    /// Accesses external network or executes code.
    High,
    /// Modifies system state or accesses sensitive credentials.
    Critical,
}

/// Retry policy for tool execution.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff_ms: u64,
    pub backoff_multiplier: f64,
    pub max_backoff_ms: u64,
    pub idempotent: bool,
    pub idempotency_key_header: Option<String>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            backoff_ms: 1000,
            backoff_multiplier: 2.0,
            max_backoff_ms: 30000,
            idempotent: false,
            idempotency_key_header: None,
        }
    }
}

/// Deprecation notice for tools and skills.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Deprecation {
    pub since_version: String,
    pub note: String,
    pub replacement_id: Option<String>,
}

/// Permission required to invoke a tool or skill.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Permission {
    /// Standard authenticated user.
    User,
    /// Advanced user with elevated privileges.
    Advanced,
    /// Administrator.
    Admin,
    /// External network access.
    ExternalNetwork,
    /// Code execution.
    CodeExecution,
}

/// Metadata for a registered tool.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolMetadata {
    pub id: String,
    pub version: String,
    pub owner: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub risk_level: RiskLevel,
    pub permissions: Vec<Permission>,
    pub external_deps: Vec<String>,
    pub deprecation: Option<Deprecation>,
    pub retry_policy: RetryPolicy,
    pub activation_phase: ActivationPhase,
    pub applicable_strategies: Vec<String>,
}

/// Metadata for a registered skill.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillMetadata {
    pub id: String,
    pub version: String,
    pub owner: String,
    pub description: String,
    pub applicable_strategies: Vec<String>,
    pub required_tools: Vec<String>,
    pub risk_level: RiskLevel,
    pub deprecation: Option<Deprecation>,
    pub activation_phase: ActivationPhase,
    pub category: String,
    pub input_schema: Option<serde_json::Value>,
    pub output_schema: Option<serde_json::Value>,
}

/// 工具/技能在策略哪个阶段可见
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationPhase {
    /// Plan + Evaluate 阶段可见：检索/搜索工具、规划类工具
    #[default]
    PlanAndEvaluate,
    /// Answer 阶段可见：输出格式技能（html/ppt/teaching）
    Answer,
}
