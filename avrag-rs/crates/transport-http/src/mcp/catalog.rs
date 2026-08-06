use serde_json::{Value, json};

pub(crate) fn mcp_all_tools() -> Vec<Value> {
    let mut tools = Vec::new();
    tools.extend(account_tools());
    tools.extend(ingest_tools());
    tools.extend(query_tools());
    tools.extend(share_tools());
    tools
}

pub(crate) fn mcp_workspace_query_tools() -> Vec<Value> {
    query_tools()
}

fn workspace_id_property() -> Value {
    json!({
        "type": "string",
        "description": "Workspace (notebook) UUID"
    })
}

fn account_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "account.create_workspace",
            "description": "Create a new workspace under the current personal account.",
            "inputSchema": {
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": { "type": "string" },
                    "description": { "type": "string" }
                }
            }
        }),
        json!({
            "name": "account.list_workspaces",
            "description": "List workspaces accessible under the current personal account.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "account.share_quota",
            "description": "Owner share-enabled workspace quota (used/max/plan). User session only; not available to workspace API keys.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
    ]
}

fn share_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "workspace.share_create_link",
            "description": "Create a share link and enable sharing for a workspace (consumes owner plan share slot per ADR-0010). User session only.",
            "inputSchema": {
                "type": "object",
                "required": ["workspace_id"],
                "properties": {
                    "workspace_id": workspace_id_property(),
                    "role": {
                        "type": "string",
                        "description": "viewer|editor|owner (default viewer)"
                    },
                    "expires_in_secs": { "type": "integer", "minimum": 1 },
                    "expires_at": {
                        "type": "string",
                        "description": "RFC3339 expiry; alternative to expires_in_secs"
                    }
                }
            }
        }),
        json!({
            "name": "workspace.share_get_settings",
            "description": "Get share settings, tokens, and members for a workspace. User session only.",
            "inputSchema": {
                "type": "object",
                "required": ["workspace_id"],
                "properties": {
                    "workspace_id": workspace_id_property()
                }
            }
        }),
        json!({
            "name": "workspace.share_update_settings",
            "description": "Update access_level (private|link|public), allow_download, and daily question limits. Enabling link/public uses share quota. User session only.",
            "inputSchema": {
                "type": "object",
                "required": ["workspace_id"],
                "properties": {
                    "workspace_id": workspace_id_property(),
                    "access_level": {
                        "type": "string",
                        "description": "private|link|public"
                    },
                    "allow_download": { "type": "boolean" },
                    "anon_question_limit": {
                        "type": "integer",
                        "description": "Daily anon visitor question cap; 0 = unlimited"
                    },
                    "member_question_limit": {
                        "description": "Daily registered visitor cap; null clears to unlimited"
                    }
                }
            }
        }),
        json!({
            "name": "workspace.share_revoke_link",
            "description": "Revoke a share token. User session only.",
            "inputSchema": {
                "type": "object",
                "required": ["workspace_id", "token"],
                "properties": {
                    "workspace_id": workspace_id_property(),
                    "token": { "type": "string" }
                }
            }
        }),
        json!({
            "name": "workspace.share_invite_member",
            "description": "Invite a member by email (viewer|editor|owner). User session only.",
            "inputSchema": {
                "type": "object",
                "required": ["workspace_id", "email"],
                "properties": {
                    "workspace_id": workspace_id_property(),
                    "email": { "type": "string" },
                    "role": {
                        "type": "string",
                        "description": "viewer|editor|owner (default viewer)"
                    }
                }
            }
        }),
    ]
}

fn ingest_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "workspace.create_upload",
            "description": "Start a file upload; PUT bytes to returned upload_url, then complete_upload.",
            "inputSchema": {
                "type": "object",
                "required": ["workspace_id", "filename", "mime_type", "file_size"],
                "properties": {
                    "workspace_id": workspace_id_property(),
                    "filename": { "type": "string" },
                    "mime_type": { "type": "string" },
                    "file_size": { "type": "integer", "minimum": 1 }
                }
            }
        }),
        json!({
            "name": "workspace.complete_upload",
            "description": "Finalize a file upload after PUT to upload_url.",
            "inputSchema": {
                "type": "object",
                "required": ["workspace_id", "document_id"],
                "properties": {
                    "workspace_id": workspace_id_property(),
                    "document_id": { "type": "string" }
                }
            }
        }),
        json!({
            "name": "workspace.document_status",
            "description": "Poll document ingest/index status.",
            "inputSchema": {
                "type": "object",
                "required": ["workspace_id", "document_id"],
                "properties": {
                    "workspace_id": workspace_id_property(),
                    "document_id": { "type": "string" }
                }
            }
        }),
        json!({
            "name": "workspace.add_url_source",
            "description": "Add a URL source to a workspace for crawling and indexing.",
            "inputSchema": {
                "type": "object",
                "required": ["workspace_id", "url"],
                "properties": {
                    "workspace_id": workspace_id_property(),
                    "url": { "type": "string" }
                }
            }
        }),
        json!({
            "name": "workspace.list_sources",
            "description": "List indexed sources in a workspace.",
            "inputSchema": {
                "type": "object",
                "required": ["workspace_id"],
                "properties": {
                    "workspace_id": workspace_id_property()
                }
            }
        }),
    ]
}

fn query_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "workspace.rag_query",
            "description": "Run a notebook-scoped RAG query over indexed sources (codegen/SDK).",
            "inputSchema": {
                "type": "object",
                "required": ["workspace_id", "query"],
                "properties": {
                    "workspace_id": workspace_id_property(),
                    "query": { "type": "string" },
                    "doc_scope": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                }
            }
        }),
        json!({
            "name": "workspace.search_query",
            "description": "Run a notebook-scoped web search agent (native web_search tools).",
            "inputSchema": {
                "type": "object",
                "required": ["workspace_id", "query"],
                "properties": {
                    "workspace_id": workspace_id_property(),
                    "query": { "type": "string" }
                }
            }
        }),
        json!({
            "name": "workspace.chat",
            "description": "Legacy alias for workspace.rag_query. Prefer agent_type rag|search|chat (or REST capabilities[]). agent_type=write is rejected (write_mode_disabled).",
            "inputSchema": {
                "type": "object",
                "required": ["workspace_id", "query"],
                "properties": {
                    "workspace_id": workspace_id_property(),
                    "query": { "type": "string" },
                    "agent_type": {
                        "type": "string",
                        "description": "Legacy mode: rag|search|chat. write is product-offline (write_mode_disabled)."
                    },
                    "doc_scope": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                }
            }
        }),
    ]
}

pub(crate) fn operation_guide_mode_for_tool(tool_name: &str) -> Option<&'static str> {
    match tool_name {
        "workspace.rag_query" | "workspace.chat" => Some("rag"),
        "workspace.search_query" => Some("search"),
        "workspace.create_upload"
        | "workspace.complete_upload"
        | "workspace.document_status"
        | "workspace.add_url_source" => Some("index"),
        "workspace.list_sources" => Some("query"),
        "account.create_workspace" | "account.list_workspaces" => Some("workspace.create"),
        "workspace.share_create_link"
        | "workspace.share_get_settings"
        | "workspace.share_update_settings"
        | "workspace.share_revoke_link"
        | "workspace.share_invite_member"
        | "account.share_quota" => Some("workspace.create"),
        _ => None,
    }
}

pub(crate) fn success_result(
    tool: &str,
    workspace_id: Option<&str>,
    data: Value,
    next_steps: Vec<&str>,
) -> Value {
    let guide = operation_guide_mode_for_tool(tool)
        .and_then(app_chat::load_invoke_operation_guide)
        .and_then(|guide| serde_json::to_value(guide).ok());
    json!({
        "ok": true,
        "tool": tool,
        "workspace_id": workspace_id,
        "data": data,
        "agent_operation_guide": guide,
        "next_steps": next_steps,
    })
}
