# Context-OSV6 Stable Layered Monolith Design

> Scope freeze: this design defines the target steady-state architecture for `context-osv6` as a strongly constrained layered monolith optimized for maintainability, AI-assisted development, and future migration safety. It is not a distributed-systems design and does not prescribe a microservices split.
> 2026-04-26 update: Qdrant references in this document are historical examples of an external retrieval adapter. The current target retrieval adapter is Milvus; see [Current Product Architecture](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md).

## Goal

Establish a stable, reliable product architecture that keeps deployment simple while making the codebase small-file, single-responsibility, and boundary-driven, so both humans and coding agents can change it without causing architectural drift.

## Non-Goals

- No microservice or distributed-service decomposition
- No multi-region or global-scale deployment design
- No introduction of platform complexity that is not justified by the current product
- No tolerance for "temporary" boundary violations that would reintroduce God objects or duplicate DTOs

## Architectural Position

`context-osv6` is a modular monolith:

- one API process
- one worker process
- external infrastructure limited to PostgreSQL, Redis, Qdrant, and object storage
- strict internal module boundaries that make future extraction possible without requiring extraction now

The system is optimized for codebase clarity first, deployment simplicity second, and future scale safety third.

## Core Design Principles

1. Contract first
   Shared transport shapes live in one place only and drive the rest of the system.

2. Single responsibility by default
   A file, module, and crate each exist for one primary purpose. Mixed-responsibility files are treated as defects.

3. Runtime wiring is not business logic
   Container/bootstrap code may assemble dependencies, but it may not execute use cases.

4. Business flow belongs to services
   Application services own orchestration, policy checks, and use-case sequencing.

5. Infrastructure is replaceable
   PostgreSQL, Redis, Qdrant, object storage, and memory adapters enter through ports and can be replaced without rewriting service logic.

6. Human and AI readability is a hard requirement
   Directory shape, dependency direction, file size limits, and naming conventions must make the correct place to modify code obvious.

## Canonical Layers

The target system is fixed to six layers.

### 1. `contracts`

Purpose:

- transport DTOs
- SSE event payloads
- error envelopes
- shared enums and protocol-visible value objects

Rules:

- may depend on `serde` and small utility crates only
- may not depend on `app`, `transport-http`, `storage-pg`, `web-ui`, or runtime infrastructure
- is the only source of truth for shared transport models

### 2. `transport-http`

Purpose:

- HTTP route registration
- request parsing
- auth extraction
- middleware
- status code mapping
- SSE framing
- SSR integration

Rules:

- may depend on `app` and `contracts`
- may not depend directly on `storage-pg`, Redis, Qdrant, or object store implementations
- may define HTTP-local form types only when they are truly transport-local and not shared protocol shapes

### 3. `application services`

Purpose:

- use-case orchestration
- transaction boundaries
- permission checks
- workflow sequencing
- calling ports and domain modules

Rules:

- may depend on `contracts`, domain crates, and ports
- may not parse HTTP, emit SSE frames, or write SQL
- owns business flow but not transport formatting or infrastructure details

### 4. `domain / RAG`

Purpose:

- RAG execution
- planner / synthesizer logic
- guardrails
- domain-level policy and algorithmic behavior

Rules:

- must not know about routes, handlers, JWTs, headers, or UI structures
- should work with stable inputs and outputs, ideally protocol-neutral or contracts-backed

### 5. `infra adapters`

Purpose:

- PostgreSQL repositories and query facades
- Redis rate limiting
- Qdrant access
- object store access
- memory adapters for dev/test mode

Rules:

- implement ports defined by the `app` layer
- may be technically complex
- may not own business decisions or orchestration

### 6. `frontend`

Purpose:

- `web-sdk`: client calls, SSE parsing, contracts re-export
- `web-ui`: routes, components, state, platform helpers

Rules:

- `web-sdk` must not become a second contracts crate
- `web-ui` must not know backend internals beyond SDK and contracts

## Canonical Repository Layout

```text
context-osv6/
├── contracts/
│   └── src/
│       ├── lib.rs
│       ├── errors.rs
│       ├── chat.rs
│       ├── auth.rs
│       ├── notebooks.rs
│       ├── documents.rs
│       ├── share.rs
│       ├── billing.rs
│       ├── admin.rs
│       └── usage_limit.rs
├── avrag-rs/
│   ├── bins/
│   │   ├── api/
│   │   └── worker/
│   └── crates/
│       ├── app/
│       │   └── src/
│       │       ├── runtime/
│       │       ├── services/
│       │       ├── ports/
│       │       ├── adapters/
│       │       └── policies/
│       ├── transport-http/
│       │   └── src/
│       │       ├── router.rs
│       │       ├── errors.rs
│       │       ├── routes/
│       │       ├── handlers/
│       │       ├── middleware/
│       │       ├── extractors/
│       │       ├── presenters/
│       │       └── compat/
│       ├── storage-pg/
│       │   └── src/
│       │       ├── chat/
│       │       ├── notebooks/
│       │       ├── documents/
│       │       ├── auth/
│       │       ├── share/
│       │       └── billing/
│       ├── rag-core/
│       ├── llm/
│       ├── guardrails/
│       └── ...
├── frontend_rust/
│   └── crates/
│       ├── web-sdk/
│       │   └── src/
│       │       ├── client/
│       │       ├── sse/
│       │       ├── errors.rs
│       │       └── lib.rs
│       └── web-ui/
│           └── src/
│               ├── routes/
│               ├── components/
│               ├── state/
│               └── platform/
└── scripts/
    ├── check_contract_governance.sh
    ├── check_file_size_limits.sh
    └── check_layer_dependencies.sh
```

## File Size And Responsibility Rules

These are hard constraints, not suggestions.

### File size

- under 300 lines: normal
- 300 to 400 lines: review whether it should split
- over 500 lines: must be split unless explicitly allowlisted

### One primary purpose per file

A file may be one of:

- route registration
- handler
- service
- port
- adapter
- contract module
- presenter
- middleware
- extractor

If a file contains three or more of those categories, it is incorrectly designed.

### `lib.rs` rule

`lib.rs` files are for:

- module declarations
- public exports
- minimal assembly logic

They are not allowed to accumulate domain logic, CRUD implementations, or transport definitions.

## Dependency Rules

Allowed direction:

```text
web-ui -> web-sdk -> contracts
transport-http -> app -> ports -> adapters
transport-http -> contracts
app -> contracts
app -> domain crates
adapters -> ports
adapters -> infrastructure clients
```

Forbidden direction:

- `transport-http -> storage-pg`
- `transport-http -> redis client`
- `transport-http -> qdrant client`
- `web-ui -> backend crates`
- `app -> transport-http`
- `contracts -> app`
- `contracts -> transport-http`
- `contracts -> storage-pg`

## Runtime Model

The runtime must be represented by a thin container, not a God object.

Target shape:

```rust
pub struct Runtime {
    pub config: AppConfig,
    pub services: ServiceRegistry,
}

pub struct ServiceRegistry {
    pub chat: Arc<ChatService>,
    pub notebooks: Arc<WorkspaceService>,
    pub documents: Arc<DocumentService>,
    pub auth: Arc<AuthService>,
    pub share: Arc<ShareService>,
    pub billing: Arc<BillingService>,
}
```

Rules:

- runtime/bootstrap code may assemble dependencies
- runtime/bootstrap code may not implement use cases
- service construction happens once during startup
- request handlers call services, not repositories

## `AppState` Outcome

`AppState` in its current form is transitional only.

The target end state is:

- no business CRUD methods on runtime state
- no direct `if pg { ... } else { ... }` business branching inside service flow
- no transport concerns inside runtime state

The current `AppState` should be reduced until it becomes equivalent to `Runtime` plus service access.

## Domain Skeleton By Business Area

Each business area must follow a standard skeleton so the correct place to add code is obvious.

### Chat

```text
app/src/services/chat/
├── mod.rs
├── service.rs
├── execute.rs
├── preflight.rs
├── session.rs
├── streaming.rs
└── response.rs
```

```text
app/src/ports/chat/
├── mod.rs
├── chat_store.rs
├── chat_session_store.rs
├── citation_store.rs
├── rate_limiter.rs
├── rag_executor.rs
└── event_publisher.rs
```

### Workspaces

```text
app/src/services/notebooks/
├── mod.rs
├── service.rs
├── list.rs
├── create.rs
├── update.rs
└── delete.rs
```

### Documents

```text
app/src/services/documents/
├── mod.rs
├── service.rs
├── list.rs
├── create.rs
├── upload.rs
├── content.rs
├── reindex.rs
└── preview.rs
```

### Auth

```text
app/src/services/auth/
├── mod.rs
├── service.rs
├── login.rs
├── register.rs
├── me.rs
├── password_reset.rs
└── preferences.rs
```

Each domain gets matching ports and matching adapter subdirectories.

## `transport-http` Final Shape

Target internal structure:

```text
transport-http/src/
├── lib.rs
├── router.rs
├── errors.rs
├── middleware/
│   ├── auth.rs
│   ├── request_context.rs
│   ├── rate_limit.rs
│   └── tracing.rs
├── extractors/
│   ├── actor.rs
│   ├── request_id.rs
│   └── pagination.rs
├── routes/
│   ├── chat.rs
│   ├── notebooks.rs
│   ├── documents.rs
│   ├── auth.rs
│   ├── share.rs
│   ├── admin.rs
│   └── infra.rs
├── handlers/
│   ├── chat.rs
│   ├── notebooks.rs
│   ├── documents.rs
│   ├── auth.rs
│   ├── share.rs
│   └── admin.rs
├── presenters/
│   ├── json.rs
│   ├── sse.rs
│   └── errors.rs
└── compat/
    └── openai.rs
```

Rules:

- routes declare endpoints only
- handlers adapt HTTP to service calls only
- presenters own JSON and SSE formatting
- middleware files are isolated by concern
- compat routes are not mixed into canonical route modules

## Contracts And SDK Rules

### `contracts`

- owns every shared request, response, event, and error envelope
- is the only place where transport-visible DTOs are allowed to be defined

### `web-sdk`

Target shape:

```text
web-sdk/src/
├── lib.rs
├── client/
│   ├── chat.rs
│   ├── notebooks.rs
│   ├── documents.rs
│   ├── auth.rs
│   ├── share.rs
│   └── billing.rs
├── sse/
│   └── chat.rs
└── errors.rs
```

Rules:

- `web-sdk` re-exports `contracts`
- `web-sdk` does not define mirrored transport DTOs
- `web-sdk` only adds client concerns such as request sending, auth headers, and stream parsing

## Memory Mode And Adapter Rules

Memory mode remains available, but only as a dev/test runtime assembled through adapters.

Correct form:

- `Runtime::new_memory()` selects memory adapters
- `Runtime::new_production()` selects postgres/redis/qdrant adapters

Incorrect form:

- service code branching directly on concrete storage backend presence

Memory mode must not become an alternate business logic path; it is only an alternate infrastructure assembly.

## Governance And CI

The architecture is enforced through automation.

### Required governance checks

1. file size limit check
2. contracts-only DTO definition check
3. layer dependency check
4. archived workspace member absence check
5. route surface tests
6. stream contract tests
7. service contract tests
8. runtime adapter tests

### CI minimum set

- `bash scripts/check_contract_governance.sh`
- `bash scripts/check_file_size_limits.sh`
- `bash scripts/check_layer_dependencies.sh`
- `cargo test --manifest-path contracts/Cargo.toml`
- `cargo test --manifest-path avrag-rs/Cargo.toml -p transport-http --test chat_stream_contract --test router_surface`
- `cargo test --manifest-path avrag-rs/Cargo.toml -p app --test chat_service_contract --test runtime_adapters --test redis_rate_limiter`

## Completion Criteria

The architecture redesign is complete only when all of the following are true:

1. `contracts` is the sole source of shared transport DTOs
2. `web-sdk` no longer contains mirrored transport model definitions
3. `transport-http` does not directly depend on infra implementations
4. runtime/container code does not expose business CRUD methods
5. business orchestration lives in domain-specific services
6. memory, postgres, redis, qdrant, and object storage enter through ports/adapters
7. large monolithic files are split to within the enforced file size limits
8. CI blocks DTO drift, file growth, and layer violations

## Recommendation

Adopt this target architecture as the non-negotiable steady state for `context-osv6`.

Do not optimize for quick local convenience at the cost of weakening these boundaries. The product's current scale does not justify distributed service complexity, but it does justify strict internal architecture because the present risk is codebase drift, not machine topology.
