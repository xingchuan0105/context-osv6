# Solo Engineering Discipline

**Audience:** single primary developer (plus AI agents) on a large monorepo.  
**Status:** active default (2026-07-09).  
**Related:** [E2E quality gates](../../avrag-rs/docs/e2e-gates.md), `.github/workflows/`.

This is the project’s default way of working. It fuses industry practice (test pyramid, fast commit-stage CI, trunk-based small-team norms, monorepo selective builds, flaky-test quarantine) with solo constraints: **process complexity scales with headcount**, not with repo size.

---

## One-line rule

> Keep the trunk trustworthy with **fast, stable** checks. Keep **expensive E2E as assets** on a later stage. Prefer path-selective work. Do not build enterprise PR theater for one person.

---

## Pipeline stages

| Stage | Purpose | When | Examples |
|-------|---------|------|----------|
| **Commit** | “Did this change obviously break known contracts?” | Every push / PR | Affected `cargo test` / `cargo check`, Frontend Vitest / typecheck, license gate |
| **Acceptance** | Smoke / product paths still hold | Wave end, pre-demo, `workflow_dispatch` | Product smoke (`smoke-e2e.yml`), Frontend smoke (`frontend-smoke.yml`) |
| **Release** | Costly or prod-like proof | Before ship | Real LLM, rag quality, release-e2e-gate |

Commit stage should finish in roughly **≤15 minutes**. If a check is slow, flaky, or environment-heavy, it is **not** a commit gate.

---

## Agent / human operating rules

1. **Touch only what the task needs** (surgical changes; YAGNI). Do not expand scope to “fix all CI.”
2. **Verify at the right layer**
   - Feature/bug: unit or focused crate/frontend tests first.
   - Do **not** treat Product/Frontend smoke as merge blockers while architecture or product surface is still moving.
3. **E2E debt is deferred on purpose during feature waves**
   - Restabilize smoke/E2E at **wave end**, not mid-ADR unless the user asks.
   - Prefer documenting debt over drive-by CI rewrites that change product semantics.
4. **Selective monorepo CI**
   - Prefer path filters and affected packages over full-workspace smoke on every PR.
   - Avoid stacking many `workflow_dispatch` runs on the same SHA.
5. **Flaky or red acceptance suites**
   - Quarantine (manual/nightly only) beats forced-green patches that hide real breakage.
   - Fix the suite when the product surface stabilizes, or when shipping depends on it.
6. **Branch / PR**
   - Short feature branches or direct trunk are both fine for one developer.
   - PRs are optional backups / review diffs, not a multi-party approval ritual.
   - Required GitHub checks should stay **few** (fast commit-stage only). Do not re-add smoke as required without an explicit decision.
7. **Toolchain upgrades**
   - Prefer a dedicated commit/branch. Do not mix major toolchain bumps with large product ADRs unless necessary.
8. **When CI is red**
   - Diagnose; stop before large fixes if the user asked for confirmation.
   - Prefer re-running only the failing **commit-stage** job.
   - Do not expand into full E2E campaigns mid-feature unless requested.

---

## Default PR / push expectations

**Should stay green (commit stage):**

- Frontend Unit Tests (Vitest) when `frontend_next/**` changes
- License / legal gates when those paths change
- Desktop Shell Check when `desktop/**` changes (path-filtered)
- Local: targeted `cargo test` / `pnpm test` / typecheck for edited packages

**Not PR merge gates (acceptance; manual until re-enabled):**

- Product Smoke E2E (`smoke-e2e.yml` → Product job): `workflow_dispatch` only
- Frontend Smoke E2E (`frontend-smoke.yml`): `workflow_dispatch` only

Details and suite semantics: [`avrag-rs/docs/e2e-gates.md`](../../avrag-rs/docs/e2e-gates.md).

---

## Industry sources (compressed)

| Idea | Practice |
|------|----------|
| Test pyramid | Many unit, few E2E; E2E not every commit |
| Fast commit CI | Feedback in minutes or people ignore it |
| Quarantine flaky tests | Untrusted suites leave the main gate |
| Trunk-based (small team) | Short branches or push to trunk; heavy gates for large teams |
| Monorepo selective CI | Build/test what changed |

This doc is the **project-local** synthesis; it is not a copy of any single vendor guide.

---

## Re-enabling PR smoke (later)

When a product wave settles and smoke is green on `workflow_dispatch`:

1. Restore `pull_request` triggers on `frontend-smoke.yml` / Product job in `smoke-e2e.yml` only if commit-stage remains fast.
2. Prefer **non-required** PR status first; promote to required only if the suite is stable for weeks.
3. Update this file and `e2e-gates.md` in the same change.

---

## Change log

| Date | Note |
|------|------|
| 2026-07-09 | Initial solo discipline; product/frontend smoke deferred from PR |
