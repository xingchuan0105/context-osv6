#![recursion_limit = "8192"]

mod auth_guard;
mod auth_types;
mod handlers;
mod lib_impl;
mod mcp;
mod middleware;
mod routes;

pub use lib_impl::build_router;
pub use lib_impl::{issue_jwt, issue_jwt_for_auth_version};
