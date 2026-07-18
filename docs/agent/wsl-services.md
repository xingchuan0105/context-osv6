Status: extracted from AGENTS/CLAUDE for progressive disclosure. AGENTS.md links here.

# WSL Environment, Docker Services & VPS

**Environment**: WSL2 with Docker CE. Core services run via system services or Docker.

## Service status (assume running — do NOT re-verify unless asked)

| Service | Type | Status | Check |
|---------|------|--------|-------|
| **Milvus** | Docker container | Long-running | `docker ps \| grep milvus-standalone` |
| **MinIO (Milvus)** | Docker container | Part of milvus stack | same |
| **etcd (Milvus)** | Docker container | Part of milvus stack | same |
| **PostgreSQL** | System (pg_ctlcluster) | Running | `pg_isready -h 127.0.0.1 -p 5432` |
| **Redis** | System service | Running | `redis-cli ping` |
| **MinIO (Dev)** | Standalone process | On-demand | `scripts/dev-services-up.sh` |

## Rules for agents

1. **Milvus stack (Docker)**: long-running containers (`milvus-standalone`, `milvus-etcd`, `milvus-minio`). **Do not** `docker-compose up` unless a container is actually stopped. Status via `docker ps | grep milvus` (not `docker-compose ps`).
2. **PostgreSQL & Redis**: system services, **not** Docker. `sudo pg_ctlcluster 16 main start` / `sudo service redis-server start` only if confirmed down.
3. **Test PostgreSQL containers** (`avrag-test-pg-*`): intentionally left running between E2E runs. **Do not** stop or prune unless asked.
4. **If a service appears down, check in order**: `docker ps` → `pg_isready -h 127.0.0.1` → `redis-cli ping`. Only then use startup scripts: Milvus `docker-compose -f avrag-rs/docker-compose.milvus.yml up -d`; dev services `bash scripts/dev-services-up.sh`.

## Port bindings (reference — do not remap without user request)

| Service | Port |
|---------|------|
| Milvus gRPC | `127.0.0.1:19530` |
| Milvus metrics REST | `127.0.0.1:19091` |
| PostgreSQL | `127.0.0.1:5432` |
| Redis | `127.0.0.1:6379` |
| MinIO (Dev) API / Console | `127.0.0.1:9000` / `127.0.0.1:9001` |

All connection endpoints are pre-configured in `avrag-rs/.env` (`MILVUS_URL`, `DATABASE_URL`, `REDIS_URL`, `MINIO_ENDPOINT`) — read from there, never ask the user.

## VPS cloud deployment

**The formal deploy path is scripts only.** Do not ad-hoc ssh/scp product code from chat.

- Credentials live in `avrag-rs/.env`: `VPS_MAIN_HOST` / `VPS_MAIN_USER` / `VPS_MAIN_PASSWORD`, `VPS_QDRANT_HOST` / `VPS_QDRANT_USER` / `VPS_QDRANT_PASSWORD`. Read from there; never ask the user, never paste values into docs or chat.
- Publish via `scripts/deploy-frontend.sh`, `scripts/deploy-backend.sh`, `scripts/deploy-public-sites.sh`, `scripts/publish-desktop-release.sh`; status via `scripts/deploy-status.sh`.
- Alignment plan: [`docs/engineering/LOCAL_VPS_ALIGNMENT_PLAN_2026-07-14.md`](../engineering/LOCAL_VPS_ALIGNMENT_PLAN_2026-07-14.md).
- When deploying, always verify service health on the VPS before reporting success.
- Servers: **main** (backend + frontend) and **qdrant** (Qdrant today, Milvus future). IPs are intentionally not repeated here — read `VPS_*_HOST` from `.env` when a script or runbook needs them.
