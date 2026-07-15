# Design: Capability Multiselect, Write Offline, and `user_context`

**Date:** 2026-07-15  
**Status:** Approved (product design)  
**Scope:** Product chat mode selection, agent tool exposure, write lane product removal, base locale tool  

---

## 1. Goals and non-goals

### Goals

1. **Remove Writing Agent from the product surface** (hard offline, not a temporary hide).
2. Replace mutually exclusive mode selection with **multi-select capability tags** (RAG and Search only).
3. **Tool exposure = union of selected capabilities**; empty selection = pure chat with no RAG/websearch tools exposed.
4. Restructure prompts into **one agent base + per-capability manuals**, composed by selection.
5. Add base tool **`user_context`**: MaxMind GeoLite2 IP→city plus frontend clock; one-shot structured return; model must not invent city.

### Non-goals (this wave)

- Deep deletion of write-core / write pipeline internals (cut reachability only).
- Additional capability tags beyond RAG and Search.
- Precise geolocation, continuous tracking, or client-side IP inference.
- Making `user_context` a user-toggleable capability tag (it is always base-resident).

---

## 2. Locked product decisions

| Item | Decision |
|------|----------|
| Capability tags | **RAG** (workspace knowledge retrieval), **Search** (web) only; multi-select |
| Default | Both off → pure chat |
| Dual select | Tool/skill **union**; budget take **max** (looser); **merged synthesis contract** |
| Prompts | `agent-base` + optional RAG manual + optional Search manual |
| API | New field `capabilities: string[]` |
| Legacy `agent_type` | **Limited compatibility** (see §3.2); `write` rejected |
| Write | Product hard offline: UI gone + API reject; backend pipeline may remain unreachable |
| Selection memory | **Session-only**; new session resets; no global user preference |
| Message display | **0–2 capability tags**; store `capabilities[]` as primary |
| Config merge approach | **Single agent lane + runtime composition** (not virtual enum explosion) |
| GeoIP | **Local MaxMind GeoLite2** (mmdb) |
| Clock | **Frontend-reported** local time + timezone |
| City inference | Server GeoIP in tool result; **no LLM city guessing** |
| Tool name | **`user_context`** |
| `user_context` visibility | **Base-resident** for all capability sets including pure chat |

---

## 3. API contract

### 3.1 Primary request shape

```json
{
  "message": "...",
  "workspace_id": "...",
  "capabilities": ["rag", "search"],
  "client_context": {
    "local_time": "2026-07-15T14:32:00+08:00",
    "timezone": "Asia/Shanghai"
  }
}
```

| Field | Rules |
|-------|--------|
| `capabilities` | Allowed values: `rag`, `search` only. Missing / `null` / `[]` → pure chat. Deduplicate. Unknown values: **ignore** (normalize). |
| `client_context` | Frontend should send on every chat turn. If missing, `user_context` reports clock fields as unavailable (no invention). |
| Precedence | If `capabilities` is present (including empty array), it **wins** over `agent_type` for mode/capability resolution. |

### 3.2 Legacy `agent_type` compatibility

| Input | Resolved capabilities |
|-------|------------------------|
| Only `agent_type=chat` or `general` | `[]` |
| Only `agent_type=rag` | `["rag"]` |
| Only `agent_type=search` | `["search"]` |
| Only `agent_type=write` | **HTTP 4xx reject** with explicit error (do not map) |
| Both `capabilities` and `agent_type` | Use **`capabilities`** |
| `agent_type=write` with any `capabilities` | Still **reject write path**; product write lane is offline |

Document `agent_type` as deprecated for mode selection; keep mapping for old clients/scripts for one compatibility window.

### 3.3 Persistence and replay

- Prefer storing **`capabilities: string[]`** on the turn / message metadata used for UI replay.
- Historical rows that only have single `mode` / `agent_type`: read-only map to a one-element list or legacy label for display; do not rewrite storage unless a migration is explicitly scheduled later.

---

## 4. Product UI

1. Composer control becomes **two toggles/tags**: knowledge retrieval (RAG), web search (Search).
2. Remove write mode entry, hints, and write-only usage copy from the mode selector.
3. No exclusive “chat mode” option: **empty selection is chat**.
4. Session UI state remembers toggles until session ends / new session.
5. Message chrome shows **0–2 capability chips** from stored `capabilities`.
6. Frontend always attaches `client_context` (`timezone` + ISO-8601 `local_time` with offset when possible).

---

## 5. Backend architecture

### 5.1 Approach (approved)

**Single agent lane + runtime ModeConfig / prompt composition** (aligned with ADR-0007 Chat/RAG/Search lane; write stays out of ToolCatalog and is product-rejected).

```text
resolve(capabilities, legacy agent_type)
  → CapabilitySet { rag: bool, search: bool }
  → assemble tool_pool / skill_catalog / budgets / loop_exit / synthesis
  → assemble system prompt (base + manuals)
  → UnifiedAgent / ReAct / ToolCatalog
```

### 5.2 Tool and skill exposure

| CapabilitySet | RAG / web tools | Base tools |
|---------------|-----------------|------------|
| `{}` | None of RAG retrieve / web_search / web_fetch product surfaces | `user_context` (+ existing chat base such as memory if already in chat) |
| `{rag}` | RAG set (current rag mode behavior) | + `user_context` |
| `{search}` | Search set (`web_search`, `web_fetch`, …) | + `user_context` |
| `{rag, search}` | **Union** | + `user_context` |

Pure chat must **not** expose RAG or websearch tool interfaces to the model.

### 5.3 Budget and synthesis merge

| Case | Behavior |
|------|----------|
| Single capability | Keep that capability’s existing budget / evidence / synthesis contract intent |
| Both | tool/skill **union**; numeric budgets (e.g. `max_iterations`) take **max**; use a **new merged synthesis contract** id (not hard-bind only rag or only search) |
| Pure chat | No evidence requirement; allow content early stop / skip synthesis-on-direct-answer (chat-like) |

Exact numeric tables can mirror current `modes/rag.yaml` and `modes/search.yaml` at implementation time.

### 5.4 Prompt assets

Recompose (do not keep three mutually exclusive full orchestrator dumps as the only model):

```text
prompts/orchestrators/
  agent-base.md            # identity + task framing (shared)
  capability-rag.md        # RAG manual (differentiated)
  capability-search.md     # Search manual (differentiated)
```

Composition:

```text
system = agent-base
       + (rag ? capability-rag : "")
       + (search ? capability-search : "")
       + existing progressive / skill injection for the resolved pool
```

Source material: split/refactor from current `chat-system.md`, `rag-system.md`, `search-system.md`.

### 5.5 Write product offline

| Layer | Action |
|-------|--------|
| Frontend | Remove write from mode UI, i18n mode labels used as selectable modes, related tests |
| API / conversation execute | Reject `write` (and reserved write-only product paths) with clear 4xx |
| Backend write pipeline | **Leave code in tree** this wave; mark deprecated / unreachable from product entry |
| Tests / contracts | Stop treating write as a user-selectable mode; add reject-write coverage |

Iron rules remain: write refine tools stay outside ReAct ToolCatalog; no re-registration on catalog.

---

## 6. Tool: `user_context`

### 6.1 Purpose

Give the model **user-local clock** and **city-level location inferred from request IP** in one structured tool result. Typical flow: weather → `user_context` → `weather_query(city, today)`.

### 6.2 Visibility

Always in the **base** tool pool for pure chat, RAG-only, Search-only, and dual.

### 6.3 Inputs and dependencies

| Data | Source |
|------|--------|
| Local time / timezone | Request `client_context` from frontend |
| Client IP | Server extract (`X-Forwarded-For` / `X-Real-IP` / peer), same family as existing rate-limit IP helpers |
| City / region / country | **MaxMind GeoLite2 City** local mmdb lookup on that IP |

Tool args: prefer no required args. Optional flags (e.g. include raw IP) default off for privacy.

### 6.4 Output shape (normative intent)

```json
{
  "local_time": "2026-07-15T14:32:00+08:00",
  "timezone": "Asia/Shanghai",
  "geo": {
    "country": "CN",
    "region": "Guangdong",
    "city": "Shenzhen",
    "confidence": "city",
    "source": "maxmind_geolite2"
  }
}
```

Failure / partial (must be distinguishable; never fabricate city):

- Missing `client_context` → clock fields absent or explicit unavailable reason.
- Private / unknown IP, missing mmdb, or lookup miss → `confidence: "none"` + `reason`; omit fake city.

### 6.5 Deployment

- Config env e.g. `GEOIP_CITY_DB_PATH` pointing at GeoLite2-City.mmdb.
- Dev without mmdb: geo degrades to `none`; chat must still work.
- Document how to obtain/update the database (MaxMind license / download process).
- Production reverse proxy must forward real client IP and only trust boundary hop.

### 6.6 Prompt / tool description constraints

- For “today / nearby / local weather” prefer calling `user_context` first.
- **Do not invent city** when `geo.confidence` is not city-level; ask the user or explain inability to locate.

### 6.7 Relation to existing weather skill

Keep `weather_query` as-is (city name or coordinates). `user_context` supplies city + local date; does not replace OpenWeather.

---

## 7. Frontend implementation notes

1. Replace single-select mode menu with capability toggles.
2. Request payload: `capabilities` + `client_context`.
3. Session store: capabilities array; default `[]`.
4. Message list: render from `capabilities[]`.
5. Normalize legacy transcript `mode` / `agent_type` for display only.
6. E2E POM: replace `setMode(...)` with capability helpers.
7. Remove write mode tests and fixtures as selectable product mode.

---

## 8. Verification (solo local trunk)

| Area | Checks |
|------|--------|
| `agent-tools` | `user_context` unit tests: clock passthrough; geo hit; mmdb missing / private IP → none |
| `app-chat` / `agent-loop` / bootstrap | Capability resolve; union pools; pure chat has no RAG/web tools; legacy map; write reject |
| Prompt load | Base-only / +rag / +search / +both composition smoke |
| `frontend_next` | Toggle UI; request body; chips; session default empty |
| Out of scope mid-wave | Full Playwright suite, real LLM gates, VPS deploy |

Targeted: `cargo test -p agent-tools --lib`, `cargo test -p app-chat --lib`, `cargo test -p agent-loop --lib` (as touched), frontend unit tests for composer/modes.

---

## 9. Risks and mitigations

| Risk | Mitigation |
|------|------------|
| Dual-capability answer quality | Explicit manuals + merged contract; keep tool union simple first |
| Missing GeoLite2 in env | Soft degrade; do not block chat |
| VPN / office egress wrong city | Document city-level approximation; user can override in dialogue |
| Old clients on `agent_type` | Compatibility table; reject write loudly |
| History only has single mode | Display mapping only |
| Scope creep into write deletion | Stay product-offline only this wave |

---

## 10. Suggested implementation waves

1. **Contract + resolve:** `capabilities`, legacy map, reject write, persistence field for capabilities.
2. **Prompt + ModeConfig assembly:** three prompt files + runtime merge rules + tests.
3. **Frontend:** toggles, `client_context`, chips, remove write UI.
4. **`user_context` + MaxMind:** skill/tool, IP plumbing into execution context, mmdb config.
5. **Docs / tests / i18n cleanup:** e2e POM, contract fixtures, agent guides.

Waves 3–4 can partially parallelize after wave 1 lands the wire fields.

---

## 11. Open implementation details (non-blocking)

Resolved at implementation plan / coding time without changing product intent:

- Exact JSON field names on stored messages if multiple wire shapes exist today (`mode` vs metadata).
- Merged synthesis contract id string and prose rules.
- Whether `agent_type` remains on response as a derived summary string for older UIs (e.g. derived label) vs dropped from new responses only.
- GeoLite2 file packaging (gitignored path vs ops-provided on host).

---

## 12. Approval

| Role | Decision | Date |
|------|----------|------|
| Product / requester | Approved; tool name `user_context` | 2026-07-15 |
| Design capture | This document | 2026-07-15 |

**Next step after user review of this file:** implementation plan (`writing-plans` / engineering plan doc), then local trunk implementation.
