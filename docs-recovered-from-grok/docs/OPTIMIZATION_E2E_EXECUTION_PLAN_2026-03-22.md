# context-osv6 Optimization + E2E Execution Plan

> Date: 2026-03-22
> Status: First-pass implementation completed
> Execution rule: follow-up work should now use residual issues from the latest verification pass

## 1. Purpose

This plan updates the current post-delivery work after:

- Rust frontend SSR/hydration recovery
- Share/public-share chat recovery
- chat orchestration migration to `graph-flow`

The goal of this phase is not feature expansion first.

The goal is:

1. simplify the code that became more complex during stabilization
2. reduce repeated frontend hydration-risk patterns
3. simplify the backend chat architecture after graphflow migration
4. add the minimum test coverage needed for safe refactoring
5. run a full E2E acceptance pass after the simplification work

## 2. Current Baseline

As of this plan:

- chat is already migrated to graphflow-backed orchestration by default
- legacy chat orchestrator has been removed
- workspace chat works
- public share chat works
- admin/api-access/share direct-entry hydration regressions were fixed
- search/general work structurally, but provider configuration may still degrade behavior

Remaining work is now mostly:

- simplification
- consolidation
- regression-risk reduction
- full E2E verification

## 2.1 Execution Update

This first execution pass completed the following:

- shared frontend hydration-safe load helper introduced
- remaining high-risk frontend pages/components moved to the shared load pattern
- frontend API base helper simplified to one function
- backend chat orchestration simplified around graphflow/core/postprocess layering
- legacy/placeholder chat fallback paths removed
- `document_fallback` compensation path removed
- `AppError` graphflow bridge simplified without `Box::leak`
- chat graphflow code extracted into `app/src/chat/graphflow.rs`
- chat service/core/postprocess methods extracted into `app/src/chat/service.rs`
- graphflow chat node-level tests added
- focused browser E2E rerun completed for login/dashboard/workspace/search/api-access/share/public-share/admin

## 3. Scope

### In scope

- frontend data-loading simplification
- frontend API helper simplification
- backend chat module simplification after graphflow landing
- graphflow hardening and cleanup
- targeted tests
- full browser E2E acceptance pass

### Out of scope

- new product features
- ingestion pipeline redesign
- admin governance deepening
- billing product redesign
- full RAG quality redesign

## 4. Guiding Principles

1. Prefer removal over abstraction when both preserve behavior.
2. Prefer one shared pattern over many local fixes.
3. Keep HTTP contracts and response shapes unchanged.
4. Keep graphflow as the default chat orchestrator.
5. Delay legacy removal until parity is revalidated.
6. Do not mix large refactors and broad behavior changes in the same step.

## 5. Workstreams

## 5.1 Frontend Hydration / Load Pattern Unification

### Problem

Several pages still trigger network loading directly in component body evaluation using:

- `loaded_*` signal
- conditional branch in render path
- immediate `spawn(...)`

This pattern is the most likely source of hydration regressions.

### Target

Unify all remaining pages/components onto one shared post-hydration loading helper.

### Candidate files

- `/home/chuan/context-osv6/frontend_rust/crates/web-ui/src/routes/search.rs`
- `/home/chuan/context-osv6/frontend_rust/crates/web-ui/src/routes/dashboard.rs`
- `/home/chuan/context-osv6/frontend_rust/crates/web-ui/src/routes/settings.rs`
- `/home/chuan/context-osv6/frontend_rust/crates/web-ui/src/routes/invite.rs`
- `/home/chuan/context-osv6/frontend_rust/crates/web-ui/src/components/billing/mod.rs`
- `/home/chuan/context-osv6/frontend_rust/crates/web-ui/src/components/document/mod.rs`

### Planned action

1. Introduce one shared helper or hook for:
   - compute load key
   - skip if already loaded
   - defer client fetch until hydration-safe moment
2. Replace local ad hoc patterns in the files above.
3. Keep the request behavior and UI output unchanged.

### Acceptance criteria

- No component-body direct fetch trigger remains in the target files.
- Direct navigation to those pages does not produce fatal hydration errors.
- Data still loads correctly after hydration.

## 5.2 Frontend API Helper Simplification

### Problem

`same_origin_api_base_url()` and `localhost_api_base_url()` currently behave the same.

### Target

Collapse to a single frontend API base helper.

### Candidate file

- `/home/chuan/context-osv6/frontend_rust/crates/web-ui/src/api.rs`

### Planned action

1. Replace the two helper functions with one canonical `api_base_url()`.
2. Update all call sites.
3. Preserve relative-path behavior for same-origin deployment.

### Acceptance criteria

- Only one frontend API base helper remains.
- No SSR/CSR text mismatch caused by differing base URL rendering.

## 5.3 Backend Chat Simplification After Graphflow Migration

### Problem

The backend originally had:

- graphflow orchestrator
- legacy orchestrator
- mode-specific `*_core` helpers
- mode-specific full execution helpers
- large chat-related code concentrated in `app/src/lib.rs`

This works, but it is not yet simple.

### Target

Move from "graphflow landed" to "graphflow-centered and maintainable."

### Candidate files

- `/home/chuan/context-osv6/avrag-rs/crates/app/src/lib.rs`
- `/home/chuan/context-osv6/avrag-rs/crates/app/src/chat_graphflow.rs`

### Planned action

Phase A: internal cleanup

1. Continue moving chat helpers into dedicated chat submodules.
2. Separate:
   - preflight
   - mode execution
   - post-processing
   - graphflow orchestration
3. Remove unvalidated alternate orchestration paths.

Phase B: graphflow-centered architecture

4. Cache graph construction instead of rebuilding per request.
5. Reduce stringly-typed graph context access where feasible.
6. Simplify error bridging between graphflow and `AppError`.

Phase C: legacy retirement readiness

7. Keep only one orchestrator path unless a real production-proven fallback is introduced later.

### Acceptance criteria

- chat code is no longer primarily concentrated in one giant `lib.rs` region
- graphflow remains default
- pseudo-fallbacks and placeholder alternates are removed
- no response-shape regression in `rag/general/search`

## 5.4 Test Coverage Improvement

### Problem

Graphflow migration is functionally live, but node-level coverage is still thin.

### Target

Add the minimum set of tests needed to support simplification safely.

### Planned tests

1. Unit tests
   - graphflow preflight node
   - mode selection node
   - output-guard/postprocess behavior

2. Integration tests
   - `rag` graphflow parity
   - `general` graphflow parity
   - `search` graphflow parity
   - share-token chat path

3. Frontend regression checks
   - direct-load SSR pages that previously mismatched

### Acceptance criteria

- graphflow path has explicit coverage for mode routing and post-processing
- share-token chat path is covered
- hydration-sensitive pages have repeatable regression checks

## 5.5 Full E2E Acceptance Pass

### Goal

Validate the post-simplification build with a realistic user lifecycle.

### User lifecycle checklist

1. Register or log in
2. Create notebook
3. Upload source document
4. Verify source status becomes completed
5. Ask workspace RAG question
6. Verify answer + citation + evidence panel
7. Use search mode
8. Use general mode
9. Open API Access
10. Create/revoke API key
11. Open Share Center
12. Generate share link
13. Open public share page
14. Ask public share question
15. Open admin pages
16. Verify unauthorized-user admin UX remains non-fatal

### Acceptance criteria

- No fatal hydration errors on target pages
- Workspace chat returns answer + source + citation
- Public share chat returns answer + source + citation
- API Access and Share Center direct entry work
- Admin direct entry does not panic

## 6. Execution Order

The implementation order after confirmation should be:

1. Create shared frontend hydration-safe load helper
2. Migrate remaining frontend pages/components to it
3. Simplify frontend API base helper
4. Continue backend chat modularization around graphflow
5. Improve graphflow error/context simplification
6. Add tests
7. Run full E2E pass
8. Produce final delta report and remaining-risk summary

## 7. Risk Controls

1. Keep each workstream behavior-preserving unless explicitly called out.

2. Treat provider-related degradation separately from code breakage:
   - missing search provider config
   - missing answer LLM config
   are not the same as application regression

## 8. Deliverables

After implementation, the expected outputs are:

- simplified frontend loading pattern
- simplified frontend API helper layer
- graphflow-centered chat architecture with reduced duplication
- new or updated tests
- updated E2E report
- final residual-risk summary

## 9. Confirmation Gate

This document is prepared and ready.

No implementation should start until the user explicitly confirms:

- execute the plan as written
- or reprioritize / trim specific workstreams
