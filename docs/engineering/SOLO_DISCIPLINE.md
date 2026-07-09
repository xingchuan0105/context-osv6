# Solo Engineering Discipline

**Audience:** single primary developer (plus AI agents) on a large monorepo.  
**Status:** active default (2026-07-09).  
**Related:** [E2E quality gates](../../avrag-rs/docs/e2e-gates.md), `.github/workflows/`.

This is the project’s default way of working. It fuses industry practice (test pyramid, fast commit-stage CI, trunk-based small-team norms, monorepo selective builds, flaky-test quarantine) with solo constraints: **process complexity scales with headcount**, not with repo size.

---

## One-line rule

> **Default: local trunk.** Code lives on disk; verify locally. Push to GitHub only for backup or deploy. Do not treat remote PR/CI as the daily development loop.

---

## Default workflow: local trunk (chosen)

Trunk branch: **`master`** (local). Primary machine: WSL path `/home/chuan/context-osv6`.

| Step | Where | What |
|------|--------|------|
| Edit | Disk | Implement on `master` (or a short local branch you merge the same day) |
| Record | Local git | `git commit` on this machine — history does **not** require GitHub |
| Verify | Local | Targeted tests only (see below). **No push required.** |
| Backup | GitHub (optional) | Occasional `git push origin master` at milestones, tags, or before risky ops |
| Deploy | As you prefer | scp/rsync/VPS scripts; not “must pass GitHub PR checks first” |

### Agents must follow

* **Do not** start with “push and wait for CI” unless the user explicitly asks for remote backup or a PR.
* **Do not** open PRs, re-run Actions, or babysit GitHub checks as the default progress loop.
* **Do** run focused **local** verification after changes (`cargo test -p …`, `pnpm test`, typecheck).
* **Do** commit locally when the user wants history; push only when asked or at an agreed milestone.
* Remote CI workflows remain **assets** (manual/`workflow_dispatch`, future multi-dev). They are not the daily gate.

### Local verify (commit stage — on this machine)

```bash
# Frontend (if frontend_next changed)
cd frontend_next && pnpm test run

# Rust (pick packages you touched)
cd avrag-rs && cargo test -p <crate> --lib
```

Acceptance (smoke/E2E): wave end or pre-ship only — local scripts or `workflow_dispatch`, not every commit.

---

## Pipeline stages (when you use automation at all)

| Stage | Purpose | Default place | Examples |
|-------|---------|---------------|----------|
| **Commit** | “Did this change break known contracts?” | **Local** | Affected `cargo test`, Vitest, typecheck |
| **Acceptance** | Smoke / product paths | Local script or manual CI, wave end | Product/Frontend smoke |
| **Release** | Prod-like proof | Before ship | Real LLM, rag quality |

If GitHub Actions is used, commit-stage jobs should stay fast. Slow/flaky suites stay off the daily path.

---

## Agent / human operating rules

1. **Touch only what the task needs** (surgical changes; YAGNI).
2. **Verify at the right layer — locally first**
   - Feature/bug: unit or focused crate/frontend tests on disk.
   - Do **not** treat Product/Frontend smoke as blockers mid-wave.
3. **E2E debt is deferred** during feature/architecture waves; restabilize at wave end unless the user asks now.
4. **GitHub is optional**
   - Push = backup / multi-machine / deploy source — not “development completed.”
   - PRs = optional changelog or future collab — not required to code.
5. **Selective work**: path-aware local tests; avoid full monorepo smoke by default.
6. **Flaky acceptance**: quarantine; fix when shipping depends on it.
7. **Toolchain upgrades**: prefer separate commits; still local-first.
8. **If user opens a PR anyway**: do not expand into multi-hour E2E CI campaigns; prefer local repro.

---

## GitHub workflows (reference only)

These exist for optional remote runs / future use — **not** the solo daily loop:

| Workflow | Role under local trunk |
|----------|-------------------------|
| Frontend Vitest / License | Optional remote recheck |
| Product / Frontend smoke | Manual only (`workflow_dispatch`); deferred as PR gates |
| Desktop Shell | Only if you care about desktop path remotely |

Details: [`avrag-rs/docs/e2e-gates.md`](../../avrag-rs/docs/e2e-gates.md).
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
| 2026-07-09 | **Default = local trunk** (`master` on disk); push only for backup/deploy |
