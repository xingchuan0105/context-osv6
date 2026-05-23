//! E2E integration tests for v5 state machine + progressive disclosure.
//!
//! These tests require a staging environment with real LLM, vector DB, and web search.
//! Run with: cargo test --ignored -p app --test e2e

#[path = "e2e/config.rs"]
pub mod config;
#[path = "e2e/recording_llm.rs"]
pub mod recording_llm;
#[path = "e2e/assertions.rs"]
pub mod assertions;

#[allow(unused_imports)]
pub use config::E2EConfig;
#[allow(unused_imports)]
pub use recording_llm::{LlmCall, RecordingLlmProvider};
