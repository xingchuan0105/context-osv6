# Context-OS Client local data plane

Full **Milvus** + PostgreSQL + Redis for the desktop client (vector knowledge graph requires Milvus).

## Quick start

```bash
# monorepo root — start stack, write client.env, apply migrations
bash scripts/desktop-local-stack.sh ensure

# or step by step
bash scripts/desktop-local-stack.sh up
bash scripts/desktop-local-stack.sh migrate
bash scripts/desktop-local-stack.sh status
```

Requires: **Docker** + **docker compose**, and **sqlx-cli** for migrations:

```bash
cargo install sqlx-cli --no-default-features --features postgres
```

Ports (localhost only):

| Service        | Port  |
|----------------|-------|
| PostgreSQL     | 5433  |
| Redis          | 6380  |
| Milvus         | 19530 |
| Milvus metrics | 19091 |

## Client process env

`ensure` / `up` / `write-env` write:

`desktop/runtime/client.env` (gitignored under `data/`; the env file itself is local-generated)

```bash
set -a && source desktop/runtime/client.env && set +a
```

Defaults:

```bash
export DATABASE_URL=postgres://avrag:avrag@127.0.0.1:5433/avrag_client
export REDIS_URL=redis://127.0.0.1:6380/0
export MILVUS_URL=http://127.0.0.1:19530
export CLIENT_PG_PORT=5433
export CLIENT_REDIS_PORT=6380
export CLIENT_MILVUS_PORT=19530
export AVRAG_RUN_MIGRATIONS=true
export AVRAG_MIGRATIONS_DIR=<monorepo>/avrag-rs/migrations
```

Point **avrag-api** / **worker** at these URLs for a monorepo-local data plane matching SaaS shape. Desktop chat remains **BYOK** in-process; full product ingest/API sidecar attach is a follow-up.

## Desktop IPC

| Command | Purpose |
|---------|---------|
| `get_local_stack_status` | TCP probe PG/Redis/Milvus + env file presence |
| `get_client_runtime_config` | Connection strings + monorepo paths |
| `ensure_local_stack` | Runs `desktop-local-stack.sh ensure` |
| `stop_local_stack` | Runs `… down` (volumes retained) |

Settings UI (**本机数据栈** tab): 启动并迁移 / 重新探测 / 停止栈.

Optional: set `CONTEXT_OS_ROOT` if the app cannot resolve the monorepo root.

## Product rules

- **No cloud account login** in the client.
- **21-day local trial** (device-bound file).
- LLM / Embedding: BYOK in client settings.
- Data plane is **on-machine** via this compose stack.

## Stop

```bash
bash scripts/desktop-local-stack.sh down
```

Data volumes stay under `desktop/runtime/data/` (gitignored).
