//! Feature-gated evaluation framework (outside main agent tree).
#![cfg(feature = "eval")]

pub mod framework;

pub use framework::*;
