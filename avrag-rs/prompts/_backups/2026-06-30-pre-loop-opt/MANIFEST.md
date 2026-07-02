# Prompt Backup — 2026-06-30 pre loop optimization

Snapshot taken before iterative RAG loop / prompt tuning.

## Git context

- **Commit (short):** `491c077`
- **Branch:** `master`
- **Working tree:** prompts had uncommitted edits at backup time (see `git status` in repo)

## What is backed up

Aligned with `modes/rag.yaml` skill catalog for RAG mode:

| Path | Role |
|------|------|
| `orchestrators/rag-system.md` | RAG system prompt (orchestrator, v5.0) |
| `synthesis/rag-answer.md` | Mandatory synthesis contract |
| `synthesis/grounded-answer.md` | `rag-answer` dependency |
| `clusters/codegen/` | Mandatory retrieve skill (sandbox SDK) |
| `clusters/memory/` | Optional retrieve skill + `reference/anaphora.md` |
| `clusters/metadata/` | Optional retrieve skill |
| `clusters/writing/` | Optional synthesis skill + references |
| `clusters/format/` | Optional synthesis skill + references |
| `modes/rag.yaml` | Mode config snapshot (catalog, budget, synthesis contract) |

**Not included:** deprecated atomic-tools, chat/search orchestrators, pipeline prompts — out of scope for RAG loop optimization.

## Restore (single file)

```bash
cp avrag-rs/prompts/_backups/2026-06-30-pre-loop-opt/orchestrators/rag-system.md \
   avrag-rs/prompts/orchestrators/rag-system.md
```

## Restore (full RAG prompt set)

```bash
BACKUP=avrag-rs/prompts/_backups/2026-06-30-pre-loop-opt
cp "$BACKUP/orchestrators/rag-system.md" avrag-rs/prompts/orchestrators/
cp "$BACKUP/synthesis/"*.md avrag-rs/prompts/synthesis/
for c in codegen memory metadata writing format; do
  rm -rf "avrag-rs/prompts/clusters/$c"
  cp -r "$BACKUP/clusters/$c" avrag-rs/prompts/clusters/
done
cp "$BACKUP/modes/rag.yaml" avrag-rs/modes/rag.yaml
```

After restore, rebuild if needed: `cargo build -p app-chat`

## File list

See `FILES.txt` (21 files).
