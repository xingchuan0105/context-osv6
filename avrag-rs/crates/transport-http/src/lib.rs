#![recursion_limit = "8192"]

mod auth_guard;
mod auth_types;
mod handlers;
mod lib_impl;
mod mcp;
mod middleware;
mod notification_emit;
mod routes;
mod sse_order;
mod turnstile;

pub use sse_order::{SseEventOrderTracker, validate_chat_sse_event_order};

pub use lib_impl::build_router;
pub use lib_impl::{issue_jwt, issue_jwt_for_auth_version};
pub use routes::relay::{RelayService, RelayUpstream, build_relay_router};
