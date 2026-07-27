# Domain Documentation

Rules for reading and maintaining domain knowledge.

## Layout

- **Global Context**: `CONTEXT.md` at the repo root.
- **Decision Records**: `docs/adr/` directory.

## Consumer Rules

1. **Alignment First**: Before starting a task, check `CONTEXT.md` for ubiquitous language and `docs/adr/` for relevant past decisions.
2. **Update on Decision**: When a new architectural decision is made, create a new ADR under `docs/adr/`.
3. **Refine Context**: If a new domain term is introduced, add it to `CONTEXT.md`.
