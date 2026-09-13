# Agent rules — context-osv6

## Scope and precedence

This file contains repository-wide rules. Before working in a subtree, read its `AGENTS.md` and the task-specific references below, even when starting at the repository root.
Within repo guidance: explicit user instructions > design principles > repo/domain rules > generic preferences. Historical plans and human-reference essays do not override current rules.

## Design principles

- **No backward compatibility tax.** Remove obsolete paths instead of adding compatibility layers, fallbacks, or migrations. Keep deletion within the requested change.
- **Simplest that fully works.** Avoid speculative abstractions, configuration, and indirection; choose a durable architecture, not a stopgap intended for replacement.
- **Layered growth.** Start with a working end-to-end slice, verify it, then add capabilities. Do not replace a working product with unfinished complexity.
- **Modular separation.** Keep concerns separate and interfaces focused.
- **Reuse before building.** Check existing dependencies, their documentation and types first; prefer proven libraries when they reduce overall complexity or improve reliability.

## Working agreement

- State material assumptions and proceed with routine, reversible implementation choices. Ask when ambiguity changes the product goal, scope, or an irreversible decision; suggest a simpler option when warranted.
- Make surgical edits, match local style, and preserve unrelated working-tree and staged changes.
- For multi-step work, give a brief plan with verification gates; do not advance past a failing gate.
- **Time-cost consent:** announce expected duration before builds/tests/scripts. Short local checks within the requested scope may proceed. Obtain approval for long or costly runs (full builds, full E2E, real-LLM evaluations) or actions affecting running services, unless already authorized.
- Verify at the layer affected by the change. Documentation-only edits need content, link, and diff checks; they do not require compilation or a code graph update.
- Work on local trunk `master` and commit the task's changes locally. No push, PR, or CI monitoring unless requested. Details: [solo discipline](docs/engineering/SOLO_DISCIPLINE.md).

## Shared product boundaries

- **Cloud workbench:** `workspace` is the sole reusable/manageable/shareable persistent-knowledge container. User-owned conversations may have no workspace; do not add notebook/global-KB alternatives.
- **Subtex plugin:** the user's project directory is the sole master; indexes are derived, disposable, and rebuildable. No ingest copies.
- **Ownership:** root ownership is `user_id` / `owner_user_id`, never product `org`. Conversation resources may use `conversation_id`; only workspace-bound resources require `workspace_id`.
- **LLM-facing prose:** author only under `avrag-rs/prompts/**/*.md`, using third-person observations of the environment. No hardcoded instruction prose or host diagnostics appended to the user's answer.
- **No golden-set leakage:** do not put corpus queries, gold answers, entity names, or evaluation numbers into product prompts, runtime policy, or fixtures presented as product policy.

## Code intelligence

- For code structure, relationships, or blast radius, query **code-review-graph first**. After structural code edits, run `code-review-graph update` in the same session before claiming completion.
- Repo-wide pattern search: `tgrep`; semantic chunks: `semble`; one-shot exact strings: `rg` / `grep`. Usage and index maintenance: [code intelligence](docs/agent/code-review-graph.md).
- Never commit `.code-review-graph/` or `.tgrep/`.

## Environment and operations

- When a task needs credentials/configuration, read the relevant entries in `avrag-rs/.env` and `avrag-rs/.env.example`; reuse values silently, never re-ask or expose secrets. Persist user-supplied new values incrementally.
- Assume existing services are running; investigate when the task or a failure requires it. Do not blindly run `docker-compose up` or stop/prune `avrag-test-pg-*` containers.
- Deploy product code only through `scripts/deploy-*.sh` and the formal release scripts in [operations](docs/agent/wsl-services.md); never ad-hoc ssh/scp. Cloud host: `VPS_MAIN_*` only; do not use or reintroduce `VPS_QDRANT_*`.
- WSL Rust builds use `jobs=2`; do not stack concurrent full Cargo builds/tests.

## Task routing

Read only the rows relevant to the task, before editing or executing in that area. Follow additional `AGENTS.md` files along the target path.

| Task | Required guidance |
|---|---|
| Backend `avrag-rs/` | [backend rules](avrag-rs/AGENTS.md) |
| Next.js frontend `frontend_next/` | [frontend rules](frontend_next/AGENTS.md) |
| Product navigation, shell, or monetization entries | [Product IA](docs/design/PRODUCT_IA.md) |
| Product prompts or harness guidance, including callers outside the backend | [LLM guidance](docs/agent/llm-guidance.md) |
| Subtex features | [PRD](docs/plans/2026-09-06-directory-plugin-prd.md); progress and gates: [development plan](docs/plans/2026-09-06-subtex-m1-dev-plan.md) |
| Rust builds, targets, or cache changes | [resource policy](docs/agent/rust-resources.md) |
| E2E / real-LLM evaluation | [E2E gates and long-run conventions](avrag-rs/docs/e2e-gates.md) |
| VGI-RAG (`docs/vgi/`, `vgi-rs`) | [VGI design-doc DAG](docs/vgi/AGENTS.md) — docs are the source of truth; regenerate from docs |
| Service diagnosis, environment changes, or deployment | [operations](docs/agent/wsl-services.md) |

## Repository map

- WSL root: `/home/chuan/context-osv6`; Windows can access the same checkout through WSL.
- Backend: `avrag-rs/`; Next.js/React/TypeScript frontend: `frontend_next/`.
- `frontend_rust/` work requires an explicit request. Desktop implementations live in `desktop/` and `desktop_gpui/`; select the requested implementation rather than inferring a migration task.
- Current and historical document index: [docs/README.md](docs/README.md). Keep migration progress, examples, and external-repository tools out of this file.
