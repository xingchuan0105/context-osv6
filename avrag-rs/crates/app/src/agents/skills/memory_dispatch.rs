//! Shared dispatch for on-demand memory tools (conversation history + user profile).

use avrag_auth::AuthContext;
use avrag_storage_pg::PgAppRepository;
use common::{ToolResult, ToolStatus};
use serde_json::Value;
use uuid::Uuid;

pub async fn conversation_history_load(
    args: &Value,
    auth: &AuthContext,
    session_id: Uuid,
    repo: &PgAppRepository,
) -> ToolResult {
    let tags: Option<Vec<String>> = args
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });
    let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(20);

    match repo
        .load_history_by_tags(auth, session_id, tags.clone(), limit)
        .await
    {
        Ok(messages) => {
            let msg_json: Vec<Value> = messages
                .into_iter()
                .map(|m| {
                    serde_json::json!({
                        "message_id": m.message_id,
                        "role": m.role,
                        "content": m.content,
                        "tags": m.tags,
                        "created_at": m.created_at.to_rfc3339(),
                    })
                })
                .collect();
            ToolResult {
                tool: "conversation_history_load".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!({
                    "tags": tags.unwrap_or_default(),
                    "limit": limit,
                    "message_count": msg_json.len(),
                    "messages": msg_json,
                })),
                trace: None,
            }
        }
        Err(e) => ToolResult {
            tool: "conversation_history_load".to_string(),
            version: "1.0".to_string(),
            status: ToolStatus::Error,
            data: Some(serde_json::json!({ "error": e.to_string() })),
            trace: None,
        },
    }
}

pub async fn user_profile_load(auth: &AuthContext, repo: &PgAppRepository) -> ToolResult {
    let tool = "user_profile_load".to_string();
    let version = "1.0".to_string();

    let Some(user_id) = auth.actor_id().map(|actor| actor.into_uuid()) else {
        return ToolResult {
            tool,
            version,
            status: ToolStatus::Error,
            data: Some(serde_json::json!({ "error": "authenticated user required" })),
            trace: None,
        };
    };

    match repo.get_user_profile(auth, user_id).await {
        Ok(Some(profile)) => ToolResult {
            tool,
            version,
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({
                "structured_profile": profile.structured_profile,
                "expertise_domains": profile.expertise_domains,
                "preferred_answer_style": profile.preferred_answer_style,
                "frequently_asked_topics": profile.frequently_asked_topics,
                "custom_preferences": profile.custom_preferences,
            })),
            trace: None,
        },
        Ok(None) => ToolResult {
            tool,
            version,
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({
                "structured_profile": {},
                "expertise_domains": [],
                "preferred_answer_style": null,
                "frequently_asked_topics": [],
                "custom_preferences": {},
            })),
            trace: None,
        },
        Err(e) => ToolResult {
            tool,
            version,
            status: ToolStatus::Error,
            data: Some(serde_json::json!({ "error": e.to_string() })),
            trace: None,
        },
    }
}

pub fn memory_tool_error(tool: &str, message: &str) -> ToolResult {
    ToolResult {
        tool: tool.to_string(),
        version: "1.0".to_string(),
        status: ToolStatus::Error,
        data: Some(serde_json::json!({ "error": message })),
        trace: None,
    }
}
