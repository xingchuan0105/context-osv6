# Prompt Templates

This directory contains externalized prompt templates for the RAG pipeline.
Moving prompts out of source code allows:
- Non-engineers to tune prompts without code changes
- A/B testing different prompt versions
- Hot-reloading prompts in production

## Files

| File | Used By | Description |
|------|---------|-------------|
| `rag_plan_system.txt` | `main_agent/mod.rs` | RAG planning mode system prompt |
| `rag_answer_system.txt` | `main_agent/mod.rs` | RAG answer synthesis system prompt |
| `search_plan_system.txt` | `agents/web_search_agent.rs` | Web search planner: intent recognition, coreference resolution, sub-query generation, preferred vertical routing |
| `web_search_system.txt` | `agents/web_search_agent.rs` | Web search answer synthesis system prompt |
| `chat_agent_system.txt` | `main_agent/mod.rs`, `agents/chat_agent.rs` | General chat mode system prompt |
| `triplet_extraction_system.txt` | `worker` | Knowledge graph triplet extraction system prompt |
| `summary_generation.v1.tmpl` | `llm/src/summary.rs` | Document summary generation system prompt |
| `summary_generation_finalize.v1.tmpl` | `llm/src/summary.rs` | Summary finalize system prompt |
| `session_summary_system.txt` | `lib_impl/chat_private.rs` | Session conversation summary system prompt |
| `user_profile_extraction_system.txt` | `lib_impl/chat_private.rs` | Dream layer: nightly user profile consolidation with slot-based fusion, confidence scoring, and eviction |

## Loading

Most prompts are loaded at compile time via `include_str!` for zero runtime overhead.

The `chat_agent_system.txt` prompt additionally supports runtime override: `ChatAgent`
reads `{PROMPT_DIR}/chat_agent_system.txt` at service startup if the file exists,
failing back to the compile-time embedded copy.

The `search_plan_system.txt` and `web_search_system.txt` prompts are loaded via
`include_str!` in `WebSearchAgent`.

### Search Plan Output Schema

`search_plan_system.txt` emits a JSON object with these fields:

| Field | Type | Description |
|-------|------|-------------|
| `sub_queries` | `string[]` | 1-3 standalone search queries that collectively cover user intent |
| `intent_summary` | `string` | One-sentence neutral summary of resolved user intent |
| `needs_clarification` | `boolean` | True when pronouns/entities cannot be resolved confidently |
| `preferred_vertical` | `"web" \| "news" \| null` | Hints which Brave API surface to use |

When `preferred_vertical` is `"news"`, the executor routes to Brave's `/res/v1/news/search`
endpoint instead of the default LLM Context endpoint. Runtime parameters (`country`,
`search_lang`, `freshness`) are injected from `SearchConfig` rather than hardcoded into queries.

The summary templates support runtime override via `load_prompt_template()`
in the worker binary, controlled by the `PROMPT_SUMMARY_VERSION` environment variable.

## Versioning

Prompt changes should be versioned alongside code releases.
Major prompt changes (new fields, new constraints) require code changes.
Minor tuning (wording, examples) can be deployed independently.
