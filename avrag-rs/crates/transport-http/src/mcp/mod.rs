mod catalog;
mod compat;
mod dispatch;
mod gateway;
mod jsonrpc;
mod tools;

pub(crate) use compat::{
    compat_mcp_jsonrpc_handler, compat_mcp_sse_handler, compat_mcp_tool_call_handler,
};
pub(crate) use gateway::{unified_mcp_jsonrpc_handler, unified_mcp_sse_handler};
pub(crate) use tools::expand_external_workspace_rag_scope;
