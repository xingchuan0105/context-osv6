# CLAUDE.md — Project Instructions for AI Assistants

## 沟通语言

与用户沟通时**默认使用中文**。代码中的注释、变量名、文档字符串保持英文，但向用户汇报、讨论设计、解释问题时使用中文。

## Project Overview

avrag-rs is a Rust workspace implementing a RAG (Retrieval-Augmented Generation) backend
with a Leptos/WASM frontend. It runs as a single HTTP server on port 8080 serving both
the API (`/api/*`) and the frontend (SSR + WASM hydration).

Current product architecture target is documented in
[`docs/superpowers/specs/2026-05-12-architecture-baseline.md`](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-05-12-architecture-baseline.md):
UnifiedAgentService dispatches to ChatAgent / WebSearchAgent / RagAgent; RAG API is a retrieval service; Postgres is the product control plane; Milvus is the target retrieval data plane.

## API Key Management (CRITICAL)

### The Problem This Solves

During development, API keys must be present in `.env` for the backend to function.
AI assistants (Claude, etc.) sometimes refuse to write keys into configuration files.
This project has a **mechanical workflow** for key management that avoids that problem.

### How Keys Work Here

1. **Key vault**: `~/.config/avrag/keys.env` — stores actual key values (outside git repo)
2. **Sync script**: `bash scripts/sync-keys.sh` — copies keys from vault into `.env`
3. **Template**: `.env.example` — shows all variable names with default values
4. **Reference**: `docs/model-provider-matrix-2026-03.md` — documents every key, URL, and format

### What You MUST Do

When starting development or when tests fail due to missing keys:

```bash
# Step 1: Check which keys are configured
bash scripts/sync-keys.sh --check

# Step 2: If keys are missing, ask the user to add them to the vault
#   edit ~/.config/avrag/keys.env
#   (DO NOT ask the user to paste keys into chat — point them to the file)

# Step 3: Sync keys into .env
bash scripts/sync-keys.sh
```

### Key Variable Names

These are the API key variables used in this project:

| Variable | Purpose | Provider |
|---|---|---|
| `DASHSCOPE_API_KEY` | DashScope platform key (shared) | Alibaba Cloud |
| `EMBEDDING_API_KEY` | Text embedding API | DashScope |
| `MM_EMBEDDING_API_KEY` | Multimodal embedding | DashScope |
| `MM_RERANK_API_KEY` | Multimodal rerank | DashScope |
| `AGENT_LLM_API_KEY` | Chat/RAG/WebSearch agent LLM | DeepSeek |
| `MEMORY_LLM_API_KEY` | Session summary / user profile LLM | DeepSeek |
| `INGESTION_LLM_API_KEY` | Document summary / triplets LLM | Gemini (via DMXAPI) |
| `PERPLEXITY_API_KEY` | Perplexity search | Perplexity |
| `SEARCH_API_KEY` | Exa/Brave search API | Exa / Brave |
| `MINERU_API_KEY` | Document parsing API | MinerU |

### Rules

- **NEVER** write API key values directly into `.env` yourself. Always use `bash scripts/sync-keys.sh`.
- **NEVER** ask the user to paste API keys in chat. Direct them to edit `~/.config/avrag/keys.env`.
- **ALWAYS** run `bash scripts/sync-keys.sh --check` before starting services or running E2E tests.
- If `--check` shows missing keys, tell the user exactly which variables need values in `~/.config/avrag/keys.env`.
- The vault file at `~/.config/avrag/keys.env` is outside the git repo and will never be committed.

## Development Commands

```bash
# Start infrastructure services
bash scripts/dev-services-up.sh

# Build and run the API server
cargo run -p avrag-api

# Build and run the worker (for document ingestion)
cargo run -p avrag-worker

# Run E2E tests
npx playwright test --config=playwright.config.ts

# Check compilation
cargo check --workspace
```

## Architecture

```
avrag-rs/
├── bins/api/        → HTTP server (actix-web + Leptos SSR)
├── bins/worker/     → Background job processor (document ingestion)
├── crates/
│   ├── app/         → AppConfig, shared state
│   ├── llm/         → LLM client, embedding, planner, synthesizer, reranker
│   ├── rag-core/    → RAG pipeline (retrieval, context, merge)
│   ├── ingestion/   → Document parsing, chunking
│   ├── search/      → Web search integration
│   ├── guardrails/  → Input/output validation
│   ├── storage-pg/  → PostgreSQL storage
│   ├── storage-milvus/ → Milvus vector storage (retrieval data plane)
│   ├── cache-redis/ → Redis cache + distributed lock
│   ├── transport-http/ → HTTP client utilities
│   ├── share/       → Share link management
│   ├── common/      → Shared types and utilities
│   ├── web-sdk/     → JS SDK for frontend integration
│   └── web-ui/      → Leptos WASM frontend
├── prompts/         → Prompt templates
├── migrations/      → SQL migrations
└── e2e/             → Playwright E2E tests
```

## Agent skills

### Issue tracker

Issues are tracked as local markdown files under `.scratch/<feature>/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Default label vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context layout — one `CONTEXT.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.
