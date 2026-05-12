# RAG E2E Attempt — minsky86.pdf — 2026-04-28

## Scope

User requested a rerun of the new RAG-chain E2E:

- Upload file: `/mnt/e/Download/minsky86.pdf`
- Ask one RAG question
- Track upload -> parse -> index -> retrieve -> answer
- Record weak points and blockers

Secrets are intentionally redacted. No raw API keys are recorded here.

## Preconditions checked

### File

`/mnt/e/Download/minsky86.pdf` exists.

Observed size:

- `5,599,744 bytes`

### Key vault / env

Commands:

```bash
bash scripts/sync-keys.sh --check
set -a; . ./.env; set +a; bash scripts/check-e2e-env.sh --strict-citations
```

Result:

- All required keys are configured.
- Strict E2E environment check passed.
- `DATABASE_URL`, `MILVUS_URL`, `EMBEDDING_API_KEY`, `ANSWER_LLM_API_KEY` were present via `.env` / key sync.

### Local dependency services

Command:

```bash
set -a; . ./.env; set +a; bash scripts/dev-services-up.sh
```

Result:

- PostgreSQL: ready on `127.0.0.1:5432`
- Redis: ready on `127.0.0.1:6379`
- MinIO: ready on `127.0.0.1:9000`
- Milvus: not started by project script; script explicitly says `Milvus ... (start separately)`

Port check showed no listener on `127.0.0.1:19530`.

## Service startup attempt

### API

Command:

```bash
set -a; . ./.env; set +a; RUST_LOG=info cargo run -p avrag-api
```

Result: build succeeded, then runtime startup failed.

Relevant error:

```text
Error: http error: error sending request for url (http://127.0.0.1:19530/v2/vectordb/collections/list)

Caused by:
    0: error sending request for url (http://127.0.0.1:19530/v2/vectordb/collections/list)
    1: client error (Connect)
    2: tcp connect error
    3: Connection refused (os error 111)
```

### Worker

Command:

```bash
set -a; . ./.env; set +a; RUST_LOG=info cargo run -p avrag-worker
```

Result: build succeeded, then runtime startup failed with the same Milvus connection error.

Relevant error:

```text
Error: http error: error sending request for url (http://127.0.0.1:19530/v2/vectordb/collections/list)

Caused by:
    0: error sending request for url (http://127.0.0.1:19530/v2/vectordb/collections/list)
    1: client error (Connect)
    2: tcp connect error
    3: Connection refused (os error 111)
```

## Playwright outer check

Attempted command:

```bash
set -a; . ./.env; set +a; npx playwright test e2e/rust-frontend-e2e.spec.ts --config=playwright.config.ts --grep 'T01: health'
```

Result: Playwright command could not load project dependency `@playwright/test` from `avrag-rs`.

Relevant error:

```text
Error: Cannot find module '@playwright/test'
Require stack:
- /home/chuan/context-osv6/avrag-rs/playwright.config.ts
```

This is secondary. Even if Playwright dependency were available, the API was not running because Milvus is unavailable.

## E2E result

Status: BLOCKED before upload.

The test did not reach:

1. document record creation
2. upload to `/dev-upload/{document_id}`
3. worker parsing
4. embedding/indexing
5. RAG retrieval
6. final answer streaming
7. frontend progress/token validation

Root blocker:

- Milvus is required at `http://127.0.0.1:19530` during both API and worker startup.
- No Milvus process is listening.
- Docker/Milvus local startup is not currently available inside this WSL environment:
  - project script does not start Milvus
  - no local `milvus` binary exists
  - Docker Desktop WSL integration is not available from this distro

## Weak points found

1. **Critical — Milvus is a hard startup dependency**
   - API and worker fail before serving `/health` if Milvus is absent.
   - This prevents even upload/auth/API smoke tests from running in an environment without Milvus.

2. **High — Dev service script leaves a required dependency manual**
   - `scripts/dev-services-up.sh` starts PostgreSQL/Redis/MinIO but not Milvus.
   - It prints `Milvus ... (start separately)` but the repo does not provide a local Milvus startup script.

3. **Medium — E2E runner dependency not self-contained in `avrag-rs`**
   - `npx playwright test ...` fails because `@playwright/test` is not installed/resolvable from `avrag-rs`.
   - This is secondary to Milvus, but it means the documented E2E command is not currently runnable as-is in this checkout.

4. **Medium — Full RAG E2E depends on external model APIs plus Milvus**
   - Keys are present, but without Milvus the chain cannot start.
   - Once Milvus is available, the next likely validation points are PDF parser/MinerU access, embeddings, Milvus indexing, RAG retrieval coverage, and DeepSeek final answer streaming.

## Recommended next step

To complete the requested E2E, first provide a running Milvus service at:

```text
http://127.0.0.1:19530
```

Then rerun:

1. `cargo run -p avrag-api`
2. `cargo run -p avrag-worker`
3. API-level upload/query script for `/mnt/e/Download/minsky86.pdf`
4. Browser/front-end validation for progress panel logs:
   - `[workspace-chat-stream:activity]`
   - `[workspace-chat-stream:trace]`
   - `[workspace-chat-stream:token]`

Potential implementation options:

- Enable Docker Desktop WSL integration and run a Milvus standalone container/compose stack.
- Add a project-local `scripts/dev-milvus-up.sh` so `scripts/dev-services-up.sh` can bring up every hard runtime dependency.
- Alternatively, change API/worker startup behavior to degrade gracefully when Milvus is unavailable, but that would be a product/runtime design change and was not made in this E2E attempt.
