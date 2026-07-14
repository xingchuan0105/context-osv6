# Context-OS Client local data plane

Full **Milvus** + PostgreSQL + Redis for the desktop client (vector knowledge graph requires Milvus).

## Quick start

```bash
# monorepo root
bash scripts/desktop-local-stack.sh up
bash scripts/desktop-local-stack.sh status
```

Ports (localhost only):

| Service    | Port  |
|------------|-------|
| PostgreSQL | 5433  |
| Redis      | 6380  |
| Milvus     | 19530 |
| Milvus metrics | 19091 |

## Client process env (optional)

```bash
export DATABASE_URL=postgres://avrag:avrag@127.0.0.1:5433/avrag_client
export REDIS_URL=redis://127.0.0.1:6380/0
export MILVUS_URL=http://127.0.0.1:19530
export CLIENT_PG_PORT=5433
export CLIENT_REDIS_PORT=6380
export CLIENT_MILVUS_PORT=19530
```

## Product rules

- **No cloud account login** in the client.
- **21-day local trial** (device-bound file).
- LLM / Embedding: BYOK in client settings.
- Data plane is **on-machine** via this compose stack.

## Stop

```bash
bash scripts/desktop-local-stack.sh down
```
