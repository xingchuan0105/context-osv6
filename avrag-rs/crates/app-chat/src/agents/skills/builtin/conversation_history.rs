use contracts::{ToolResult, ToolSpec};
use serde_json::Value;

use crate::agents::skills::{ExecutionContext, SkillComponent};

/// Load conversation history via recency + FTS hybrid search.
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
        "Load when the agent needs to recall previous user messages beyond runtime-injected recent turns. \
         Searches by query (jieba-segmented FTS) merged with recency; default scope is notebook (cross-session)."
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "conversation_history_load".to_string(),
            version: "1.0".to_string(),
            description: "Search prior user messages in the current notebook (or session-only). \
                          Combines recent messages with full-text search on segmented query tokens."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Keywords or phrases to match in prior user messages. Empty returns recent messages only."
                    },
                    "scope": {
                        "type": "string",
                        "enum": ["notebook", "session"],
                        "description": "Search within the current notebook (default) or current session only.",
                        "default": "notebook"
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
                    "query": { "type": "string" },
                    "scope": { "type": "string" },
                    "limit": { "type": "integer" },
                    "message_count": { "type": "integer" },
                    "messages": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "message_id": { "type": "integer" },
                                "session_id": { "type": "string" },
                                "role": { "type": "string" },
                                "content": { "type": "string" },
                                "created_at": { "type": "string" }
                            }
                        }
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
