//! RAG runtime tools wrapped as AgentTool implementations.
//!
//! These adapters bridge `rag-core::RagRuntime` tools into the
//! `AgentToolRegistry` so that the generic `AgentLoop` can dispatch them.

use crate::agents::tool_registry::AgentTool;
use common::{ToolResult, ToolSpec, ToolStatus};

/// Wrapper that exposes RAG runtime tools through the AgentTool interface.
pub struct RagRuntimeTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

impl RagRuntimeTool {
    pub fn dense_retrieval() -> Self {
        Self {
            name: "dense_retrieval".to_string(),
            description: "Semantic search over document chunks using dense embeddings. \
                          Returns the most relevant chunks ranked by vector similarity."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "queries": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "List of semantic search queries"
                    },
                    "modality": {
                        "type": "string",
                        "enum": ["text", "mm", "both"],
                        "default": "text"
                    },
                    "top_k": {
                        "type": "integer",
                        "default": 10,
                        "description": "Number of results per query"
                    }
                },
                "required": ["queries"]
            }),
        }
    }

    pub fn lexical_retrieval() -> Self {
        Self {
            name: "lexical_retrieval".to_string(),
            description: "Keyword/BM25 search over document chunks. \
                          Returns chunks matching the provided terms."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "terms": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "List of keyword terms to search for"
                    },
                    "top_k": {
                        "type": "integer",
                        "default": 10,
                        "description": "Number of results"
                    }
                },
                "required": ["terms"]
            }),
        }
    }

    pub fn graph_retrieval() -> Self {
        Self {
            name: "graph_retrieval".to_string(),
            description: "Traverse the knowledge graph to find related entities and documents. \
                          Uses graph hints and placeholder triplets to guide traversal."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "graph_hints": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "subject": {"type": "string"},
                                "predicate": {"type": "string"},
                                "object": {"type": "string"}
                            }
                        }
                    },
                    "placeholder_triplets": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "subject": {"type": "string"},
                                "predicate": {"type": "string"},
                                "object": {"type": "string"}
                            }
                        }
                    },
                    "relation_limit": {"type": "integer", "default": 20},
                    "supporting_chunk_limit": {"type": "integer", "default": 10},
                    "hop_limit": {"type": "integer", "default": 1},
                    "fan_out_limit": {"type": "integer", "default": 10}
                }
            }),
        }
    }

    pub fn doc_summary() -> Self {
        Self {
            name: "doc_summary".to_string(),
            description: "Generate a summary of one or more documents. \
                          Level 'doc' summarizes the whole document; 'section' summarizes relevant sections."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "doc_ids": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Document IDs to summarize"
                    },
                    "level": {
                        "type": "string",
                        "enum": ["doc", "section"],
                        "default": "doc"
                    }
                },
                "required": ["doc_ids"]
            }),
        }
    }

    pub fn index_lookup() -> Self {
        Self {
            name: "index_lookup".to_string(),
            description: "Look up specific chunks by their IDs within a document. \
                          Use when the exact chunk IDs are known."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "doc_id": {"type": "string"},
                    "chunk_ids": {
                        "type": "array",
                        "items": {"type": "string"}
                    }
                },
                "required": ["doc_id", "chunk_ids"]
            }),
        }
    }

    pub fn doc_metadata() -> Self {
        Self {
            name: "doc_metadata".to_string(),
            description: "Retrieve metadata fields for specified documents."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "doc_ids": {
                        "type": "array",
                        "items": {"type": "string"}
                    },
                    "fields": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Specific metadata fields to retrieve"
                    }
                },
                "required": ["doc_ids"]
            }),
        }
    }
}

#[async_trait::async_trait]
impl AgentTool for RagRuntimeTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name.clone(),
            version: "1.0".to_string(),
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
            output_schema: serde_json::json!({"type": "object"}),
        }
    }

    async fn execute(
        &self,
        _args: serde_json::Value,
    ) -> anyhow::Result<ToolResult> {
        // Phase C stub: actual execution requires RagRuntime reference.
        // Full implementation will dispatch to rag_runtime.execute_tools().
        Ok(ToolResult {
            tool: self.name.clone(),
            version: "1.0".to_string(),
            status: ToolStatus::NotImplemented,
            data: Some(serde_json::json!({
                "status": "stub",
                "reason": "RagRuntimeTool requires runtime context — not yet wired"
            })),
            trace: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dense_retrieval_spec_is_valid() {
        let tool = RagRuntimeTool::dense_retrieval();
        let spec = tool.spec();
        assert_eq!(spec.name, "dense_retrieval");
        assert!(!spec.description.is_empty());
        assert!(spec.input_schema.get("properties").is_some());
    }

    #[test]
    fn all_rag_tools_have_unique_names() {
        let tools = vec![
            RagRuntimeTool::dense_retrieval(),
            RagRuntimeTool::lexical_retrieval(),
            RagRuntimeTool::graph_retrieval(),
            RagRuntimeTool::doc_summary(),
            RagRuntimeTool::index_lookup(),
            RagRuntimeTool::doc_metadata(),
        ];
        let names: Vec<String> = tools.iter().map(|t| t.spec().name).collect();
        let unique: std::collections::HashSet<String> = names.iter().cloned().collect();
        assert_eq!(names.len(), unique.len());
    }

    #[tokio::test]
    async fn stub_execution_returns_not_implemented() {
        let tool = RagRuntimeTool::dense_retrieval();
        let result = tool.execute(serde_json::json!({"queries": ["test"]}))
            .await
            .unwrap();
        assert_eq!(result.status, ToolStatus::NotImplemented);
    }
}
