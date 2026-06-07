use common::{ToolResult, ToolSpec, ToolStatus};
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

    async fn execute<'a>(&self, args: &Value, _ctx: &'a ExecutionContext<'a>) -> ToolResult {
        let tags: Vec<String> = args
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

        ToolResult {
            tool: self.id().to_string(),
            version: self.version().to_string(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({
                "tags": tags,
                "limit": limit,
                "message_count": 0,
            })),
            trace: None,
        }
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
