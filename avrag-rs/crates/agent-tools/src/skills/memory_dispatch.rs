//! Shared dispatch for on-demand memory tools (conversation history + user profile).

use app_core::domain_rows::{ConversationHistoryHit, ConversationHistoryScope};
use app_core::{MessagePort, ProfilePort};
use contracts::auth_runtime::AuthContext;
use contracts::{ToolResult, ToolStatus};
use serde_json::Value;
use uuid::Uuid;

use crate::MAX_PROMPT_HISTORY_TURNS;

pub async fn conversation_history_load(
    args: &Value,
    auth: &AuthContext,
    session_id: Uuid,
    repo: &dyn MessagePort,
) -> ToolResult {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let scope = parse_history_scope(args.get("scope"));
    let limit = args
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(20)
        .clamp(1, 50);

    let exclude_ids = collect_excluded_message_ids(repo, auth, session_id).await;

    match repo
        .search_conversation_history(auth, session_id, &query, scope, limit, &exclude_ids)
        .await
    {
        Ok(messages) => {
            let scope_label = scope_label(scope);
            let msg_json: Vec<Value> = messages.into_iter().map(history_hit_json).collect();
            ToolResult {
                tool: "conversation_history_load".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!({
                    "query": query,
                    "scope": scope_label,
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

fn parse_history_scope(value: Option<&Value>) -> ConversationHistoryScope {
    match value
        .and_then(|v| v.as_str())
        .map(str::trim)
        .map(str::to_lowercase)
    {
        Some(scope) if scope == "session" => ConversationHistoryScope::Session,
        _ => ConversationHistoryScope::Workspace,
    }
}

fn scope_label(scope: ConversationHistoryScope) -> &'static str {
    match scope {
        ConversationHistoryScope::Session => "session",
        ConversationHistoryScope::Workspace => "notebook",
    }
}

fn history_hit_json(hit: ConversationHistoryHit) -> Value {
    serde_json::json!({
        "message_id": hit.message_id,
        "session_id": hit.session_id.to_string(),
        "role": hit.role,
        "content": hit.content,
        "created_at": hit.created_at.to_rfc3339(),
    })
}

async fn collect_excluded_message_ids(
    repo: &dyn MessagePort,
    auth: &AuthContext,
    session_id: Uuid,
) -> Vec<i64> {
    match repo.list_messages(auth, session_id).await {
        Ok(messages) => {
            let mut ids: Vec<i64> = messages
                .into_iter()
                .filter(|m| m.role == "user")
                .map(|m| m.id)
                .collect();
            // Mirror runtime injection: current query + last MAX prior user turns.
            // If the in-flight user row is already persisted, drop the latest before taking priors.
            if ids.len() > MAX_PROMPT_HISTORY_TURNS + 1 {
                ids.pop();
            }
            if ids.len() > MAX_PROMPT_HISTORY_TURNS {
                ids = ids.split_off(ids.len() - MAX_PROMPT_HISTORY_TURNS);
            }
            ids
        }
        Err(_) => Vec::new(),
    }
}

pub async fn user_profile_load(auth: &AuthContext, repo: &dyn ProfilePort) -> ToolResult {
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
                "structured_profile": serde_json::json!({}),
                "expertise_domains": [],
                "preferred_answer_style": null,
                "frequently_asked_topics": [],
                "custom_preferences": serde_json::json!({}),
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
