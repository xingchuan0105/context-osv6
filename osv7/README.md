# osv7

Go backend rewrite (modular monolith) + pi agent runtime. Design: `docs/plans/2026-08-11-osv7-go-rewrite-design.md`.

## Layout

| Path | Role |
|------|------|
| `cmd/retrieval-mcp` | **P1** harness MCP (题卡 + 闸 + dense/lexical/grep + 句柄) |
| `cmd/retrieval-client` | P1 smoke client (stdio MCP) |
| `cmd/hello-retrieval-mcp` | P0 minimal lexical MCP |
| `internal/store` | PG pool only (SQL boundary) |
| `internal/index` | lexical / dense / grep |
| `internal/retrieval` | card, gates, session handles |
| `internal/billing` | capability + usage stub |
| `docs/p0-spike-findings.md` | P0 pi 摸底 |
| `docs/p1-retrieval-findings.md` | P1 检索腿 |

## P1 smoke + full-149 subset

```bash
# requires DATABASE_URL + EMBEDDING_* (source avrag-rs/.env)
bash scripts/p1-retrieval-smoke.sh
go test ./internal/retrieval/ -count=1
# Layer A retrieval vs golden_set_realistic (available needles only)
bash scripts/p1-full149-subset.sh available
```

Optional HTTP: `OSV7_MCP_HTTP_ADDR=:8081 ./bin/retrieval-mcp` (stdio still runs; multi-session map later).

## P2 agentd

```bash
# needs DEEPSEEK_API_KEY (or CHAT_LLM_API_KEY from avrag-rs/.env)
bash scripts/p2-agentd-smoke.sh
bash scripts/p2-harness-smoke.sh   # retrieval tools + HTTP
bash scripts/p2-session-smoke.sh   # multi-turn + PG projection
./bin/agentd-chat -harness -workspace <uuid> "检索问题"
OSV7_AGENTD_ADDR=:8090 ./bin/agentd-server
# POST /v1/chat  POST /v1/chat/stream
# GET  /v1/sessions  GET /v1/sessions/{id}/messages
# GET  /healthz
```

See `docs/p2-agentd-findings.md`.

## P3 ingest

```bash
bash scripts/p3-ingest-smoke.sh
# agent package:
#   bin/ingest-cli agent-package --workspace <uuid> --file ir.json
# server text parse:
#   bin/ingest-cli server-parse --workspace <uuid> --file doc.md
```

See `docs/p3-ingest-findings.md`.

## P4 share + billing

```bash
bash scripts/p4-share-billing-smoke.sh
OSV7_ADDR=:8090 ./bin/osv7d
# POST /v1/billing/topup  GET /v1/billing/wallet
# POST /v1/share  GET /public/s/{token}
```

See `docs/p4-share-billing-findings.md`.

## P0

```bash
bash scripts/p0-hello-retrieval.sh
set -a; source ../avrag-rs/.env; set +a
node scripts/p0-pi-mcp-path.mjs
```

## Package name

`github.com/context-os/osv7` (local module path; not published).
