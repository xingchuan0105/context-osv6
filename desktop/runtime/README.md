# Context-OS Client local data plane + product

Full **Milvus** + PostgreSQL + Redis for the desktop client, plus optional **avrag-api / avrag-worker** processes and a **local B2C personal session** (no cloud login).

## Quick start (monorepo)

```bash
# Optional: stage api/worker into desktop/runtime/bin (and Tauri binaries/)
bash scripts/stage-desktop-sidecars.sh
# or STAGE_BUILD=1 bash scripts/stage-desktop-sidecars.sh

# 1) Data plane (compose + client.env + migrations + JWT_SECRET)
bash scripts/desktop-local-stack.sh ensure

# 2) Product processes (api :18080 + worker)
bash scripts/desktop-local-product.sh ensure

# status / stop
bash scripts/desktop-local-product.sh status
bash scripts/desktop-local-product.sh stop
bash scripts/desktop-local-stack.sh down
```

Requires: **Docker** + **docker compose**, **sqlx-cli** (migrations), staged or cargo-built `avrag-api` / `avrag-worker`.

## Non-monorepo / install layout

```text
CONTEXT_OS_CLIENT_HOME/
  bin/avrag-api[.exe]
  bin/avrag-worker[.exe]
  docker-compose.client.yml
  client.env          # generated
  jwt.secret          # generated
  data/ run/ logs/ objects/
  desktop-local-stack.sh
  desktop-local-product.sh
```

```bash
export CONTEXT_OS_CLIENT_HOME=/path/to/runtime-sidecars
# scripts resolve ROOT relative to themselves; prefer running from monorepo
# or copy scripts into CLIENT_HOME and set CONTEXT_OS_ROOT if needed.
```

Release packaging (`scripts/package-desktop-release.sh`) stages a `runtime-sidecars/` companion folder next to the NSIS installer.

Tauri `bundle.resources` ships compose + README with the app. Product binaries are staged via `stage-desktop-sidecars.sh` (optional `desktop/src-tauri/binaries/*-<triple>` for future `externalBin`).

## Ports

| Service        | Port  |
|----------------|-------|
| PostgreSQL     | 5433  |
| Redis          | 6380  |
| Milvus         | 19530 |
| Product API    | 18080 |

## Local B2C session

- Email: `local@context-os.client` (personal account; **no org**)
- Credentials: app data `local_user.json` (device-local)
- JWT: app data `local_session.json`
- Desktop shell calls `ensure_local_session` after license OK — never cloud `/login`
- REST via `api_call` with Bearer token

Legal versions on register: `2026-06-13` (terms + privacy).

## Desktop IPC (summary)

| Command | Purpose |
|---------|---------|
| `ensure_local_stack` / `stop_local_stack` | Compose + migrate |
| `ensure_local_product` / `stop_local_product` | api + worker |
| `ensure_local_session` / `get_local_session` | Local personal JWT |
| `api_call` | Proxy to `:18080` |

## Product rules

- **No cloud account login** in the client.
- **21-day local trial**.
- LLM / Embedding: BYOK; product may also use keys from `avrag-rs/.env` in monorepo.
