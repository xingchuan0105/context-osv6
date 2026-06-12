// rustc 1.94.0: suppress lints that ICE Cargo JSON annotate_snippets.
#![allow(dead_code)]
#![allow(deprecated)]
#![allow(unused_mut)]

pub use app_chat::{
    agents, chat_streaming, memory_helpers, rag_prompts, AgentKind, ChatContext, LlmContext,
    OrchestratorContext,
};
mod chat;
pub mod adapters;
pub mod ports;
pub mod runtime;
pub mod services;

pub use app_chat::token_budget;

pub use app_core::{
    analytics_context::AnalyticsContext, analytics_context::AnalyticsServiceCtx,
    analytics_context::CostEventRecord, config::*, load_prompt_template, domain_ports::*,
    MemoryState, RetrievedContext, StoredDocument, StorageContext,
};

pub mod storage_context;
pub mod lib_impl;
pub use lib_impl::*;
