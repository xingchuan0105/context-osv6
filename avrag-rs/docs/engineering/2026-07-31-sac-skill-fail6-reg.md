# SaC knowledge-base skill — fail-6 regression harness (P2)

**Date:** 2026-07-31  
**Scope:** After editing `prompts/clusters/knowledge-base/**` (and related progressive table reference), run a **six-question subset** before golden-14. No golden query text is stored here — indices only.

## Why

Skill edits that target table order, `total_hits`, multi-claim coverage, and joint sources should not require a full 14- or 149-question run mid-iteration. The product E2E already supports `E2E_QUESTIONS` (1-based indices into `golden_set_realistic.json` example order).

## Default subset

| # | Fail-mode family (skill-side) |
|---|-------------------------------|
| 65 | High-variance / answer vs refuse boundary |
| 86 | Table sort-key sticky (label ≠ order key) |
| 88 | Multi-count / `total_hits` vs row sample |
| 105 | Cross-doc similarity distraction |
| 106 | Multi-claim half-coverage sticky |
| 121 | Dual-source (KB + web) joint |

Script default: `65,86,88,105,106,121`.

## How to run

```bash
# full fail-6 (script adds --features product-e2e -- --ignored --test-threads=1)
bash avrag-rs/scripts/sac-skill-fail6-reg.sh

# sticky-only slice while editing table ontology
QUESTIONS=86,106 bash avrag-rs/scripts/sac-skill-fail6-reg.sh

# print env only
DRY_RUN=1 bash avrag-rs/scripts/sac-skill-fail6-reg.sh
```

Prerequisites: services per `docs/agent/wsl-services.md`; credentials in `avrag-rs/.env` (script sources it; never commit secrets). Respect WSL `jobs=2` — do not stack concurrent full `cargo test` runs.

## Read results

- Cargo `test result: ok` means harness did not panic — **not** v2 PASS rate.
- Judge-first lines: `v2: label=PASS|…` in the log under `/tmp/sac_e2e/fail6_*.log`.
- Artifacts: `avrag-rs/crates/app/tests/e2e_output/rag_eval_v2/v2_*`.

## Skill edit loop (eval-first)

1. Reproduce on fail-6 (or sticky slice).
2. Patch skill **gotchas / contrast examples / multi-claim checklist** only — no golden entity names in prompts.
3. Re-run fail-6.
4. On green-enough sticky (86/106), optional golden-14.

Related prompts: `prompts/clusters/knowledge-base/SKILL.md` v4.1+, `reference/how-to-read-tables.md`.
