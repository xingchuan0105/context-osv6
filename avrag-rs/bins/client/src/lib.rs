//! Shared client for local Context-OS HTTP MCP / product API.
//!
//! Used by:
//! - `context-os-mcp` (stdio → HTTP MCP proxy)
//! - `context-os` (thin CLI: status / ingest / ask / sources)

pub mod config;
pub mod discover;
pub mod mcp_client;
pub mod mime;
pub mod proxy;
pub mod token_store;
