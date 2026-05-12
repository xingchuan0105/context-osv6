# RAG E2E / DeepSeek / Backend Model Inventory / Streaming Diagnostics Follow-ups

## Goal

Record the next batch of backend/frontend follow-up tasks for `context-osv6/avrag-rs` after the ingestion/retrieval hardening work.

Important security rule: the user provided a DeepSeek API key in chat. Do not copy the raw key into this plan, logs, commits, screenshots, or summaries. Use `[REDACTED]` everywhere except the project’s already-existing dedicated key/config document when executing the key-recording task.

## Current context / assumptions

- Workspace: `/home/chuan/context-osv6/avrag-rs`
- Input PDF for E2E test:
  - Windows path: `E:\Download\minsky86.pdf`
  - WSL path: `/mnt/e/Download/minsky86.pdf`
- DeepSeek docs: `https://api-docs.deepseek.com/zh-cn/`
- Target model requested by user: `DeepSeek v4-flash(max)`
- Known candidate key/config-related files discovered by filename only:
  - `/home/chuan/context-osv6/avrag-rs/.env`
  - `/home/chuan/context-osv6/avrag-rs/.env.example`
  - `/home/chuan/context-osv6/avrag-rs/.env.bak-20260413-140607`
  - `/home/chuan/context-osv6/avrag-rs/scripts/sync-keys.sh`
- Do not inspect or print secret-bearing files unless needed for the specific key-recording task, and redact any secret values in outputs.

## Task 1 — New RAG chain E2E test only

### Goal

Run an end-to-end test that covers only the new RAG chain:

1. Upload `/mnt/e/Download/minsky86.pdf`.
2. Ask one question against the uploaded document.
3. Trace the full chain:
   - upload/API request
   - document status transitions
   - parsing
   - IR generation
   - chunking/indexing
   - retrieval
   - answer generation
   - frontend/backend observable behavior if applicable
4. Report whether it works normally.
5. Identify weak links and concrete problem points.

### Constraints

- This is diagnostic/verification work, not broad refactoring.
- Keep logs useful but avoid leaking credentials, private keys, or connection strings.
- Prefer focused tracing around this one document rather than running unrelated E2E suites.

### Likely verification commands / checks

- Confirm file exists: `/mnt/e/Download/minsky86.pdf`.
- Check required services/processes and env before starting.
- Use existing API/client paths where possible instead of ad-hoc DB mutations.
- Query database state only as needed to trace document lifecycle.
- Capture exact failure stage if the chain breaks.

### Deliverable

A diagnostic report with:

- pass/fail result
- document ID / request ID / trace IDs if available
- timeline of status transitions
- retrieved evidence/chunks if available
- final answer behavior
- weak spots and recommended next fixes

## Task 2 — Switch main agent to DeepSeek v4-flash(max)

### Goal

Change the main agent model/provider configuration to use DeepSeek v4-flash(max), based on the official docs.

### Required docs

- `https://api-docs.deepseek.com/zh-cn/`

### API key handling

- The user provided a DeepSeek API key in chat.
- Do not write the raw key into this plan, logs, commit messages, or responses.
- When executing, locate the project’s already-existing dedicated key/config document and write the real key only there.
- If the dedicated key document cannot be confidently identified, stop and ask before writing the key anywhere.

### Likely implementation areas

- Main agent model/provider configuration.
- Backend LLM client/provider abstraction.
- Environment/key loading.
- Any model allowlist/default model config.
- Tests or config examples should use placeholders only.

### Validation

- Confirm new config is loaded.
- Confirm main agent requests target DeepSeek endpoint/model.
- Confirm no raw key appears in git diff, logs, or test output.

## Task 3 — Backend model/API inventory

### Goal

Audit current backend code to determine whether it still calls old models or old APIs.

### Scope

Search and inspect backend references to:

- model names
- provider names
- base URLs
- API endpoint paths
- OpenAI-compatible clients
- embedding/rerank/chat/completion clients
- config defaults and fallbacks
- test fixtures and examples that may accidentally preserve old defaults

### Output

Produce a structured inventory:

- file path
- symbol/function/config key
- old/new provider or model reference
- runtime path or test-only path
- whether it needs migration
- risk level
- recommended action

### Constraints

- This is an audit/梳理 task first. Do not rewrite broadly unless separately requested.

## Task 4 — Verify real streaming after model switch; add token-arrival log if needed

### Goal

After switching to the new model, verify whether the frontend/backend path has true streaming behavior.

### Requested behavior

- Inspect the frontend streaming component/path.
- Add or identify a log that records token/chunk arrival if real tokens arrive.
- If no true streaming effect exists, diagnose and explain why.
- Do not fix the streaming implementation in this task unless explicitly requested later.

### Areas to inspect

- Backend streaming response path: SSE / chunked HTTP / WebSocket / fetch streaming.
- Agent/LLM client streaming API usage.
- Transport layer buffering behavior.
- Frontend component state updates during streaming.
- Browser/network behavior and logs.

### Deliverable

A diagnostic report with:

- whether real token-level/chunk-level streaming exists
- where the stream is generated
- where it may be buffered
- frontend evidence/log output
- root cause if the UI is not truly streaming
- recommended fix options, without implementing them

## Execution order recommendation

1. Task 2: switch main agent model/key config, because Task 4 depends on the new model.
2. Task 3: backend model/API inventory, to catch leftover old-provider calls after or alongside Task 2.
3. Task 4: streaming diagnosis after the model switch.
4. Task 1: full new-RAG E2E with `minsky86.pdf`, once model/config and streaming observability are understood.

Alternative if the user wants pure diagnostic first: run Task 3 before Task 2.

## Open questions before execution

- What exact question should be asked against `minsky86.pdf` during the E2E test? If not specified, choose a simple content-grounded question after extracting/previewing the PDF text.
- Which file is the project’s canonical dedicated key document? Candidate files exist, but the exact intended file should be confirmed or discovered carefully without printing secrets.
