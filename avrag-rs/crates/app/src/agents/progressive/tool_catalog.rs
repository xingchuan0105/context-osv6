use std::sync::OnceLock;

use super::Tool;

static RAG_TOOL_CATALOG: OnceLock<Vec<Tool>> = OnceLock::new();
static ATOMIC_TOOL_CATALOG: OnceLock<Vec<Tool>> = OnceLock::new();
static SEARCH_SPECIFIC_CATALOG: OnceLock<Vec<Tool>> = OnceLock::new();

// ============================================================================
// RAG-specific tools
// ============================================================================

/// Build the complete RAG tool catalog as progressively-disclosable [`Tool`]s.
///
/// Each [`Tool`] wraps a [`common::ToolSpec`] with a JSON Schema derived from
/// the strongly-typed args structs in `common::tool_call`.  The render output
/// is injected into the Plan phase so the LLM sees only the tool catalog
/// (not the full planner prompt), achieving true progressive disclosure.
///
/// For hot paths prefer [`rag_tool_catalog_cached`] which returns a
/// lazily-initialised `&'static Vec<Tool>`.
pub fn rag_tool_catalog() -> Vec<Tool> {
    vec![
        Tool::new(common::ToolSpec {
            name: "dense_retrieval".to_string(),
            version: "1.0".to_string(),
            description: concat!(
                "Semantic vector retrieval (text + multimodal fusion). ",
                "When to use: meaning-based recall, paraphrases, conceptual questions, ",
                "policy questions, task-oriented questions, rewritten standalone requests, ",
                "or any request where relevant wording may differ from the source text.\n",
                "Rules:\n",
                "- Write each query as a standalone retrieval query that includes the resolved target, core intent, and necessary constraints.\n",
                "- Do not stuff queries with keyword lists.\n",
                "- Prefer fewer high-signal queries over exhaustive coverage."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "queries": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "One or more standalone semantic queries."
                    },
                    "modality": {
                        "type": "string",
                        "enum": ["text", "mm", "both"],
                        "default": "text",
                        "description": "Retrieval modality."
                    },
                    "top_k": {
                        "type": "integer",
                        "default": 10,
                        "description": "Number of top results to retrieve."
                    }
                },
                "required": ["queries"]
            }),
            output_schema: serde_json::json!({
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "chunk_id": {"type": "string", "description": "Unique chunk identifier."},
                        "doc_id": {"type": "string", "description": "Parent document identifier."},
                        "text": {"type": "string", "description": "Retrieved text content."},
                        "score": {"type": "number", "description": "Relevance score (higher is better)."},
                        "page": {"type": "integer", "description": "Page number in the source document."},
                        "source": {"type": "string", "description": "Tool that produced this chunk."}
                    }
                }
            }),
        }).with_gotchas(vec![
            "Empty queries array returns no results — always provide at least one query.".to_string(),
            "Each query must be a standalone sentence, not a keyword list.".to_string(),
            "top_k defaults to 10; values above 50 may degrade latency without improving recall.".to_string(),
        ]),
        Tool::new(common::ToolSpec {
            name: "lexical_retrieval".to_string(),
            version: "1.0".to_string(),
            description: concat!(
                "BM25 exact lexical retrieval. ",
                "When to use: exact string matching is important — filenames, document titles, IDs, ",
                "codes, ticket numbers, version strings, acronyms, exact product names, exact API names, ",
                "rare terms, or literals likely to appear verbatim in text.\n",
                "Rules:\n",
                "- Keep terms compact and literal.\n",
                "- Do not use as a weaker duplicate of a semantic query.\n",
                "- Do not add unless exact lexical anchoring is likely to improve recall."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "terms": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Exact strings to match verbatim."
                    },
                    "top_k": {
                        "type": "integer",
                        "default": 10,
                        "description": "Number of top results to retrieve."
                    }
                },
                "required": ["terms"]
            }),
            output_schema: serde_json::json!({
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "chunk_id": {"type": "string", "description": "Unique chunk identifier."},
                        "doc_id": {"type": "string", "description": "Parent document identifier."},
                        "text": {"type": "string", "description": "Retrieved text content."},
                        "score": {"type": "number", "description": "Relevance score (higher is better)."},
                        "page": {"type": "integer", "description": "Page number in the source document."},
                        "source": {"type": "string", "description": "Tool that produced this chunk."}
                    }
                }
            }),
        }).with_gotchas(vec![
            "Empty terms array returns no results — always provide at least one term.".to_string(),
            "Terms are matched verbatim; typos or variations return no matches.".to_string(),
            "Do not use as a weaker duplicate of dense_retrieval for the same intent.".to_string(),
        ]),
        Tool::new(common::ToolSpec {
            name: "graph_retrieval".to_string(),
            version: "1.0".to_string(),
            description: concat!(
                "Knowledge-graph relation retrieval. ",
                "When to use: relations between entities, comparisons, ownership, dependency, ",
                "lineage, authorship, responsibility, connection paths, cause/effect across entities, ",
                "or likely multi-hop retrieval.\n",
                "Rules:\n",
                "- Use only when relationship retrieval can materially help connect missing links that chunk retrieval alone may miss.\n",
                "- Prefer triplets with one unknown placeholder when possible.\n",
                "- Use '?' or named placeholders such as '?owner', '?service', '?document' for unknown positions.\n",
                "- Keep graph hints sparse and semantically meaningful.\n",
                "- Limit parameters (relation_limit, hop_limit, fan_out_limit, supporting_chunk_limit) have sensible defaults; only override when you have a specific reason."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "graph_hints": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "subject": { "type": "string" },
                                "predicate": { "type": "string" },
                                "object": { "type": "string" }
                            }
                        },
                        "default": [],
                        "description": "Hints for graph traversal."
                    },
                    "placeholder_triplets": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "subject": { "type": "string" },
                                "predicate": { "type": "string" },
                                "object": { "type": "string" }
                            },
                            "required": ["subject", "predicate", "object"]
                        },
                        "default": [],
                        "description": "Triplets with placeholders for unknown entities."
                    },
                    "relation_limit": {
                        "type": "integer",
                        "default": 20,
                        "description": "Total relations across all hops."
                    },
                    "supporting_chunk_limit": {
                        "type": "integer",
                        "default": 10,
                        "description": "Chunks to retrieve per relation."
                    },
                    "hop_limit": {
                        "type": "integer",
                        "default": 1,
                        "description": "Max graph hops (max 3)."
                    },
                    "fan_out_limit": {
                        "type": "integer",
                        "default": 10,
                        "description": "Max fan-out per hop (max 20)."
                    },
                    "query": {
                        "type": "string",
                        "description": "Optional original query for reranking relation paths."
                    }
                },
                "required": ["graph_hints"]
            }),
            output_schema: serde_json::json!({
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "chunk_id": {"type": "string", "description": "Unique chunk identifier."},
                        "doc_id": {"type": "string", "description": "Parent document identifier."},
                        "text": {"type": "string", "description": "Retrieved text content."},
                        "score": {"type": "number", "description": "Relevance score (higher is better)."},
                        "page": {"type": "integer", "description": "Page number in the source document."},
                        "source": {"type": "string", "description": "Tool that produced this chunk."}
                    }
                }
            }),
        }).with_gotchas(vec![
            "Empty graph_hints and placeholder_triplets return random relations — always provide at least one hint.".to_string(),
            "Overly broad hints (e.g. subject: 'company') may return too many relations and timeout.".to_string(),
            "hop_limit defaults to 1; values above 2 significantly increase latency.".to_string(),
        ]),
        Tool::new(common::ToolSpec {
            name: "doc_index".to_string(),
            version: "1.0".to_string(),
            description: concat!(
                "Read the LLM-generated document index (chapter list with chunk IDs). ",
                "When to use: you need to understand document structure, chapter hierarchy, ",
                "and which chunks belong to which sections before targeted retrieval.\n",
                "Rules:\n",
                "- Call this before index_lookup to know which chunk IDs to request.\n",
                "- Each entry contains title, level, abstract, and the exact chunk_ids for that section."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "doc_ids": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Document UUIDs to read the index for."
                    }
                },
                "required": ["doc_ids"]
            }),
            output_schema: serde_json::json!({
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "doc_id": {"type": "string", "description": "Document identifier."},
                        "index": {
                            "type": "array",
                            "description": "List of sections with chunk IDs.",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "title": {"type": "string", "description": "Section title."},
                                    "level": {"type": "integer", "description": "Heading level (1 = top)."},
                                    "chunk_ids": {"type": "array", "items": {"type": "string"}, "description": "Chunk IDs belonging to this section."}
                                }
                            }
                        }
                    }
                }
            }),
        }).with_gotchas(vec![
            "Must be called before index_lookup for the same document to obtain valid chunk IDs.".to_string(),
            "Empty doc_ids returns an empty index.".to_string(),
            "The index is LLM-generated; chunk IDs may be stale if the document was re-indexed.".to_string(),
        ]),
        Tool::new(common::ToolSpec {
            name: "index_lookup".to_string(),
            version: "1.0".to_string(),
            description: concat!(
                "Direct chunk ID lookup for section-level precision reading. ",
                "When to use: you already know the exact chunk IDs from doc_index and want ",
                "to fetch the corresponding text blocks.\n",
                "Rules:\n",
                "- Only use chunk IDs obtained from doc_index.\n",
                "- If unsure about chunk IDs, use dense_retrieval instead."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "doc_id": {
                        "type": "string",
                        "description": "Target document UUID."
                    },
                    "chunk_ids": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Exact chunk UUIDs to fetch (from doc_index)."
                    }
                },
                "required": ["doc_id", "chunk_ids"]
            }),
            output_schema: serde_json::json!({
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "chunk_id": {"type": "string", "description": "Unique chunk identifier."},
                        "doc_id": {"type": "string", "description": "Parent document identifier."},
                        "text": {"type": "string", "description": "Retrieved text content."},
                        "score": {"type": "number", "description": "Relevance score (higher is better)."},
                        "page": {"type": "integer", "description": "Page number in the source document."},
                        "source": {"type": "string", "description": "Tool that produced this chunk."}
                    }
                }
            }),
        }).with_gotchas(vec![
            "chunk_ids must come from doc_index; invented IDs return empty results.".to_string(),
            "Invalid doc_id format returns an error.".to_string(),
            "Always call doc_index first to obtain valid chunk_ids for the target document.".to_string(),
        ]),
        Tool::new(common::ToolSpec {
            name: "doc_summary".to_string(),
            version: "1.0".to_string(),
            description: concat!(
                "Read pre-generated document summaries. ",
                "When to use: broad document-level understanding, 'what does this document cover', ",
                "or to disambiguate where the answer likely lives before chunk recall."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "doc_ids": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Document UUIDs to read summaries for."
                    },
                    "level": {
                        "type": "string",
                        "enum": ["doc", "section"],
                        "default": "doc",
                        "description": "'doc' for full-document summary, 'section' for section-level TOC entries."
                    }
                },
                "required": ["doc_ids"]
            }),
            output_schema: serde_json::json!({
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "doc_id": {"type": "string"},
                        "level": {"type": "string", "enum": ["doc", "section"], "description": "Summary granularity."},
                        "summary": {"type": "string", "description": "Full-document summary (when level='doc')."},
                        "section_title": {"type": "string", "description": "Section title (when level='section')."},
                        "heading_level": {"type": "integer", "description": "Heading level (when level='section')."},
                        "page": {"type": "integer", "description": "Page number (when level='section')."}
                    }
                }
            }),
        }).with_gotchas(vec![
            "Summary is pre-generated and may not reflect the latest document version.".to_string(),
            "Empty doc_ids returns empty summaries.".to_string(),
            "level='section' returns TOC entries, not full text summaries.".to_string(),
        ]),
        Tool::new(common::ToolSpec {
            name: "doc_metadata".to_string(),
            version: "1.0".to_string(),
            description: concat!(
                "Read document metadata (name, mime_type, file_size, status, chunk_count). ",
                "When to use: you need basic file info or the user asks meta questions ",
                "about the document itself."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "doc_ids": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Document UUIDs to read metadata for."
                    },
                    "fields": {
                        "type": "array",
                        "items": { "type": "string" },
                        "default": [],
                        "description": "Optional filter, e.g. ['name', 'mime_type']. Omit for all fields."
                    }
                },
                "required": ["doc_ids"]
            }),
            output_schema: serde_json::json!({
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "doc_id": {"type": "string"},
                        "name": {"type": "string", "description": "File name."},
                        "mime_type": {"type": "string", "description": "File MIME type."},
                        "file_size": {"type": "integer", "description": "File size in bytes."},
                        "status": {"type": "string", "description": "Processing status."},
                        "chunk_count": {"type": "integer", "description": "Number of chunks."},
                        "toc": {
                            "type": "array",
                            "description": "Table of contents entries.",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "title": {"type": "string"},
                                    "heading_level": {"type": "integer"},
                                    "page": {"type": "integer"},
                                    "rank": {"type": "integer"}
                                }
                            }
                        }
                    }
                }
            }),
        }).with_gotchas(vec![
            "Only reads metadata, not document content. Use dense_retrieval or index_lookup for content.".to_string(),
            "Empty doc_ids returns empty metadata.".to_string(),
            "fields filter restricts which keys are returned; omit fields for complete metadata.".to_string(),
        ]),
    ]
}

/// Return a lazily-initialised global singleton of the RAG tool catalog.
///
/// Prefer this over [`rag_tool_catalog()`] in hot paths (e.g. inside the
/// ReAct loop) to avoid reconstructing six `Tool` instances on every
/// iteration.
pub fn rag_tool_catalog_cached() -> &'static [Tool] {
    RAG_TOOL_CATALOG.get_or_init(rag_tool_catalog).as_slice()
}

// ============================================================================
// Atomic tools (all modes)
// ============================================================================

/// Build the universal atomic tool catalog shared across all agent modes.
///
/// These tools are disclosed in the Plan and Execute phases regardless of
/// the agent mode (Chat / RAG / Search).
///
/// Definitions are loaded from the `SkillRegistry` so adding a new atomic
/// tool only requires registering a `SkillComponent` — no edits here.
///
/// For hot paths prefer [`atomic_tool_catalog_cached`].
pub fn atomic_tool_catalog() -> Vec<Tool> {
    let registry = crate::agents::skills::registry::builtin_registry_cached();
    registry
        .iter()
        .map(|skill| {
            let gotchas = skill.gotchas().iter().map(|s| s.to_string()).collect();
            Tool::new(skill.spec()).with_gotchas(gotchas)
        })
        .collect()
}

// ============================================================================
// Calculator evaluation helper
// ============================================================================

/// Evaluate a mathematical expression string and return the numeric result.
///
/// Delegates to the calculator skill implementation so the logic lives in
/// one place (`skills/builtin/calculator.rs`).
pub fn evaluate_calculator_expression(expression: &str) -> Result<f64, String> {
    crate::agents::skills::builtin::calculator::evaluate_calculator_expression(expression)
}

/// Return a lazily-initialised global singleton of the atomic tool catalog.
pub fn atomic_tool_catalog_cached() -> &'static [Tool] {
    ATOMIC_TOOL_CATALOG.get_or_init(atomic_tool_catalog).as_slice()
}

// ============================================================================
// Search-specific tools
// ============================================================================

/// Build the search-specific tool catalog.
///
/// These tools are disclosed only when the agent mode is Search.
/// Loaded from the `SkillRegistry` so the definition lives in one place.
///
/// For hot paths prefer [`search_specific_tools_cached`].
pub fn search_specific_tools() -> Vec<Tool> {
    match crate::agents::skills::registry::builtin_registry_cached().get("web_search") {
        Some(skill) => {
            let gotchas = skill.gotchas().iter().map(|s| s.to_string()).collect();
            vec![Tool::new(skill.spec()).with_gotchas(gotchas)]
        }
        None => vec![],
    }
}

/// Return a lazily-initialised global singleton of the search-specific tool catalog.
pub fn search_specific_tools_cached() -> &'static [Tool] {
    SEARCH_SPECIFIC_CATALOG.get_or_init(search_specific_tools).as_slice()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_tool_catalog_has_all_atomic_tools() {
        let tools = atomic_tool_catalog();
        assert!(tools.len() >= 3);
        let names: Vec<&str> = tools.iter().map(|t| t.spec().name.as_str()).collect();
        assert!(names.contains(&"calculator"));
        assert!(names.contains(&"code_interpreter"));
        assert!(names.contains(&"weather_query"));
    }
}
