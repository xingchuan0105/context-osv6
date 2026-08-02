/// Delegate to the single shared implementation in `rag-core`.
///
/// The doc-scope intersection logic (and the RAG dispatch path that enforces it)
/// lives in `avrag_rag_core::runtime::scoped_rag_dispatch`. This module keeps the
/// legacy `agent-tools` entry points so existing call sites (the `agent-loop`
/// re-export) keep working without duplicating the scope logic.
pub use avrag_rag_core::runtime::scoped_rag_dispatch::{force_doc_scope, intersect_doc_scope};
