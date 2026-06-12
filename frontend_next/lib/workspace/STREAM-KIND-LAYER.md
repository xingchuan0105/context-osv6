# Workspace chat stream: `kind` vs wire `event`

## Context

Generated contract `ChatEvent` (see `lib/contracts`) uses discriminator field **`event`**, matching SSE `event:` lines and JSON payloads from `/api/v1/chat`.

Frontend reducers and UI tests use **`WorkspaceChatStreamEvent`** with discriminator **`kind`**, produced by `chatEventToWorkspace()` in `stream.ts`.

## Why two names?

| Layer | Field | Role |
|-------|-------|------|
| Wire / contracts | `event` | Codegen-aligned transport shape |
| Workspace reducers | `kind` | Narrowed, validated view for React state updates |

`WireToWorkspace<T>` maps `event → kind` and applies stricter runtime parsing where the wire type is loose (e.g. `citations[].source_locator`, `done.payload` cast to `ChatResponse`).

## Evaluation (Brooks K8 / N11)

**Option A — Delete `kind`, consume `ChatEvent` directly in reducers**

- Requires renaming discriminator and updating every `switch (event.kind)` / mock fixture.
- **Out of K8 scope:** `components/share/shared-workspace-surface.tsx`, `lib/runtime/transport.ts`, `lib/runtime/tauri-ipc.ts`, `lib/share/client.ts`, multiple Vitest files under `tests/workspace/` (owned by K7), `tests/contracts/golden-fixtures.test.ts`, `tests/share/`.
- Risk: large cross-directory blast radius; easy to miss a fixture and get silent exhaustiveness gaps.

**Option B — Keep `kind` mapping at transport boundary (chosen)**

- Single parsing seam: `parseWireChatEvent` → `chatEventToWorkspace` → reducers.
- Reducers stay decoupled from SSE field naming and contract renames.
- K8 scope limited to extracting per-event reducer functions in `hooks/chat-session/`.

## Decision

**Keep the `kind` layer** for this cycle. Revisit removal only in a dedicated frontend-wide refactor that can update share surface, runtime transports, and all stream test fixtures in one PR.

## Follow-up (optional, not K8)

- Collapse `chatEventToWorkspace` switch into a generic `{ ...wire, kind: wire.event }` helper once citation/done normalization moves into `parseWireChatEvent` only.
- Or align reducers on `event` and delete `WireToWorkspace` in a single PR touching all consumers listed above.
