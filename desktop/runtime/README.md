# Context-OS Client local data plane + product

Full **Milvus** + PostgreSQL + Redis for the desktop client, plus optional **avrag-api / avrag-worker** processes on the same machine.

## Quick start

```bash
# 1) Data plane (compose + client.env + migrations)
bash scripts/desktop-local-stack.sh ensure

# 2) Product processes (api :18080 + worker)
bash scripts/desktop-local-product.sh ensure

# status / stop
bash scripts/desktop-local-stack.sh status
bash scripts/desktop-local-product.sh status
bash scripts/desktop-local-product.sh stop
bash scripts/desktop-local-stack.sh down
```

Requires: **Docker** + **docker compose**, **sqlx-cli** (migrations), prebuilt or buildable `avrag-api` / `avrag-worker`.

```bash
cargo install sqlx-cli --no-default-features --features postgres
# binaries preferred from avrag-rs/target/{release,debug}/
```

## Ports (localhost only)

| Service        | Port  | Notes |
|----------------|-------|--------|
| PostgreSQL     | 5433  | client DB `avrag_client` |
| Redis          | 6380  | |
| Milvus         | 19530 | may share host Milvus; collections use `avrag_client` prefix |
| Milvus metrics | 19091 | |
| **Product API**| **18080** | offset from product-dev `:8080` |

## Env

`desktop/runtime/client.env` (generated, gitignored):

```bash
set -a && source desktop/runtime/client.env && set +a
```

Includes `DATABASE_URL`, `REDIS_URL`, `MILVUS_URL`, `AVRAG_API_ADDR`, `AVRAG_PUBLIC_BASE_URL`, `AVRAG_OBJECT_ROOT`, `MILVUS_COLLECTION_PREFIX=avrag_client`.

Product start also layers LLM keys from `avrag-rs/.env` (data-plane vars from `client.env` win).

Runtime dirs (gitignored): `run/` (pid), `logs/`, `objects/`, `data/`.

## Desktop IPC

| Command | Purpose |
|---------|---------|
| `get_local_stack_status` | TCP probe PG/Redis/Milvus |
| `get_client_runtime_config` | Connection strings |
| `ensure_local_stack` / `stop_local_stack` | Compose + migrate |
| `get_local_product_status` | API health + worker pid |
| `ensure_local_product` / `stop_local_product` | Stack ensure + api/worker |
| `api_call` | HTTP proxy → `http://127.0.0.1:18080` when product is up |

Settings UI (**本机数据栈**): data plane + product process controls.

Chat remains **BYOK in-process**; document ingest / Product Apps REST go through the local product API when started.

Optional: `CONTEXT_OS_ROOT` if monorepo root cannot be resolved.

## Product rules

- **No cloud account login** in the client.
- **21-day local trial**.
- LLM / Embedding: BYOK in client settings (also used by product if keys are in `avrag-rs/.env`).
- Data + product processes are **on-machine**.
