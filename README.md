# Context-OS

**Workspace-centric knowledge base · RAG · agents**

把文档放进 workspace，用检索与 Agent 答得出、写得出。

| | |
|--|--|
| 产品 | [contextlm.top](https://contextlm.top) · [应用](https://app.contextlm.top) · [客户端](https://app.contextlm.top/desktop) |
| 作者 | [@xingchuan0105](https://github.com/xingchuan0105) |

## What’s in this monorepo

| Path | Role |
|------|------|
| `avrag-rs/` | Rust backend — API, worker, RAG, agent loop, ingestion |
| `frontend_next/` | Next.js product UI |
| `desktop/` | Tauri client (Windows-first) |
| `prompts/` (under `avrag-rs/`) | LLM-facing prompts (source of truth; not hardcoded in Rust) |
| `docs/` | Architecture, engineering, design baseline |
| `scripts/` | Local verify, deploy, E2E helpers |

## Stack (high level)

- **Backend:** Rust, PostgreSQL, Redis, object storage; retrieval via **pgvector** or **Milvus**
- **Agent:** single-agent ReAct loop; retrieval/tools via host + sandbox
- **Frontend:** Next.js + React + TypeScript (pnpm)
- **Desktop:** Tauri + optional bundled local stack

## Development notes

- **Source of truth:** local trunk `master` (solo workflow). See `docs/engineering/SOLO_DISCIPLINE.md`.
- **Agent rules for this repo:** [`AGENTS.md`](AGENTS.md)
- **Deploy:** only `scripts/deploy-*.sh` (not ad-hoc scp)
- **Docs index:** [`docs/README.md`](docs/README.md)

```bash
# Backend (from avrag-rs/)
cargo build -p avrag-api -p avrag-worker

# Frontend
cd frontend_next && pnpm install && pnpm dev
```

Services and ports: `docs/agent/wsl-services.md`. Credentials live in `avrag-rs/.env` (never commit secrets).

## License

See repository `LICENSE` / `THIRD_PARTY_NOTICES.md` as applicable.
