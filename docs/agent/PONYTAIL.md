Status: extracted from AGENTS/CLAUDE for progressive disclosure. AGENTS.md links here.

# Ponytail coexistence (Context OS monorepo)

The **ponytail** plugin owns generic code minimalism (the YAGNI ladder). This repo's rules own **architecture and ops law**. They coexist under one precedence stack:

```text
1. User's explicit request for this turn
2. This repo's hard rules (Product T1–T8, workspace/org, .env reuse, solo trunk,
   graphify update after structural edits, deploy scripts only, service assumptions)
3. Ponytail minimalism ladder (how to implement within 1–2)
4. Generic style preferences
```

If 2 and 3 conflict: **obey 2**, still take the smallest diff that satisfies 2.

## What ponytail may NOT override here

- **Product hard rules (T1–T8):** minimal code still goes through `conversation()` / `agent()` / `workspace()`; Write stays outside ToolCatalog; no new org/notebook surface.
- **Mandatory bookkeeping:** `graphify update .` after structural edits; persisting user-supplied config to `.env`; deploy via `scripts/deploy-*.sh` only. Ponytail may not skip these as "not needed for the feature."
- **Surgical non-deletion:** only delete dead code **you** introduced or the user asked to remove — this overrides ponytail's "deletion over addition" for unrelated pre-existing code.
- **Package tests:** `cargo test -p …` / `scripts/test-l1.sh` for product paths; ponytail's "one micro-check" is an optional extra, never a substitute.

## Recommended mode

- Default: **full**.
- **ultra**: only when the user explicitly asks (e.g. pure greenfield util).
- Architecture / migration / checklist-driven waves: **full or off** — minimalism must not fight the migration checklist.

## Useful commands (host-dependent)

- `/skill:ponytail-review` after large features: prune overbuild **without** breaking T1–T8.
- `/skill:ponytail-audit` when a crate feels bloated; treat findings as suggestions, repo law decides.

## When to turn ponytail off for a task

- Large regulated refactors with explicit wave plans (e.g. org-removal, workspace rename) where the plan's checklist is the source of truth and minimalism could skip steps.
- When the user asks for maximal explicitness (security-sensitive code, migrations) over brevity.
