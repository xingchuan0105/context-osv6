//! 题型卡（query-card）结构化埋点机制（2026-08-03）。
//!
//! Pre-loop 一次 `json_mode` 调用把当前查询分类为
//! `calculation / rag_fact / table_count / chitchat / other` 并声明
//! `required_actions`（SDK 原语 id 列表）。卡缺省 = 埋点不激活：
//! 通用证据闸（`exit_policy::has_retrieval_observation`）仍然生效。
//!
//! 边界：这是纯结构埋点，不是语义裁判（决策②）。闸只做「计数」：
//! 零 Ok 回传 / 必做动作缺失，才在 DirectAnswer 接受点放行或拦截；
//! coverage / 语义充分性仍归 skill + model（AGENTS.md stop-decision）。

use std::collections::HashSet;

use avrag_llm::{ChatMessage, LlmClient};
use contracts::ToolResult;
use serde::{Deserialize, Serialize};

use super::config::ModeConfig;

/// 题型 v1（4+1 类，决策③）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionType {
    Calculation,
    RagFact,
    TableCount,
    Chitchat,
    /// 宽容解析：未知字符串一律落到 `other`（先例 answer_contract/parse.rs）。
    #[serde(other)]
    Other,
}

impl Default for QuestionType {
    fn default() -> Self {
        QuestionType::Other
    }
}

impl QuestionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            QuestionType::Calculation => "calculation",
            QuestionType::RagFact => "rag_fact",
            QuestionType::TableCount => "table_count",
            QuestionType::Chitchat => "chitchat",
            QuestionType::Other => "other",
        }
    }
}

/// 题型卡：`question_type` + 本次查询必须完成的 SDK 动作列表。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct QueryCard {
    pub question_type: QuestionType,
    pub required_actions: Vec<String>,
}

impl Default for QueryCard {
    fn default() -> Self {
        Self {
            question_type: QuestionType::Other,
            required_actions: Vec::new(),
        }
    }
}

/// 宽容解析（答案契约 parse.rs 先例）：剥 json fence 后直接反序列化，
/// 未知 type → `other`，缺字段 → 默认。
pub fn parse_query_card(raw: &str) -> Option<QueryCard> {
    let stripped = super::json_fence::strip_json_fence(raw);
    serde_json::from_str::<QueryCard>(&stripped).ok()
}

impl QueryCard {
    /// 过滤 `required_actions`：
    /// 1. 未知动作（不在 `contracts::sdk_primitives` 注册表）→ 丢弃。
    /// 2. 已知但未挂载（不在 `mode.sdk_primitives`）→ 丢弃（未挂载的动作
    ///    要求无意义——沙箱里根本不可达）。
    /// 返回清洗后的卡。清洗后的空 `required_actions` 即「无必做动作」。
    pub fn validate(&self, mode: &ModeConfig) -> QueryCard {
        let mounted: HashSet<&str> = mode.sdk_primitives.iter().map(|s| s.as_str()).collect();
        let required_actions = self
            .required_actions
            .iter()
            .filter(|a| {
                contracts::sdk_primitives::primitive(a).is_some() && mounted.contains(a.as_str())
            })
            .cloned()
            .collect();
        QueryCard {
            question_type: self.question_type,
            required_actions,
        }
    }
}

/// 原语 id → ToolResult.tool 名的别名表。SDK 方法名（`web`/`history`/`save`…）
/// 与桥接层落盘的 ToolResult.tool（`web_search`/`conversation_history_load`/
/// `session_fs_save`…）不一致，`deps.rs::SacHostBridge::call` 逐项映射。
fn action_tool_aliases(action: &str) -> &[&'static str] {
    match action {
        "calculator" => &["calculator"],
        "weather_query" => &["weather_query"],
        "user_context" => &["user_context"],
        "web" => &["web_search"],
        "fetch" => &["web_fetch"],
        "history" => &["conversation_history_load"],
        "user_profile" => &["user_profile_load"],
        "save" => &["session_fs_save"],
        "load" => &["session_fs_load"],
        // 检索原语：runtime 桥接按能力命名，别名表覆盖主要形态。
        "dense" => &["dense_retrieval", "dense"],
        "lexical" => &["lexical_retrieval", "lexical"],
        "grep" => &["doc_grep", "grep"],
        "doc_profile" => &["doc_profile"],
        "doc_summary" => &["doc_summary"],
        "struct_catalog" => &["struct_catalog"],
        "struct_query" => &["struct_query"],
        _ => &[],
    }
}

/// 必做动作是否已有对应的 Ok ToolResult（按 tool 名匹配，决策⑤格式）。
/// 未知原语（validate 之后不该出现）或空别名 → 未满足。
pub fn required_action_satisfied(action: &str, tool_results: &[ToolResult]) -> bool {
    let aliases = action_tool_aliases(action);
    if aliases.is_empty() {
        return false;
    }
    tool_results
        .iter()
        .any(|r| r.status == contracts::ToolStatus::Ok && aliases.contains(&r.tool.as_str()))
}

/// 系统提示（prompts/pipeline/query-card.system.md，第三人称事实化题型定义）。
const QUERY_CARD_SYSTEM_PROMPT: &str =
    include_str!("../../../../prompts/pipeline/query-card.system.md");

/// Parse-error 回贴修复提示（prompts/pipeline/query-card-repair.md，`{parse_error}` 占位）。
const QUERY_CARD_REPAIR_PROMPT: &str =
    include_str!("../../../../prompts/pipeline/query-card-repair.md");

/// Pre-loop 题型卡调用：一次 `json_mode`（deepseek 真下发 json_object），
/// 失败做一次「parse error 回贴」免费重试（heavytail/src/llm.rs:66-103 范式），
/// 再失败 → `None`（卡缺省 = 埋点不激活，优雅降级）。
///
/// 该调用不占迭代预算；usage 计入调用方 telemetry（ReActIterationRecord 通道）。
pub async fn fetch_query_card(
    llm: &LlmClient,
    mode: &ModeConfig,
    query: &str,
) -> Option<QueryCard> {
    let temperature = mode.temperature.unwrap_or(0.4);
    let messages = vec![
        ChatMessage::system(QUERY_CARD_SYSTEM_PROMPT),
        ChatMessage::user(query),
    ];
    let response = llm
        .complete_json_mode(&messages, Some(temperature))
        .await
        .ok()?;
    match parse_query_card(&response.content) {
        Some(card) => Some(card),
        None => {
            // 一次免费纠错轮：把 parse error 回贴给模型要求重发。
            let first_err = serde_json::from_str::<serde_json::Value>(
                &super::json_fence::strip_json_fence(&response.content),
            )
            .err()
            .map(|e| e.to_string())
            .unwrap_or_else(|| "response was not valid JSON".to_string());
            let repair_user = QUERY_CARD_REPAIR_PROMPT.replace("{parse_error}", &first_err);
            let repair_messages = vec![
                ChatMessage::system(QUERY_CARD_SYSTEM_PROMPT),
                ChatMessage::assistant(&response.content),
                ChatMessage::user(repair_user),
            ];
            let repaired = llm
                .complete_json_mode(&repair_messages, Some(temperature))
                .await
                .ok()?;
            parse_query_card(&repaired.content)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_mode() -> ModeConfig {
        super::super::config::load_mode_config("rag").unwrap()
    }

    #[test]
    fn parses_valid_card() {
        let raw = r#"{ "question_type": "calculation", "required_actions": ["calculator"] }"#;
        let card = parse_query_card(raw).expect("valid card");
        assert_eq!(card.question_type, QuestionType::Calculation);
        assert_eq!(card.required_actions, vec!["calculator".to_string()]);
    }

    #[test]
    fn unknown_type_falls_back_to_other() {
        let raw = r#"{ "question_type": "poetry_reading", "required_actions": [] }"#;
        let card = parse_query_card(raw).expect("lenient card");
        assert_eq!(card.question_type, QuestionType::Other);
    }

    #[test]
    fn missing_fields_default() {
        let raw = r#"{ "question_type": "table_count" }"#;
        let card = parse_query_card(raw).expect("defaulted card");
        assert_eq!(card.question_type, QuestionType::TableCount);
        assert!(card.required_actions.is_empty());
    }

    #[test]
    fn empty_raw_is_none() {
        assert!(parse_query_card("").is_none());
        assert!(parse_query_card("not json").is_none());
    }

    #[test]
    fn validates_against_registry_and_mounted() {
        let mut mode = base_mode();
        mode.sdk_primitives = super::super::sdk_primitives_for_caps(true, false)
            .iter()
            .map(|s| s.to_string())
            .collect();
        let card = QueryCard {
            question_type: QuestionType::RagFact,
            required_actions: vec![
                "dense".to_string(),
                "calculator".to_string(),
                "web".to_string(),
                "not_a_real_action".to_string(),
            ],
        };
        let cleaned = card.validate(&mode);
        // dense / calculator 均已挂载（BASE+RAG 组）；web 仅 SEARCH 组未挂载；
        // 未知动作丢弃。
        assert!(cleaned.required_actions.contains(&"dense".to_string()));
        assert!(cleaned.required_actions.contains(&"calculator".to_string()));
        assert!(!cleaned.required_actions.contains(&"web".to_string()));
        assert!(!cleaned.required_actions.contains(&"not_a_real_action".to_string()));
    }

    #[test]
    fn unmounted_actions_are_dropped_when_primitives_empty() {
        let mode = base_mode(); // load_mode_config → sdk_primitives 为空
        let card = QueryCard {
            question_type: QuestionType::Calculation,
            required_actions: vec!["calculator".to_string()],
        };
        assert!(card.validate(&mode).required_actions.is_empty());
    }

    #[test]
    fn required_action_satisfied_matches_by_tool_name() {
        let ok_calculator = ToolResult {
            tool: "calculator".into(),
            version: "1".into(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({"result": 42})),
            trace: None,
        };
        let ok_web = ToolResult {
            tool: "web_search".into(),
            version: "1".into(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({"results": []})),
            trace: None,
        };
        let err_calculator = ToolResult {
            tool: "calculator".into(),
            version: "1".into(),
            status: contracts::ToolStatus::Error,
            data: None,
            trace: None,
        };
        assert!(required_action_satisfied("calculator", &[ok_calculator.clone()]));
        assert!(required_action_satisfied("web", &[ok_web.clone()]));
        assert!(!required_action_satisfied("calculator", &[ok_web]));
        assert!(!required_action_satisfied("calculator", &[err_calculator]));
        assert!(!required_action_satisfied("calculator", &[]));
    }
}
