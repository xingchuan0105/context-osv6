# context-osv6 E2E Acceptance Report

> Date: 2026-03-22
> Scope: configured local acceptance pass after chat graphflow migration, simplification, and fallback removal
> Historical report. Qdrant references describe the tested environment on 2026-03-22, not the current target architecture. See [2026-04-26 Current Product Architecture](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md).

## 1. Environment Summary

### Runtime used

- API: `/home/chuan/context-osv6/avrag-rs/target/debug/avrag-api`
- Worker: `/home/chuan/context-osv6/avrag-rs/target/debug/avrag-worker`
- Database: PostgreSQL local dev instance
- Queue/cache: Redis local dev instance
- Vector store: Qdrant local dev instance

### Provider configuration result

- `ANSWER_LLM_*`: configured and confirmed working
- `SEARCH_*`: partially configured
  - provider/base URL present
  - actual search API key missing
- embedding/rerank/intent/summary config: present

### Additional runtime adjustment applied

- `QDRANT_COLLECTION=rag_chunks_qwen3_4096`
- `AVRAG_EMBEDDING_DIM=4096`

This was required because the previous Qdrant collection dimension did not match the active embedding model output dimension.

## 2. Build / Test Verification

The following commands completed successfully:

- `cargo build --manifest-path /home/chuan/context-osv6/avrag-rs/Cargo.toml -p app -p avrag-api -p avrag-worker`
- `cargo test --manifest-path /home/chuan/context-osv6/avrag-rs/Cargo.toml -p app --lib`
- `cargo build --manifest-path /home/chuan/context-osv6/frontend_rust/Cargo.toml -p web-ui`
- `cargo build --manifest-path /home/chuan/context-osv6/frontend_rust/Cargo.toml -p web-ui --target wasm32-unknown-unknown --no-default-features --features hydrate`

## 3. API Acceptance Results

### 3.1 Ready endpoint

Result: pass

- `/ready` returned:
  - `status=ready`
  - `scope=postgres`

### 3.2 General chat

Result: pass

- `agent_type=general` returned a real answer
- no longer degraded for missing `ANSWER_LLM`

### 3.3 Search chat

Result: degraded by environment gap

- request path works
- response shape is correct
- current answer is:
  - `Search mode is unavailable: external search provider not configured`

Root cause:

- `SEARCH_API_KEY` / `EXA_API_KEY` is not actually set in the available local configuration

Conclusion:

- code path is healthy
- provider credential is missing

### 3.4 RAG chat

Result: structurally pass, retrieval quality still failing

- request path works
- graphflow orchestration works
- response shape is correct
- current result is still:
  - explicit `insufficient_evidence`

Root cause status:

- reindex task now processes successfully after fixing collection dimension mismatch
- Qdrant collection exists and contains points
- however runtime retrieval still returns zero candidates

Conclusion:

- this is now a retrieval-quality / retrieval-wiring issue, not a transport or orchestration failure

## 4. Browser Acceptance Results

### 4.1 Passed flows

- login
- dashboard
- workspace direct entry
- API Access direct entry
- Share Center direct entry
- public share page direct entry
- admin feature flags direct entry for non-admin user
- public share chat form path
- workspace chat form path

### 4.2 Behavior observed

- Workspace `general` mode is now usable at the API layer
- Workspace `search` mode still shows provider-unavailable degrade copy
- Workspace `rag` mode still shows explicit insufficient-evidence copy
- Public share chat also follows the same RAG insufficient-evidence outcome after fallback removal

## 5. Console / Frontend Warnings

No fatal browser console errors were observed in the final acceptance pass.

However, non-fatal warnings remain:

1. Signal tracking warning from Leptos/Tachys
   - indicates some signal reads are still performed in a non-reactive way

2. DOMTokenList invalid class-token warnings
   - some class additions still pass whitespace-containing strings into DOMTokenList as a single token

These are not current blockers for function, but they should be cleaned up in a future frontend polish pass.

## 6. Final Acceptance Status

### Accepted

- chat graphflow orchestration
- fallback-removal cleanup
- frontend hydration/load-pattern simplification
- API Access / Share / Admin direct-entry stability
- public share chat route and contract stability
- general mode runtime availability

### Not fully accepted

- search mode real external search behavior
- rag mode real retrieval behavior

## 7. Remaining Blocking Gaps

### Gap A: search provider credential missing

Needed:

- valid `SEARCH_API_KEY` or `EXA_API_KEY`

### Gap B: RAG retrieval still yields zero candidates

Current state:

- collection dimension issue fixed
- reindex job completes
- Qdrant collection contains points
- runtime still returns zero hits

Needed next:

- inspect RAG retrieval path end-to-end:
  - embedding output shape
  - Qdrant payload shape
  - filter construction
  - collection name/runtime wiring
  - sparse/dense merge inputs

## 8. Bottom Line

This round achieved:

- stable graphflow-based chat orchestration
- removal of pseudo-fallbacks and placeholder alternates
- successful general-mode configuration
- successful local acceptance of the main UI surfaces

Two environment/product-quality gaps remain:

1. real search provider credentials are absent
2. RAG retrieval quality/path is still not producing candidates despite successful indexing
