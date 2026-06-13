use contracts::{ToolResult, ToolSpec, ToolStatus};
use serde_json::Value;

use crate::agents::skills::{ExecutionContext, SkillComponent};

/// Load conversation history for targeted or full recall.
pub struct ConversationHistoryLoad;

#[async_trait::async_trait]
impl SkillComponent for ConversationHistoryLoad {
    fn id(&self) -> &str {
        "conversation_history_load"
    }

    fn version(&self) -> &str {
        "1.0"
    }

    fn description(&self) -> &str {
        "Load when the agent needs to recall previous messages from this session. \
         Use without tags for full history analysis. Use with tags for targeted recall."
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "conversation_history_load".to_string(),
            version: "1.0".to_string(),
            description: "Load previous messages from the current conversation session. \
                          Optionally filter by tags or limit the number of messages."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "tags": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Optional tags to filter messages by."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of messages to return. Defaults to 20.",
                        "default": 20
                    }
                }
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "tags": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Tags that were used for filtering."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of messages requested."
                    },
                    "message_count": {
                        "type": "integer",
                        "description": "Number of messages returned."
                    }
                }
            }),
        }
    }

    async fn execute<'a>(&self, args: &Value, ctx: &'a ExecutionContext<'a>) -> ToolResult {
        let (auth, session_id, repo) = match (ctx.auth, ctx.session_id, ctx.chat_persistence) {
            (Some(auth), Some(session_id), Some(repo)) => (auth, session_id, repo),
            _ => {
                return crate::agents::skills::memory_dispatch::memory_tool_error(
                    self.id(),
                    "conversation history requires auth, session_id, and pg repository",
                )
            }
        };
        crate::agents::skills::memory_dispatch::conversation_history_load(
            args, auth, session_id, repo,
        )
        .await
    }
}

/// Load the user's long-term profile and preferences on demand.
pub struct UserProfileLoad;

#[async_trait::async_trait]
impl SkillComponent for UserProfileLoad {
    fn id(&self) -> &str {
        "user_profile_load"
    }

    fn version(&self) -> &str {
        "1.0"
    }

    fn description(&self) -> &str {
        "Load when the agent needs the user's long-term profile, expertise, or stated preferences \
         beyond what runtime already injected. Skip when recent turns suffice."
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "user_profile_load".to_string(),
            version: "1.0".to_string(),
            description: "Load the authenticated user's structured profile and preference memory."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "structured_profile": { "type": "object" },
                    "expertise_domains": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "preferred_answer_style": { "type": ["string", "null"] },
                    "frequently_asked_topics": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                }
            }),
        }
    }

    async fn execute<'a>(&self, _args: &Value, ctx: &'a ExecutionContext<'a>) -> ToolResult {
        let (auth, repo) = match (ctx.auth, ctx.chat_persistence) {
            (Some(auth), Some(repo)) => (auth, repo),
            _ => {
                return crate::agents::skills::memory_dispatch::memory_tool_error(
                    self.id(),
                    "user profile load requires auth and pg repository",
                )
            }
        };
        crate::agents::skills::memory_dispatch::user_profile_load(auth, repo).await
    }
}

/// Tag conversation messages for future recall.
pub struct ConversationHistoryTag;

#[async_trait::async_trait]
impl SkillComponent for ConversationHistoryTag {
    fn id(&self) -> &str {
        "conversation_history_tag"
    }

    fn version(&self) -> &str {
        "1.0"
    }

    fn description(&self) -> &str {
        "Load when the agent needs to label messages with descriptive tags for future recall. \
         Every analyzed message should receive at least one specific, distinguishable tag."
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "conversation_history_tag".to_string(),
            version: "1.0".to_string(),
            description: "Label messages with descriptive tags for future targeted recall. \
                          Supports add, remove, and replace operations on message tags."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operations": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "message_id": {
                                    "type": "integer",
                                    "description": "ID of the message to tag."
                                },
                                "action": {
                                    "type": "string",
                                    "enum": ["add", "remove", "replace"],
                                    "description": "Tag operation to perform."
                                },
                                "tags": {
                                    "type": "array",
                                    "items": {"type": "string"},
                                    "description": "Tags to apply."
                                }
                            },
                            "required": ["message_id", "action", "tags"]
                        },
                        "description": "List of tagging operations to perform."
                    }
                },
                "required": ["operations"]
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation_count": {
                        "type": "integer",
                        "description": "Number of operations processed."
                    }
                }
            }),
        }
    }

    async fn execute<'a>(&self, args: &Value, _ctx: &'a ExecutionContext<'a>) -> ToolResult {
        let operations = args
            .get("operations")
            .and_then(|v| v.as_array())
            .map(|arr| arr.len())
            .unwrap_or(0);

        ToolResult {
            tool: self.id().to_string(),
            version: self.version().to_string(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({
                "operation_count": operations,
            })),
            trace: None,
        }
    }
}
