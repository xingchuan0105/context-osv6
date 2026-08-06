# Context-OS local data plane + product

**Default: no Docker.** Data plane is **PostgreSQL + pgvector** + **Redis**, started as **native host processes** (`pg_ctl` + `redis-server`). Optional Docker compose remains a fallback (`STACK_MODE=docker`).

Retrieval: `RETRIEVAL_BACKEND=pgvector` (cloud SaaS still uses Milvus). Graph hop / VGRAG gates: G1+G2 in `avrag-rs/docs/engineering/2026-08-04-pgvector-graph-hop-g1-spec.md`.

## Prerequisites (native)

| OS | Packages |
|----|----------|
| Debian/Ubuntu | `postgresql-16` `postgresql-16-pgvector` `redis-server` |
| Fedora | `postgresql-server` + pgvector package / build, `redis` |
| macOS | `brew install postgresql@16 redis` + pgvector |
| Windows | Official PostgreSQL 16 installer + [pgvector](https://github.com/pgvector/pgvector) + Redis/Memurai |

Also: `sqlx-cli` for migrations (`cargo install sqlx-cli --no-default-features --features postgres`).

## Quick start (monorepo)

```bash
# 1) Native data plane (default STACK_MODE=auto → native when tools exist)
bash scripts/desktop-local-stack.sh ensure

# Force native / docker
# STACK_MODE=native bash scripts/desktop-local-stack.sh ensure
# STACK_MODE=docker bash scripts/desktop-local-stack.sh ensure

# 2) Product processes (api :18080 + worker)
bash scripts/stage-desktop-sidecars.sh   # once
bash scripts/desktop-local-product.sh ensure

bash scripts/desktop-local-product.sh status
bash scripts/desktop-local-stack.sh down
```

## Ports

| Service               | Port  | Native data |
|-----------------------|-------|-------------|
| PostgreSQL + pgvector | 5433  | `desktop/runtime/data/pg-native/` |
| Redis                 | 6380  | `desktop/runtime/data/redis-native/` |
| Product API           | 18080 | logs under `desktop/runtime/logs/` |

Docker volumes (legacy) live under `data/pg` / `data/redis` and are **not** used when `STACK_MODE=native`.

## Layout

```text
desktop/runtime/
  client.env          # generated
  jwt.secret
  stack.mode          # native | docker
  data/pg-native/     # native PGDATA
  data/redis-native/
  run/ logs/ objects/
  docker-compose.client.yml   # optional fallback only
  bin/avrag-api[.exe] …
  bin/context-os-mcp[.exe]  # stdio MCP for coding agents (forwards to :18080/api/v1/mcp)
  bin/context-os[.exe]      # thin CLI: status / ingest / ask / sources
  bundled/            # portable PG+Redis stage (see bundled/README.md; binaries gitignored)
    pins.env
    windows-x64/      # after stage-desktop-bundled-runtime.sh fetch|assemble
```

Path resolution prefers **install `runtime/pgsql` / `bundled/*` / `native/`** before system packages (`COS_USE_SYSTEM_PG=1` forces system).

## Switching off Docker (this machine)

If cos-client containers still hold ports:

```bash
STACK_MODE=native bash scripts/desktop-local-stack.sh ensure
# script stops cos-client-* containers and starts host pg/redis
```

## Migrations

- **0060** pgvector tables: required.
- **0061** pg_bigm: soft-skipped when extension package missing (CJK lexical may degrade; VGRAG dense/graph OK).

## Product rules

- No cloud account login; 21-day local trial.
- LLM / Embedding: BYOK.
- `RETRIEVAL_BACKEND=pgvector`.
