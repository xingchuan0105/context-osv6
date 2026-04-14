#![recursion_limit = "8192"]

mod auth_types;
mod handlers;
mod middleware;
mod routes;

include!("lib_impl.rs");
