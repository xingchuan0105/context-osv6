use serde_json::{Value, json};

pub(crate) fn mcp_all_tools() -> Vec<Value> {
    let mut tools = Vec::new();
    tools.extend(org_tools());
    tools.extend(ingest_tools());
    tools.extend(query_tools());
    tools
}

pub(crate) fn mcp_workspace_query_tools() -> Vec<Value> {
    query_tools()
}

fn notebook_id_property() -> Value {
    json!({
        "type": "string",
        "description": "Workspace (notebook) UUID"
    })
}

fn org_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "org.create_workspace",
            "description": "Create a new workspace (notebook) in the current organization.",
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
            "name": "org.list_workspaces",
            "description": "List workspaces (notebooks) accessible in the current organization.",
            "inputSchema": {
                "type": "object",
                "properties": {}
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
                "required": ["notebook_id", "filename", "mime_type", "file_size"],
                "properties": {
                    "notebook_id": notebook_id_property(),
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
                "required": ["notebook_id", "document_id"],
                "properties": {
                    "notebook_id": notebook_id_property(),
                    "document_id": { "type": "string" }
                }
            }
        }),
        json!({
            "name": "workspace.document_status",
            "description": "Poll document ingest/index status.",
            "inputSchema": {
                "type": "object",
                "required": ["notebook_id", "document_id"],
                "properties": {
                    "notebook_id": notebook_id_property(),
                    "document_id": { "type": "string" }
                }
            }
        }),
        json!({
            "name": "workspace.add_url_source",
            "description": "Add a URL source to a workspace for crawling and indexing.",
            "inputSchema": {
                "type": "object",
                "required": ["notebook_id", "url"],
                "properties": {
                    "notebook_id": notebook_id_property(),
                    "url": { "type": "string" }
                }
            }
        }),
        json!({
            "name": "workspace.list_sources",
            "description": "List indexed sources in a workspace.",
            "inputSchema": {
                "type": "object",
                "required": ["notebook_id"],
                "properties": {
                    "notebook_id": notebook_id_property()
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
                "required": ["notebook_id", "query"],
                "properties": {
                    "notebook_id": notebook_id_property(),
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
                "required": ["notebook_id", "query"],
                "properties": {
                    "notebook_id": notebook_id_property(),
                    "query": { "type": "string" }
                }
            }
        }),
        json!({
            "name": "notebook.chat",
            "description": "Legacy alias for workspace.rag_query.",
            "inputSchema": {
                "type": "object",
                "required": ["notebook_id", "query"],
                "properties": {
                    "notebook_id": notebook_id_property(),
                    "query": { "type": "string" },
                    "agent_type": { "type": "string" },
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
        "workspace.rag_query" | "notebook.chat" => Some("rag"),
        "workspace.search_query" => Some("search"),
        "workspace.create_upload"
        | "workspace.complete_upload"
        | "workspace.document_status"
        | "workspace.add_url_source" => Some("index"),
        "workspace.list_sources" => Some("query"),
        "org.create_workspace" | "org.list_workspaces" => Some("workspace.create"),
        _ => None,
    }
}

pub(crate) fn success_result(
    tool: &str,
    notebook_id: Option<&str>,
    data: Value,
    next_steps: Vec<&str>,
) -> Value {
    let guide = operation_guide_mode_for_tool(tool)
        .and_then(app_chat::load_invoke_operation_guide)
        .and_then(|guide| serde_json::to_value(guide).ok());
    json!({
        "ok": true,
        "tool": tool,
        "notebook_id": notebook_id,
        "data": data,
        "agent_operation_guide": guide,
        "next_steps": next_steps,
    })
}
