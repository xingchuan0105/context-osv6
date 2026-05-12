# Model/API Inventory and Streaming Diagnostics — 2026-04-28

## Scope

Backend model/API inventory after switching the Main Agent to DeepSeek v4-flash(max), plus streaming-path diagnosis.

Secrets are redacted. The DeepSeek key is stored only in the approved local key vault and synced into `.env`; this report intentionally contains no raw key.

## Main Agent switch result

Configured Main Agent (`ANSWER_LLM_*`) target:

- `ANSWER_LLM_BASE_URL=https://api.deepseek.com`
- `ANSWER_LLM_MODEL=deepseek-v4-flash`
- `ANSWER_LLM_ENABLE_THINKING=true`
- request body maps DeepSeek thinking mode to:
  - `thinking.type=enabled`
  - `thinking.reasoning_effort=max`

DeepSeek docs checked:

- OpenAI-compatible base URL: `https://api.deepseek.com`
- Chat path: `/chat/completions`
- Models include `deepseek-v4-flash` and `deepseek-v4-pro`
- `reasoning_effort` supports `high` and `max`
- `stream=true` returns SSE (`text/event-stream`)

## Backend model/API inventory

| Area | Config / code path | Current provider/model/API | Runtime role | Status / risk |
|---|---|---|---|---|
| Main Agent planning + final answer | `crates/app/src/lib_impl/config.rs`, `state_methods.rs`, `main_agent.rs` | DeepSeek `deepseek-v4-flash`, OpenAI-compatible `/chat/completions` | Product `/api/v1/chat` RAG planning, RAG final answer, general chat | Updated. This is the requested new Main Agent path. |
| Main Agent stream request body | `crates/llm/src/client.rs` | DeepSeek `thinking={type: enabled, reasoning_effort: max}` | Streaming/non-streaming chat completion body | Updated. Focused tests cover max reasoning effort. |
| Legacy planner LLM | `INTENT_LLM_*`, `config.rs`, `state_methods.rs` | DashScope compatible-mode, `qwen3.5-flash` | `RagRuntime` planner component; docs say product chat uses Main Agent instead | Still present. Risk: legacy or lower-level RAG runtime can still use old DashScope planner if called directly. Not changed because task requested Main Agent only. |
| Summary LLM | `SUMMARY_LLM_*`, `config.rs`, worker `build_worker_triplet_llm` fallback | DMXAPI, `gemini-3.1-flash-lite-preview` | summaries; also first choice for worker triplet extraction fallback before answer LLM | Still old provider/model. Risk: backend can still call DMX/Gemini during summary/triplet generation. Needs separate migration decision if all LLM calls should move to DeepSeek. |
| Worker triplet extraction LLM | `bins/worker/src/main.rs::build_worker_triplet_llm` | `SUMMARY_LLM_*` first, else `ANSWER_LLM_*` | graph/triplet extraction during ingestion | Partially old. If summary key is configured, triplet extraction still uses summary DMX/Gemini. If summary config absent, it falls back to DeepSeek answer config. |
| Text embedding | `EMBEDDING_*`, `crates/llm/src/embedding.rs` | `.env` currently points at DashScope compatible-mode `text-embedding-v4`; code default still SiliconFlow `Qwen/Qwen3-Embedding-8B` | ingestion embeddings + query embeddings | Runtime env is DashScope, but code default/fallback still SiliconFlow. Risk if `.env` missing or not loaded. |
| Memory-mode fallback embedding | `crates/app/src/lib_impl/state_methods.rs` | hardcoded SiliconFlow `Qwen/Qwen3-Embedding-8B` with empty key | fallback object when no embedding config exists | Old model in code. Because key is empty, it is not useful for real calls; risk is confusing fallback and degraded behavior if env is missing. |
| Multimodal embedding | `MM_EMBEDDING_*`, `embedding.rs` | DashScope native `qwen3-vl-embedding` | multimodal asset embeddings | Still DashScope. Expected non-Main-Agent path. |
| Multimodal rerank | `MM_RERANK_*`, `reranker.rs` | DashScope native `qwen3-vl-rerank` | final multimodal/text rerank | Still DashScope. Expected retrieval path. |
| Text rerank fallback | `RERANK_*`, `reranker.rs` | SiliconFlow-style `/rerank`, `Qwen/Qwen3-Reranker-8B` | fallback reranker | Old/legacy fallback remains. Risk only if configured; docs already warn not to point this at DashScope native without client changes. |
| Search LLM | `SEARCH_LLM_*`, `crates/search` | DashScope compatible-mode `qwen3.5-plus` | search planner/tool mode | Still old provider/model. Separate from Main Agent. |
| Search provider | `SEARCH_PROVIDER`, `SearchExecutor` | Perplexity + optional Exa config | web search mode | Still external search provider. Not part of Main Agent switch. |
| HTTP OpenAI-compatible route | `/v1/notebooks/{notebook_id}/chat/completions` | internal API compatibility route | transport compatibility | Name is OpenAI-compatible API surface, not necessarily old external OpenAI provider. |
| Docs/comments | `docs/model-provider-matrix-2026-03.md`, `CLAUDE.md`, older planning docs | still mention DMXAPI as Main Agent | documentation | Stale after this switch. Reported, not fully rewritten here. |

## Audit conclusion

The product Main Agent path is switched to DeepSeek v4-flash(max), but the backend as a whole still contains intentional old-provider call paths:

1. Summary/triplet extraction may still call DMXAPI Gemini-family models.
2. Legacy planner / lower-level RAG runtime can still call DashScope `qwen3.5-flash`.
3. Search mode still calls DashScope/Perplexity.
4. Embedding/rerank paths still use DashScope/SiliconFlow-style providers.
5. A hardcoded empty-key SiliconFlow embedding fallback remains in `state_methods.rs`.
6. Provider matrix docs are stale for Main Agent.

Recommended next decisions:

- If “all chat/generation LLMs” should move to DeepSeek, migrate `SUMMARY_LLM_*` and worker triplet extraction policy next.
- If only Main Agent should move, leave retrieval/search/summary paths as-is but update docs to explicitly say so.
- Consider deleting or replacing the hardcoded SiliconFlow fallback embedding object to fail more explicitly when embedding config is absent.

## Streaming diagnosis

### Provider-level check

Direct DeepSeek streaming check succeeded without printing secrets:

- HTTP status: 200
- Content-Type: `text/event-stream; charset=utf-8`
- Byte chunks observed: 95
- SSE data events observed: 76
- Content delta events observed: 16
- First byte latency: ~0.672s

This confirms DeepSeek itself can produce real streamed deltas for the configured model.

### Backend streaming path

Relevant path:

- HTTP: `crates/transport-http/src/handlers.rs`
  - `Accept: text/event-stream` selects `chat_live_stream_response`
  - Axum `Sse` emits events as the channel receives them
  - `Cache-Control: no-cache`
  - `x-accel-buffering: no`
- Main Agent / LLM stream:
  - `crates/app/src/lib_impl/chat_streaming.rs::execute_rag_chat_stream`
  - `answer_rag_with_main_agent_stream(... on_delta ...)`
  - `crates/main_agent` delegates to `LlmClient::complete_stream`
  - `crates/llm/src/client.rs` reads SSE chunks and calls `on_delta` for every content delta

So the backend has a real streaming path for RAG final answer generation.

Important caveat:

- If the model/client produces no deltas, the backend falls back to `chunk_text_for_stream(answer_output.answer_text)`, which emits 24-character chunks after the full answer is already available. That looks like streaming in the UI but is not true provider streaming.
- Clarify/fallback/buffered paths also emit chunked tokens after full text exists.

### Frontend logging added

Frontend transport now logs every token SSE event at:

- `frontend_next/lib/workspace/stream.ts`

Log label:

- `[workspace-chat-stream:token]`

Payload:

- `request_id`
- `message_id`
- `chars`
- `content`

This confirms tokens reached the frontend transport. It does not by itself prove whether those tokens came directly from provider deltas or backend fallback chunking; timing and backend logs are needed for that distinction.

### Streaming diagnostic conclusion

- DeepSeek provider: true streaming confirmed.
- Backend: true streaming code path exists for RAG final answer and general chat.
- Frontend: SSE parser receives token events and now logs them.
- Progress panel: `activity` and `trace` SSE events are now also logged, and the research progress card is no longer hidden by `answer_start` / first `token` for `rag` or `search` modes. It remains visible while answer streaming starts, then is cleared on `done` / `error`.
- Remaining uncertainty: local end-to-end RAG run must confirm whether the RAG final-answer path gets real DeepSeek deltas or falls back to buffered chunking in this specific deployment.

## 2026-04-28 follow-up: progress panel fix

User-reported symptom: the screenshot panel (`网络搜索中` / `正在生成网络搜索计划`) should show streaming process updates, but a previous conversation showed no process content and the answer appeared directly.

Implemented frontend fix:

- `frontend_next/lib/workspace/stream.ts`
  - logs `activity` events as `[workspace-chat-stream:activity]`
  - logs `trace` events as `[workspace-chat-stream:trace]`
  - keeps existing `[workspace-chat-stream:token]` answer-token diagnostics
- `frontend_next/components/workspace/workspace-chat-pane.tsx`
  - stops hiding the research progress card on `answer_start`
  - stops hiding the research progress card on first `token` when active mode is `rag` or `search`
  - keeps clearing progress on `done` / `error`
  - uses a ref-backed active progress mode so stream callbacks do not see stale React state

Tests:

- `pnpm test tests/workspace/stream.test.ts tests/workspace/workspace-chat-pane.test.tsx -- --runInBand` passed: 18 tests.
- `pnpm typecheck` passed before reverting the generated `next-env.d.ts` noise back out of the working tree.
- `git diff --check` passed.

Non-goal / not changed yet:

- This does not expose raw DeepSeek `reasoning_content` / chain-of-thought fields. The panel currently displays product progress events (`activity` / `trace`), not hidden model reasoning tokens.
- Real browser E2E is still needed to confirm the deployed local app receives visible `activity` events during an actual RAG/search request.
