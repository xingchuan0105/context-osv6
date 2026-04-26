# Model Provider Matrix

Last updated: 2026-03-26

> Historical provider matrix. Local infra and retrieval references to Qdrant/Tantivy reflect the March 2026 implementation profile. The target retrieval data plane is now Milvus; see [2026-04-26 Current Product Architecture](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md).

This document records the current model-provider wiring used by `context-osv6/avrag-rs`, including:
- environment variables
- base URLs
- request paths
- request body shapes
- active codepaths
- provider-specific caveats

The goal is to prevent config drift during E2E, debugging, and future model swaps.

## Active Runtime Split

Recommended current profile:
- local infra: PostgreSQL + Redis + Qdrant + local object storage
- DashScope: text embedding, multimodal embedding, multimodal rerank
- DMXAPI: Main Agent answer/planning model, summary generation
- 2026-04-26 target addition: graph triplet extraction uses the benchmarked Gemini 3.1 Flash-family provider/model via the existing DMXAPI-compatible configuration, with thinking disabled when supported.
- `INTENT_LLM_*` is retained only for legacy RAG planner-compatible paths; `/api/v1/chat` uses `ANSWER_LLM_*` for Main Agent planning and answering.

This means a full RAG E2E does not require SiliconFlow keys if text embedding is switched to DashScope-compatible mode and text rerank fallback is left unset.

## Local Infra

Required local env:
- `DATABASE_URL`
- `REDIS_URL`
- `QDRANT_URL`
- `AVRAG_API_ADDR`
- `AVRAG_PUBLIC_BASE_URL`
- `AVRAG_RUN_MIGRATIONS`
- `AVRAG_OBJECT_ROOT`
- `TANTIVY_INDEX_DIR` (optional lexical index; API reads it, worker writes it)

Typical local values:

```env
DATABASE_URL=postgres://avrag:avrag@127.0.0.1:5432/avrag_rs
REDIS_URL=redis://127.0.0.1:6379/0
QDRANT_URL=http://127.0.0.1:6333
AVRAG_API_ADDR=127.0.0.1:38080
AVRAG_PUBLIC_BASE_URL=http://127.0.0.1:38080
AVRAG_RUN_MIGRATIONS=true
AVRAG_OBJECT_ROOT=/home/chuan/.local/share/avrag-dev/objects
TANTIVY_INDEX_DIR=/home/chuan/.local/share/avrag-dev/tantivy-index
```

## Provider Map

| Function | Env Prefix | Recommended Provider | Base URL | Request Path | Code |
|---|---|---|---|---|---|
| Legacy planner LLM | `INTENT_LLM_*` | DashScope compatible-mode | `https://dashscope.aliyuncs.com/compatible-mode/v1` | `/chat/completions` | `crates/llm/src/client.rs` |
| Main Agent / Answer LLM | `ANSWER_LLM_*` | DMXAPI OpenAI format | `https://www.dmxapi.cn/v1` | `/chat/completions` | `crates/llm/src/client.rs` |
| Summary LLM | `SUMMARY_LLM_*` | DMXAPI OpenAI format | `https://www.dmxapi.cn/v1` | `/chat/completions` | `crates/llm/src/client.rs` |
| Graph triplet extraction target | graph extraction config TBD; benchmark used DMXAPI-compatible key | Gemini 3.1 Flash-family | `https://www.dmxapi.cn/v1` | `/chat/completions` | planned RAG API ingestion operator |
| Text embedding | `EMBEDDING_*` | DashScope OpenAI-compatible embedding | `https://dashscope.aliyuncs.com/compatible-mode/v1` | `/embeddings` | `crates/llm/src/embedding.rs` |
| Multimodal embedding | `MM_EMBEDDING_*` | DashScope native multimodal embedding | `https://dashscope.aliyuncs.com/api/v1/services/embeddings/multimodal-embedding/multimodal-embedding` | same URL | `crates/llm/src/embedding.rs` |
| Multimodal rerank | `MM_RERANK_*` | DashScope native rerank | `https://dashscope.aliyuncs.com/api/v1/services/rerank/text-rerank/text-rerank` | same URL | `crates/llm/src/reranker.rs` |
| Text rerank fallback | `RERANK_*` | optional fallback | current default is SiliconFlow-style | `/rerank` | `crates/llm/src/reranker.rs` |

## Request Formats

### 1. OpenAI-style chat completions

Used by:
- Main Agent RAG execute-plan generation and answer synthesis through `ANSWER_LLM_*`
- legacy planner-compatible paths through `INTENT_LLM_*`
- summary generation

Environment:
- `INTENT_LLM_BASE_URL`
- `INTENT_LLM_API_KEY`
- `INTENT_LLM_ENABLE_THINKING`
- `ANSWER_LLM_BASE_URL`
- `SUMMARY_LLM_BASE_URL`

Request:

```json
{
  "model": "model-name",
  "messages": [
    { "role": "system", "content": "..." },
    { "role": "user", "content": "..." }
  ],
  "temperature": 0.2
}
```

HTTP shape:
- method: `POST`
- path: `/chat/completions`
- auth header: `Authorization: Bearer <API_KEY>`
- content-type: `application/json`

Code:
- `crates/llm/src/client.rs`

Planner profile:
- `INTENT_LLM_BASE_URL=https://dashscope.aliyuncs.com/compatible-mode/v1`
- `INTENT_LLM_MODEL=qwen3.5-flash`
- `INTENT_LLM_ENABLE_THINKING=false`
- This is a legacy planner setting. The current `/api/v1/chat` RAG path generates `ExecutePlanRequest` with the Main Agent on `ANSWER_LLM_*`.

### 2. OpenAI-compatible text embedding

Used by:
- text dense retrieval

Environment:
- `EMBEDDING_BASE_URL`
- `EMBEDDING_API_KEY`
- `EMBEDDING_MODEL`
- `EMBEDDING_DIMENSIONS`
- `AVRAG_EMBEDDING_DIM` (legacy alias used by the worker for Qdrant collection sizing; keep it equal to `EMBEDDING_DIMENSIONS`)

Recommended DashScope profile:

```env
EMBEDDING_BASE_URL=https://dashscope.aliyuncs.com/compatible-mode/v1
EMBEDDING_MODEL=text-embedding-v4
EMBEDDING_DIMENSIONS=1024
AVRAG_EMBEDDING_DIM=1024
EMBEDDING_API_KEY=...
```

Official DashScope note:
- `text-embedding-v4` defaults to `1024` dimensions.
- Supported `dimensions` values are `64`, `128`, `256`, `512`, `768`, `1024`, `1536`, and `2048`.
- For retrieval, `1024` is the safe default unless we intentionally optimize storage/latency.

Request:

```json
{
  "model": "text-embedding-v4",
  "input": ["text a", "text b"],
  "dimensions": 1024
}
```

HTTP shape:
- method: `POST`
- path: `/embeddings`
- auth header: `Authorization: Bearer <API_KEY>`

Code:
- `crates/llm/src/embedding.rs`

Operational caveat:
- Qdrant collection size must match the requested embedding dimension exactly.
- If an older collection was created at `64`, either delete/recreate it or point `QDRANT_COLLECTION` to a fresh collection before running ingestion.

### 3. DashScope native multimodal embedding

Used by:
- `query item -> Multimodal Dense`

Environment:
- `MM_EMBEDDING_BASE_URL`
- `MM_EMBEDDING_API_KEY`
- `MM_EMBEDDING_MODEL`
- `MM_EMBEDDING_API_STYLE=dashscope_multimodal_embedding`

Request:

```json
{
  "model": "qwen3-vl-embedding",
  "input": {
    "contents": [
      {
        "text": "diagram explanation",
        "image": "https://..."
      }
    ]
  },
  "parameters": {
    "output_type": "dense",
    "enable_fusion": true
  }
}
```

Code:
- `crates/llm/src/embedding.rs`

### 4. DashScope native multimodal rerank

Used by:
- final `text_pool + multimodal_pool -> qwen3-vl-rerank`

Environment:
- `MM_RERANK_BASE_URL`
- `MM_RERANK_API_KEY`
- `MM_RERANK_MODEL=qwen3-vl-rerank`
- `MM_RERANK_API_STYLE=dashscope_vl_rerank`

Request:

```json
{
  "model": "qwen3-vl-rerank",
  "input": {
    "query": { "text": "user query" },
    "documents": [
      { "text": "candidate text" },
      { "image": "https://..." }
    ]
  },
  "parameters": {
    "return_documents": false,
    "top_n": 30,
    "instruct": "Given a web search query, retrieve relevant passages that answer the query."
  }
}
```

Code:
- `crates/llm/src/reranker.rs`

### 5. Text rerank fallback

Used by:
- fallback when multimodal rerank is unavailable

Current client expectation:

```json
{
  "model": "model-name",
  "query": "user query",
  "documents": ["doc a", "doc b"]
}
```

HTTP shape:
- method: `POST`
- path: `/rerank`

Important caveat:
- current non-VL rerank client is still written for a SiliconFlow-style `/rerank` path.
- do not point `RERANK_BASE_URL` directly at DashScope native rerank unless the client is updated.
- for a DashScope-only runtime, it is safer to leave `RERANK_*` unset and rely on `MM_RERANK_*`.

Code:
- `crates/llm/src/reranker.rs`

## Recommended DashScope + DMXAPI Profile

```env
DASHSCOPE_API_KEY=...

EMBEDDING_BASE_URL=https://dashscope.aliyuncs.com/compatible-mode/v1
EMBEDDING_MODEL=text-embedding-v4
EMBEDDING_API_KEY=...

INTENT_LLM_BASE_URL=https://dashscope.aliyuncs.com/compatible-mode/v1
INTENT_LLM_MODEL=qwen3.5-flash
INTENT_LLM_API_KEY=...

ANSWER_LLM_BASE_URL=https://www.dmxapi.cn/v1
ANSWER_LLM_MODEL=gemini-3-flash-preview-thinking
ANSWER_LLM_API_STYLE=openai
ANSWER_LLM_API_KEY=...

MM_EMBEDDING_BASE_URL=https://dashscope.aliyuncs.com/api/v1/services/embeddings/multimodal-embedding/multimodal-embedding
MM_EMBEDDING_MODEL=qwen3-vl-embedding
MM_EMBEDDING_API_STYLE=dashscope_multimodal_embedding
MM_EMBEDDING_API_KEY=...

MM_RERANK_BASE_URL=https://dashscope.aliyuncs.com/api/v1/services/rerank/text-rerank/text-rerank
MM_RERANK_MODEL=qwen3-vl-rerank
MM_RERANK_API_STYLE=dashscope_vl_rerank
MM_RERANK_API_KEY=...

SUMMARY_LLM_BASE_URL=https://www.dmxapi.cn/v1
SUMMARY_LLM_MODEL=gemini-3.1-flash-lite-preview
SUMMARY_LLM_API_STYLE=openai
SUMMARY_LLM_API_KEY=...
```

Optional:
- `RERANK_*` can remain unset for this profile.
- `SEARCH_*` is only needed for `search` mode E2E.

## Drift Notes

- `crates/app/src/lib.rs` still defaults `EMBEDDING_*` and `RERANK_*` to SiliconFlow-shaped values in `AppConfig::default()`.
- E2E and runtime startup should override these via env when using DashScope-only mode.
- If DashScope-only becomes the permanent default, `AppConfig::default()` and `.env.example` should be updated together.

## External References

- DashScope OpenAI-compatible embedding:
  https://help.aliyun.com/zh/model-studio/developer-reference/embedding-interfaces-compatible-with-openai
- DashScope text rerank / multimodal rerank:
  https://help.aliyun.com/zh/model-studio/text-rerank-api
- DMXAPI common OpenAI-format endpoints:
  https://doc.dmxapi.cn/jiekou.html
- DMXAPI Gemini native chat:
  https://doc.dmxapi.cn/gemini-chat.html
