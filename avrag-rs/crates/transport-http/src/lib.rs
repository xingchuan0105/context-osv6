#![recursion_limit = "8192"]

mod auth_guard;
mod auth_types;
mod handlers;
mod middleware;
mod mcp;
mod routes;

include!("lib_impl.rs");
