#![recursion_limit = "8192"]

mod auth_guard;
mod auth_types;
mod handlers;
mod mcp;
mod middleware;
mod routes;

include!("lib_impl.rs");
