# Thermo-Nuclear Code Quality Audit — `contracts` crate

**Scope:** `/home/chuan/context-osv6/contracts` (src/, bindings/, tests/, Cargo.toml, typeshare.toml, codegen scripts) and the downstream TS output in `frontend_next/lib/contracts/generated/`.
**Baseline:** `cargo test` → 8 + 3 + 16 passing, 1 ignored; **3 ts-rs codegen warnings** present.
**Verdict:** ❌ **Not approved.** One CRITICAL silent-codegen-drift defect, several HIGH structural issues, and a chronically leaky codegen boundary. The Rust side is disciplined; the codegen pipeline is the liability.

---

## Findings

### [CRITICAL] `ChatResponse.agent_operation_guide` silently disappears from the TS contract — codegen boundary leak, undetected

- **File**: `contracts/src/chat.rs:670-687` (Rust source), `frontend_next/lib/contracts/generated/contracts.ts:338-353` (generated TS)
- **Problem**: The Rust `ChatResponse` carries an `agent_operation_guide: Option<AgentOperationGuide>` field (`chat.rs:670-671`). The generated `ChatResponse` TS interface (`contracts.ts:338-353`) **omits this field entirely**. `AgentOperationGuide` and `ToolSpec` are absent from the typeshare output (`grep AgentOperationGuide contracts.ts` → 0 matches).
  - Root cause: `AgentOperationGuide` is annotated `#[typeshare]` (`chat.rs:680`), but it embeds `Vec<crate::tool_call::ToolSpec>` and `ToolSpec` (`tool_call.rs:11`) has **no `#[typeshare]` attribute**. typeshare silently drops the type rather than failing.
  - ts-rs exports `AgentOperationGuide` separately (`agent_operation_guide.ts`) but hand-forces `tool_schemas: Array<Record<string, unknown>>` (`chat.rs:685`), discarding the `ToolSpec` shape.
  - The golden-fixture test (`golden-fixtures.test.ts:122-142`) only spot-checks `answer`, `answer_blocks`, `guard_report`, `planner_output`, `degrade_trace` — it never asserts the *full* set of keys, so the drift is invisible. The fixture JSON itself contains no `agent_operation_guide` key.
  - `check_contract_governance.sh` only guards against duplicate Rust DTOs in transport crates; it does **not** verify TS-codegen completeness.
- **Code-judo move**: This is the strongest argument for consolidating to a single codegen tool. Either:
  1. Add `#[typeshare]` to `ToolSpec` so typeshare can emit the full `ChatResponse` (and delete the separate ts-rs `agent_operation_guide.ts`), **or**
  2. Standardize on ts-rs for *all* chat types (it handles tags + rename natively) and delete typeshare for chat.rs.
  Then add a TS-side golden test that does `expect(Object.keys(loadFixture<ChatResponse>(...)).sort()).toEqual([...])` against the Rust-derived key set so any future drift fails CI instead of shipping silently.

### [HIGH] Dual codegen stack (typeshare + ts-rs + 2 mutating Python patch scripts) — the pipeline is fighting itself

- **File**: `scripts/generate-contracts.sh`, `scripts/patch-chat-contract-codegen.py`, `scripts/annotate-contract-typeshare-integers.py`, `contracts/typeshare.toml`, `contracts/Cargo.toml:12-13`
- **Problem**: Four overlapping mechanisms generate TS from Rust:
  1. `typeshare` CLI → `contracts.ts`
  2. `ts-rs` `export-types` bin → `answer_block.ts`, `chat_event.ts`, `agent_operation_guide.ts`
  3. `patch-chat-contract-codegen.py` — **mutates committed Rust source** (`contracts/src/chat.rs`) via regex to inject `#[typeshare]` / `#[ts(...)]` annotations
  4. `annotate-contract-typeshare-integers.py` — **also mutates committed Rust source** across all `contracts/src/*.rs` to inject `#[typeshare(serialized_as = "number")]`
  - `typeshare.toml` *already* maps `i64`/`u64`/`usize` → `"number"`, yet the Python integer annotator exists to add per-field `serialized_as` because typeshare 1.13 rejects unmapped integer fields. Two tools doing the same integer→number job is redundant self-fighting.
  - The Python scripts mutate committed source as a side effect; if their regex drifts from the Rust structure, the generated TS silently diverges (as the CRITICAL finding demonstrates).
  - `generate-contracts.sh` then post-processes the *output* with `sed` (stripping imports, rewriting `bigint`→`number`, inlining `ChatActivitySourcePreview`) — more fragile string surgery on generated code.
- **Code-judo move**: Pick one tool. ts-rs handles `#[serde(tag)]`, `#[serde(rename_all)]`, and `i64 → number` natively and emits per-type files (which the patch script is already simulating for `AnswerBlock`/`ChatEvent`). Migrating fully to ts-rs lets you delete: `typeshare.toml`, both Python patch scripts, the `typeshare` dependency, the `sed` post-processing, and the manual `import type { AnswerBlock }` injection in `generate-contracts.sh`. That removes an entire category of failure (source mutation by codegen) and collapses 4 tools → 1.

### [HIGH] `chat.rs` (817 lines) mixes three unrelated domains and is approaching the 1k threshold

- **File**: `contracts/src/chat.rs:1-817`
- **Problem**: `chat.rs` is a grab-bag containing:
  - Agent-type dispatch (`AgentTypeKind`, lines 6-53) — runtime logic, not a wire DTO
  - Chat request/response DTOs (`ChatRequest`, `ChatResponse`, `ChatMessage`, lines 55-126, 644-697)
  - Citation/source DTOs (`Citation`, `SourceRef`, lines 134-198)
  - Guard/safety contracts (`GuardResult`, `GuardReport`, `RiskLevel`, `GuardAction`, lines 332-459)
  - RAG planner/debug trace contracts (`RagPlan`, `RagPlanItem`, `PlannerOutput`, `RagTraceItem`, `RagModeDebug`, lines 461-584)
  - Tool-result contracts (`ToolResult`, `ToolTrace`, `ToolStatus`, lines 586-624)
  - SSE event enum (`ChatEvent`, lines 711-788)
  - Token usage (`ChatTokenUsage`, lines 626-642)
  - Feedback (`MessageFeedbackRating`, `MessageFeedbackRequest`, lines 802-817)
  - `DegradeReason` with 90+ lines of manual `as_str`/`from_str`/`as_stage`/`message` + custom Serde impls (lines 207-321)
  At 817 lines it's the largest file and the most likely to cross 1k on the next feature. It also forces `rag_execute.rs` to import 6 separate types from it (`chat.rs:3-5`), creating a tight cross-module coupling.
- **Code-judo move**: Decompose by domain into focused submodules, all re-exported from `chat/mod.rs` for API stability:
  - `chat/agent_type.rs` — `AgentTypeKind` (runtime dispatch helper)
  - `chat/guard.rs` — `GuardResult`, `GuardReport`, `RiskLevel`, `GuardAction`, `DegradeReason`, `DegradeTraceItem`
  - `chat/plan.rs` — `RagPlan`, `RagPlanItem`, `PlannerOutput`, `SearchPlan`, `GeneralPlan`, trace structs
  - `chat/event.rs` — `ChatEvent`, `ChatActivitySourcePreview`, `AgentOperationGuide`
  - `chat.rs` (remainder) — `ChatRequest`, `ChatResponse`, `ChatMessage`, `ChatTurnInput`, `ToolResult`, `ToolTrace`, `ChatTokenUsage`, feedback
  This keeps the public API (`pub use`) identical while making each file scannable and letting `rag_execute.rs` depend on `chat::plan` instead of the whole bag.

### [HIGH] `DegradeReason` is a hand-rolled string-enum with 90+ lines of boilerplate — and it's lossy on the TS side

- **File**: `contracts/src/chat.rs:207-321`
- **Problem**: `DegradeReason` manually implements `as_str` (21 arms), `from_str` (20 arms), `as_stage` (7 arms with a catch-all `_ => "degraded"`), `message` (6+ arms), custom `Serialize`, custom `Deserialize`, and `#[typeshare(serialized_as = "String")]`. That's ~115 lines for what is conceptually a string-tagged enum.
  - `as_stage()` (`chat.rs:281-290`) is **dead code** — defined but never called anywhere in the workspace (`grep as_stage` → only the definition).
  - The TS side gets `export type DegradeReason = string` (`contracts.ts:10`) — the entire rich variant set is erased. The backend uses typed variants (`DegradeReason::PlannerFailed`, `DegradeReason::EmbeddingUnavailable`, etc.) across 20+ call sites, but the frontend cannot distinguish them.
  - `from_str` accepts aliases (`"no_results" | "no_results_after_all_fallbacks"` → same variant) — implicit backward-compat logic baked into a contract type.
- **Code-judo move**: Replace the manual impl with `strum` (`strum::EnumString`, `strum::Display`, `#[strum(serialize = "...")]` for aliases) — cuts ~90 lines. For the TS side, emit a real discriminated-string union (`"budget_exhausted" | "no_results_after_all_fallbacks" | ...`) so the frontend gets type safety. Delete `as_stage()` (dead). If `strum` is undesirable, at minimum delete `as_stage`, `message`, and the custom Serde impls in favor of `#[serde(rename_all = "snake_case")]` + a typed `Other(String)` variant handled by serde's default enum string serialization.

### [HIGH] `rag_execute.rs` carries legacy compat shims that should be deleted, not maintained

- **File**: `contracts/src/rag_execute.rs:282-448`
- **Problem**: Three "compat" methods live on a *contract* type:
  - `ensure_original_query_text_dense_item` (lines 282-306) — mutates `self.items` to prepend the original query. This is **runtime behavior**, not contract shape.
  - `to_chat_request_compat` (lines 392-420) — builds a `ChatRequest` from an `ExecutePlanRequest`. The comment at line 379-382 admits this was a hack (`"replaces the previous to_chat_request_compat() + request_doc_ids() hack"`).
  - `to_rag_plan_compat` (lines 422-448) — round-trips `ExecutePlanRequest` back into `RagPlan` just to feed `build_item_trace_with_total` (`avrag-rs/crates/rag-core/src/runtime/execute.rs:287-290`).
  - `doc_ids` (lines 383-390) exists *because* of the compat hack and is only partially a replacement.
  The contract crate is now hosting runtime orchestration logic (`ensure_original_query_text_dense_item` literally reorders and truncates items to `MAX_ITEMS`). This is a layering violation: contracts should describe wire shapes, not implement retrieval-prep policy.
- **Code-judo move**: Move `ensure_original_query_text_dense_item`, `to_chat_request_compat`, `to_rag_plan_compat` into `rag-core` (where they're called) as free functions taking `&ExecutePlanRequest`. The contract type stays a pure data shape. The call site in `execute.rs:287-290` already imports `compat_request`/`compat_plan` as locals — making them explicit function calls there is cleaner than method-call sugar on a DTO.

### [MEDIUM] `tool_call.rs` embeds a 277-line test module — contract crate hosting unit tests for adapter logic

- **File**: `contracts/src/tool_call.rs:346-623`
- **Problem**: The `from_tool_calls` adapter (lines 251-344) is runtime translation logic living in the contracts crate, and its 13 tests (lines 346-623, 277 lines) bloat the file to 623 lines. The adapter is a *compat shim* (per its own doc comment, line 246-250: "compatibility shim for Phase 1"). Contract crates should not own adapter logic; this belongs next to the runtime that consumes `ExecutePlanRequest`.
- **Code-judo move**: Move `from_tool_calls` + `ToolCallAdapterError` + its tests into `rag-core/src/runtime/` (where `ExecutePlanRequest` is actually consumed). `tool_call.rs` shrinks to ~90 lines of pure DTOs. This also removes the awkward `impl ExecutePlanRequest { ... }` block that lives in `tool_call.rs` but extends a type defined in `rag_execute.rs`.

### [MEDIUM] Triplicated chunk-shape structs — `Citation`, `RetrievedChunk`, `AnswerContextChunk`, `CitationLookupResponse` share 10-11 fields

- **File**: `contracts/src/chat.rs:134-167` (`Citation`), `contracts/src/rag_execute.rs:463-504` (`RetrievedChunk`), `contracts/src/documents.rs:140-188` (`CitationLookupResponse`, `AnswerContextChunk`)
- **Problem**: Four structs carry near-identical chunk metadata:
  - `Citation` (16 fields) and `CitationLookupResponse` (10 fields): **11 fields overlap** (`doc_name, content, doc_id, chunk_id, page, chunk_type, asset_id, caption, image_url, parser_backend, source_locator`).
  - `RetrievedChunk` (14 fields) and `AnswerContextChunk` (10 fields): **10 fields overlap**; there's already a manual `as_answer_context_chunk()` conversion (`rag_execute.rs:489-503`) copying field-by-field.
  - The repeated `asset_id`/`caption`/`image_url`/`parser_backend`/`source_locator`/`parse_run_id` block appears in 3+ structs with identical `#[serde(default)]` + `Option<String>` / `Option<serde_json::Value>` shape.
- **Code-judo move**: Extract a `ChunkMeta` struct holding the shared 10 fields (`asset_id`, `caption`, `image_url`, `parser_backend`, `source_locator`, `parse_run_id`, `chunk_type`, `page`, `doc_id`, `chunk_id`). Embed it (`#[serde(flatten)]`) into `Citation`, `RetrievedChunk`, `CitationLookupResponse`. `AnswerContextChunk` becomes `ChunkMeta + text`. The `as_answer_context_chunk()` manual copier becomes a trivial `ChunkMeta` clone. This deletes the duplication at its root rather than papering over it with conversion methods.

### [MEDIUM] `as_str`/`Display` boilerplate duplicated across 6+ enums — missing `strum`

- **File**: `contracts/src/chat.rs:25-53` (`AgentTypeKind`), `chat.rs:232-308` (`DegradeReason`), `chat.rs:340-349` (`RiskLevel`), `chat.rs:361-371` (`GuardAction`), `documents.rs:21-34` (`DocumentStatus`), `rag_execute.rs:22-37` (`ExecutePlanSummaryMode`)
- **Problem**: Six enums each hand-roll `as_str(&self) -> &'static str` and `Display` with identical match-arm-per-variant boilerplate. `DocumentStatus` even has both `#[serde(rename_all = "lowercase")]` *and* a manual `as_str` that duplicates the same lowercase strings — two sources of truth for the same mapping. `AgentTypeKind::from_str` accepts both `"write_refine"` and `"write-refine"` (alias), which only works because it's hand-written.
- **Code-judo move**: Add `strum` as a dependency and derive `Display` + `EnumString` + `AsRefStr` in one attribute block per enum. Use `#[strum(serialize = "write-refine", alias = "write_refine")]` for the alias case. Delete every manual `as_str`/`from_str`/`Display` impl. The `DocumentStatus` dual-source disappears because `#[serde(rename_all]` and `strum` derive from the same variant names. Net deletion: ~120 lines.

### [MEDIUM] ts-rs codegen emits 3 warnings — `skip_serializing_if = "Vec::is_empty"` not parsed

- **File**: `contracts/src/chat.rs:684` (`AgentOperationGuide.tool_schemas`), and 2 others
- **Problem**: `cargo test` prints 3× `warning: failed to parse serde attribute | #[serde(default, skip_serializing_if = "Vec::is_empty")] | = note: ts-rs failed to parse this attribute. It will be ignored.` ts-rs silently ignores the skip-if-empty hint, meaning the generated `agent_operation_guide.ts` will always include `tool_schemas` (even when empty) — a behavioral divergence between Rust serialization (omits empty) and the TS contract (always present). Warnings in a codegen step are silent drift seeds.
- **Code-judo move**: Replace `skip_serializing_if = "Vec::is_empty"` with the ts-rs-supported `#[ts(type = "...")]` + `#[serde(default)]` without `skip_serializing_if` if the field should always be present in TS, or use `Option<Vec<...>>` if it's genuinely optional. Eliminate the warnings so codegen is clean-or-fails, not warn-and-hope.

### [MEDIUM] `AgentTypeKind` is runtime dispatch logic living in a wire-contract crate

- **File**: `contracts/src/chat.rs:6-53`
- **Problem**: `AgentTypeKind` is not a wire DTO — it's a runtime classifier with `from_str`/`as_str`/`Display` and a `ChatRequest::agent_kind()` helper (line 96-98). The wire field is `agent_type: String` (line 64), so the enum is purely internal dispatch. Hosting it in `contracts` (whose job is cross-language wire shapes) blurs the boundary: the frontend can't use it (it's not typeshare-exported as an enum), and the backend has to reach into `contracts` for runtime logic.
- **Code-judo move**: Move `AgentTypeKind` into the backend crate that dispatches on it (likely `app-chat`). Keep `ChatRequest.agent_type: String` as the wire contract. The `agent_kind()` helper moves with it. `contracts/src/chat.rs` shrinks and stops mixing "wire shape" with "runtime classification".

### [LOW] `GuardResult` constructors (`pass`/`block`/`redact`/`flag`) are runtime helpers in a contract crate

- **File**: `contracts/src/chat.rs:389-447`
- **Problem**: Four constructor methods on `GuardResult` (`pass`, `block`, `redact`, `flag`) are convenience builders used by the guardrails layer, not by the wire contract. They encode default assumptions (`RiskLevel::Low` for pass, `GuardAction::Block` for block) that are policy decisions, not contract invariants.
- **Code-judo move**: Move the constructors to `avrag-rs/crates/guardrails` where the policy lives. `GuardResult` in `contracts` stays a pure serializable struct.

### [LOW] `bindings/ChatActivitySourcePreview.ts` is a stale orphan — superseded by inline annotation

- **File**: `contracts/bindings/ChatActivitySourcePreview.ts`
- **Problem**: This 3-line file is the only thing in `contracts/bindings/`. It's not referenced by the generated `chat_event.ts` (the `generate-contracts.sh` script explicitly `sed`-strips the import and inlines the type: `sed -i '/^import type { ChatActivitySourcePreview/d'` + `sed -i 's/Array<ChatActivitySourcePreview>/Array<{ id: string; ... }>/g'`). It's a leftover from before the inline annotation was added.
- **Code-judo move**: Delete `contracts/bindings/ChatActivitySourcePreview.ts` and the `bindings/` directory. The type is defined inline in `chat_event.ts` now.

### [LOW] `default_*` helper functions scattered across modules — minor repetition

- **File**: `chat.rs:790-800` (`default_chat_agent`, `default_rag_plan_version`, `default_rag_plan_confidence`), `notebooks.rs:272-274` (`default_rag_agent`), `share.rs:38-40` (`default_scope`), `rag_execute.rs:8-10` (`default_execute_plan_version`)
- **Problem**: Five modules each define private `fn default_*() -> String/bool/f32` for `#[serde(default = "...")]`. This is fine in isolation but the pattern repeats verbatim and could be a single `serde_defaults` module if it grows. Low severity — not worth refactoring now, but flag for pattern awareness.
- **Code-judo move**: Leave as-is unless the count doubles. If consolidating, a `defaults.rs` with `pub fn chat_agent() -> String { ... }` etc. would centralize it.

---

## Summary Table

| # | Severity | File | Issue |
|---|----------|------|-------|
| 1 | CRITICAL | chat.rs:670 / contracts.ts:338 | `agent_operation_guide` silently dropped from TS — undetected codegen drift |
| 2 | HIGH | generate-contracts.sh + 2 Python scripts | 4-tool codegen stack mutates committed Rust source; self-fighting |
| 3 | HIGH | chat.rs (817 lines) | Mixes 7+ domains; approaching 1k threshold |
| 4 | HIGH | chat.rs:207-321 | `DegradeReason` 115-line manual enum + dead `as_stage()` + lossy TS `string` |
| 5 | HIGH | rag_execute.rs:282-448 | Runtime orchestration methods on a contract DTO (`ensure_*`, `to_*_compat`) |
| 6 | MEDIUM | tool_call.rs:346-623 | 277-line test module for a compat adapter in the contract crate |
| 7 | MEDIUM | chat.rs / rag_execute.rs / documents.rs | 4 chunk-shape structs with 10-11 overlapping fields |
| 8 | MEDIUM | 6 enums across src/ | Hand-rolled `as_str`/`Display` boilerplate — missing `strum` |
| 9 | MEDIUM | chat.rs:684 | 3 ts-rs codegen warnings — silent behavioral divergence |
| 10 | MEDIUM | chat.rs:6-53 | `AgentTypeKind` is runtime dispatch logic in a wire-contract crate |
| 11 | LOW | chat.rs:389-447 | `GuardResult` constructors are policy helpers in a contract crate |
| 12 | LOW | bindings/ChatActivitySourcePreview.ts | Stale orphan file, superseded by inline annotation |
| 13 | LOW | 5 modules | Scattered `default_*` serde helpers — minor repetition |

---

## Architectural Verdict

The **Rust contracts are well-disciplined** as DTOs (with the exceptions of #5, #6, #10, #11 where runtime logic leaked in). The **codegen pipeline is the systemic liability**: it's a 4-tool, source-mutating, sed-post-processed pipeline that has already silently lost a field (#1) and emits warnings on every run (#9). The single highest-leverage move is **collapsing to one codegen tool** (ts-rs) and adding a **full-key-set golden test** so drift fails CI instead of shipping. That one move deletes an entire failure category and makes findings #2, #9, and the codegen half of #1 disappear.

The second highest-leverage move is **decomposing `chat.rs`** (#3) — it's the file most likely to cross 1k and is the coupling hub that forces `rag_execute.rs` to import 6 types from it.

**Approval status: ❌ BLOCKED.** Findings #1 and #2 are presumptive blockers (silent codegen drift + source-mutating multi-tool pipeline). #3, #4, #5 are structural regressions that should be addressed before the crate grows further.