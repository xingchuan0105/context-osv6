# Context-OSV6 Architecture Governance And Contract Convergence Design

> Scope freeze: this document governs boundary cleanup, contract convergence, and runtime simplification only. It does not introduce new product features.
> 2026-04-26 update: Qdrant references in this governance document mean "retrieval adapter boundary" historically. The current retrieval target is Milvus; see [Current Product Architecture](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md).

## Goal

Reduce system uncertainty caused by protocol drift, overloaded application wiring, and unclear runtime boundaries, while preserving the existing healthy core in `rag-core` and ingestion routing.

## Non-Goals

- No rewrite of retrieval, reranking, parsing, or synthesis internals
- No expansion of unfinished product capabilities currently returning `501`
- No compatibility bridge for unknown third-party clients
- No attempt to make `memory mode` production-equivalent

## Problem Summary

The system's main architectural risk is not in the core RAG or ingestion layers. The highest risk sits in the delivery shell around them:

- Chat contract drift across backend routing, SSE event shape, frontend SDK, UI expectations, and docs
- `AppState` acting as a god object that mixes runtime wiring, business flow, storage branching, and fallback behavior
- `transport-http` coupling middleware, auth, SSR, routing, business handlers, and stubs in one boundary
- Manually mirrored DTOs across backend and frontend, allowing silent type drift
- Archived frontend crates still present in the backend workspace, increasing cognitive and build complexity
- `memory mode` embedded in core application flow as a runtime branch instead of a dev/test adapter
- Process-local rate limiting that is not valid for multi-instance deployment

## Architectural Principles

This redesign is governed by three principles:

1. Contract first
   The chat protocol and shared DTOs are the primary source of truth. Implementations follow the contract, not the reverse.

2. Application services mediate domain flow
   HTTP handlers should delegate to focused application services. Runtime wiring should not leak into business APIs.

3. Infrastructure is replaceable
   PostgreSQL, Redis, Qdrant, object storage, and in-memory adapters sit behind explicit interfaces where they influence application flow.

## Decided Constraints

The following decisions are fixed for this redesign.

### 1. Unified Chat Entry Point

`POST /api/v1/chat` becomes the single official chat entry point.

- If `stream = false`, the endpoint returns a complete JSON `ChatResponse`.
- If `stream = true`, the endpoint returns an SSE stream.
- `GET /api/v1/chat` is not part of the target contract.
- `Accept: text/event-stream` may be validated for correctness, but request-body `stream` is the authoritative switch.

Rationale:

- Product semantics stay stable: "send a chat request" always means one endpoint.
- Frontend, SDK, contract tests, and docs only maintain one entrypoint.
- The implementation avoids split evolution between GET-streaming and POST-JSON behaviors.

### 2. Simplified SSE Contract

The official SSE event set is:

- `start`
- `trace`
- `token`
- `citations`
- `done`
- `error`

The stream rules are:

- Every stream must begin with `start`
- Zero or more `trace` events may appear at any point after `start`
- Zero or more `token` events may appear after `start`
- Zero or more `citations` events may appear after `start`
- Every stream must terminate with exactly one of `done` or `error`
- Event payloads are JSON objects only

Fields required for event handling:

- `start`: must include `request_id` and `session_id`
- `trace`: must include `request_id`, `stage`, `status`; optional detail fields allowed
- `token`: must include `request_id`, `message_id`, `content`
- `citations`: must include `request_id`, `message_id`, `citations`
- `done`: must include `request_id`, `session_id`, `message_id`, final answer payload
- `error`: must include `request_id`, stable `code`, user-safe `message`

Outcomes:

- Existing debug-specific events such as planner- or RAG-specific event names are folded into `trace` or the terminal `done` payload.
- The frontend debug panel remains possible, but protocol evolution becomes much safer.

### 3. Single Source Of Truth For Contracts

A dedicated shared Rust crate will be introduced, referred to in this document as `crates/contracts`.

This crate owns:

- request DTOs
- response DTOs
- chat event DTOs
- serialization and deserialization rules
- stable error envelopes used across frontend and backend

Constraints:

- Backend HTTP handlers must consume and emit contract types from this crate.
- Frontend `web-sdk` must consume the same crate rather than maintain mirrored DTOs.
- OpenAPI, if retained, is generated from or aligned to this crate. It is not a competing source of truth.

Outcome:

- Type drift becomes a compile-time failure instead of a late runtime discovery.

### 4. Scope Freeze For Unfinished Capabilities

Any frontend feature that currently depends on backend `501 Not Implemented` endpoints is removed from the visible UI or hidden behind a feature gate for this redesign window.

Constraints:

- This redesign does not implement those missing endpoints.
- UI must not expose affordances that knowingly dead-end at `501`.

Outcome:

- The system stops pretending unfinished functionality is available.
- The redesign stays bounded around architecture instead of silently expanding into feature delivery.

### 5. Archived Frontend Removal

The archived `avrag-rs/crates/web-sdk` and `avrag-rs/crates/web-ui` crates are not part of the target architecture.

Removal policy:

1. Remove them from the `avrag-rs` workspace membership first
2. Verify the build and dependency graph
3. Delete the archived directories once no active dependency remains

Outcome:

- The production frontend lives in one place: `frontend_rust/`
- Build and ownership boundaries become easier to understand

### 6. Memory Mode Is Dev/Test Only

`memory mode` is retained, but only as a development and test adapter.

Target semantics:

- It is not a production runtime mode
- It is not expected to mimic full PostgreSQL/Qdrant/worker behavior
- It exists to support lightweight local development, smoke testing, and UI/API bootstrapping without full infrastructure

Implementation consequence:

- `memory mode` must stop being a pervasive `if pg { ... } else { memory ... }` branch pattern in core application services
- It should instead implement explicit application-facing ports used in dev/test environments only

Outcome:

- Production behavior becomes single-path
- Local ergonomics are preserved without contaminating production design

### 7. Lockstep Internal Release

This redesign assumes no critical external client compatibility obligation.

Release policy:

- Backend and frontend may ship together in lockstep
- No temporary dual-contract bridge is required
- If future external clients are identified, compatibility policy must be revisited in a separate RFC

### 8. CI Is A Governance Tool, Not A Future Nice-To-Have

Contract integrity and runtime behavior become merge blockers during this redesign.

Mandatory CI gates are defined later in this document.

## Target Architecture

## 1. Layered Runtime Shape

The target runtime is:

- `contracts`
  Shared transport and event types

- `transport-http`
  HTTP routing, auth extraction, request validation, SSE framing, SSR integration

- application services
  Focused service layer such as `ChatService`, `WorkspaceService`, `DocumentService`, `AuthService`, `ShareService`

- domain/runtime modules
  Existing healthy modules such as `rag-core`, ingestion parser routing, guardrails, search execution

- infrastructure adapters
  PostgreSQL repositories, Qdrant backend, Redis limiter, object storage, memory dev/test adapters

The service layer becomes the only place where transport concerns are translated into business operations.

## 2. Chat Flow

The target chat flow is:

1. `transport-http` validates auth and request shape
2. request is deserialized from the shared `contracts` crate
3. handler calls `ChatService`
4. `ChatService` performs preflight, session resolution, mode selection, and orchestration
5. GraphFlow and `rag-core` remain internal execution mechanisms, not transport contract owners
6. service emits contract-typed stream events or final JSON response
7. `transport-http` frames those events as SSE when `stream = true`

Important boundary:

- SSE framing belongs to transport
- event semantics belong to contracts
- orchestration belongs to application services

## 3. Storage And Runtime Ports

Introduce explicit ports where runtime branching currently leaks into `AppState`.

Expected ports include:

- `WorkspaceStore`
- `DocumentStore`
- `ChatStore`
- `ShareStore`
- `UsageLimitStore`
- `RateLimiter`

Production adapters:

- PostgreSQL-backed stores
- Redis-backed limiter

Non-production adapters:

- memory-backed stores for dev/test only

Important boundary:

- `rag-core` should depend on retrieval-facing capabilities, not a monolithic all-purpose repository

## 4. Frontend Boundary

The target frontend shape is:

- `frontend_rust/crates/web-sdk`
  Thin transport client over the shared `contracts` crate

- `frontend_rust/crates/web-ui`
  UI state and presentation only

Constraints:

- no mirrored transport structs owned by the frontend
- no dependency on archived crates
- no UI path leading to unsupported backend routes

## 5. Rate Limiting

Rate limiting moves from process-local memory to Redis-backed shared state.

Business outcome:

- consistent behavior across multi-instance deployments
- correct enforcement regardless of which API node receives the request

This redesign does not require changing product quota semantics. It only moves enforcement to a deployable architecture.

## 6-Week Execution Plan

## Phase 1: Contract Convergence And Surface Cleanup

### Week 1

Deliverables:

- create shared `contracts` crate
- define official `ChatRequest`, `ChatResponse`, error envelopes, and `ChatEvent`
- switch backend chat transport to the shared contract types
- switch frontend `web-sdk` to the shared contract types
- remove archived frontend crates from workspace membership
- hide or gate all UI paths that currently land on backend `501`

Success criteria:

- one canonical chat request type
- one canonical SSE event enum
- no active frontend path knowingly calls a `501` backend endpoint

## Phase 2: HTTP Boundary And AppState Decomposition

### Weeks 2-3

Deliverables:

- split `transport-http` into focused modules:
  - `middleware`
  - `auth`
  - `chat`
  - `notebooks`
  - `documents`
  - `share`
  - `infra`
- reduce `AppState` to dependency container and runtime bootstrap only
- introduce focused application services:
  - `ChatService`
  - `WorkspaceService`
  - `DocumentService`
  - `AuthService`
  - `ShareService`

Success criteria:

- handlers delegate to services rather than owning business logic
- `AppState` no longer contains large numbers of direct business methods
- chat SSE behavior is implemented from service output, not ad-hoc handler assembly

## Phase 3: Storage Ports And Memory Mode Isolation

### Weeks 4-5

Deliverables:

- split monolithic PostgreSQL repository responsibilities into domain-focused store interfaces
- move retrieval-facing behavior behind dedicated ports consumed by runtime modules
- isolate `memory mode` as dev/test-only adapter implementations
- move rate limiting from process-local state to Redis-backed shared enforcement

Success criteria:

- production path no longer branches repeatedly between PG and in-memory implementations
- `memory mode` remains usable locally but is clearly non-production
- multi-instance rate limiting semantics are valid

## Phase 4: Verification And Release Hardening

### Week 6

Deliverables:

- full chat E2E verification over the official SSE contract
- CI enforcement for contract and unsupported-route regressions
- lockstep frontend/backend release validation

Success criteria:

- end-to-end chat works on the new single contract
- archived frontend is no longer in active dependency graphs
- no hidden fallback path silently bypasses the official contract

## Acceptance Criteria

The redesign is complete only when all of the following are true.

### Contract Integrity

- backend and frontend compile against the same contract crate
- no mirrored transport DTOs remain in frontend `web-sdk`
- official chat contract is documented from the shared source

### Chat Runtime Behavior

- `POST /api/v1/chat` is the only official chat entrypoint
- streaming chat emits only supported official events
- each stream satisfies `start -> ... -> done|error`

### Boundary Clarity

- `AppState` is reduced to runtime composition and shared dependencies
- `transport-http` is modularized by responsibility
- business logic is owned by focused application services

### Runtime Simplicity

- `memory mode` exists only as dev/test adapter behavior
- production path does not depend on in-memory fallbacks
- Redis owns shared rate limiting state

### Workspace Hygiene

- archived frontend crates are not workspace members
- production frontend ownership is unambiguous

### Product Surface Integrity

- no visible frontend control leads to a backend `501`

## CI Merge Gates

The following checks become required for merge.

1. Workspace contract check
   Build must prove backend and frontend are compiling against the shared contract crate.

2. Chat contract tests
   Provider and consumer tests must validate request shape, event names, and terminal stream behavior.

3. Chat E2E stream test
   At least one E2E test must complete a valid stream with:
   `start -> token(s) -> citations? -> done`

4. Unsupported route check
   Automated frontend/API smoke tests must fail if any visible user flow triggers `501 Not Implemented`.

5. DTO drift prevention
   CI must fail if new hand-maintained mirrored transport DTOs are introduced outside the shared contract crate.

## Risks And Controls

### Risk: Protocol cleanup breaks the current UI

Control:

- converge the contract first
- gate unsupported UI before deeper refactors
- add consumer-driven tests before changing transport behavior

### Risk: Refactor expands into feature delivery

Control:

- enforce scope freeze
- treat any new endpoint implementation outside governance work as out of scope

### Risk: Memory mode hides real runtime bugs

Control:

- keep it as dev/test adapter only
- do not use it as evidence of production correctness

### Risk: Archived crate removal breaks hidden dependencies

Control:

- remove from workspace membership before deleting directories
- verify build graph and E2E before final deletion

## Final Recommendation

Proceed with the redesign exactly as a boundary-governance initiative, not as a feature sprint and not as a core-runtime rewrite.

The correct order is:

1. converge the contract
2. shrink the application and transport boundaries
3. isolate runtime adapters
4. harden CI

This preserves the healthy core while removing the shell-level architectural risks currently driving maintenance cost and delivery uncertainty.
