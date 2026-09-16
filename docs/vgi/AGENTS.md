# VGI-RAG — agent rules for this tree

VGI is specified as a **design-doc DAG**. Humans edit the docs. Code is `G(spec)`: regenerated from the docs, not patched onto a previous implementation.

Paper: Kushnir et al., *Design Docs Are All You Need*, arXiv:2609.05364. A user-level `spec-regeneration` skill exists under `~/.agents/skills/` and is **not** routed from the product `AGENTS.md`; this file is the repo pointer for VGI.

## When to read which doc

| Branch | Doc |
|---|---|
| Returning after the PRD / zg design | [HANDOFF](HANDOFF.md) first |
| Any VGI work | [README](README.md) (DAG + milestone) |
| Product intent, non-goals, review resolutions | [overview](overview.md) |
| Corpus path, `doc_id`, ingest, fingerprint | [corpus](corpus.md) |
| Token windows / bge-m3 batching | [token-batch](token-batch.md) |
| zvec M0 spike | [zvec-spike](zvec-spike.md) |
| Hybrid identity, RRF, read, rg | [retrieval](retrieval.md) |
| How recall is measured | [eval](eval.md) |
| Tree, lexical engine, tools, sandbox, cloud | [deferred](deferred.md) |
| Agent misreads | [log](log.md) |

## Process

1. **Scope.** Open [README](README.md). Implement only docs in the current milestone. Done when every in-scope module is named and out-of-scope docs stay closed.
2. **Spec first.** Behavior changes land in the module doc: interface, semantics, worked example, anchor. The worked example wins if it disagrees with the prose. Done when the example and the anchor still agree, and numbers in the anchor were computed by hand (or by `anchors/test_anchors.py`, which encodes those hand values).
3. **Regenerate.** One agent per in-scope doc. That agent sees its doc plus the *interfaces* of `depends_on` — not the previous module source. Write code into `VGI_ROOT` (default `../vgi-rs`, outside this product workspace). Done when anchors pass and the module has no hand-edits that the doc does not state.
4. **Log.** Record misread prose and failed anchors in [log](log.md), pointing at the doc that caused them.

## Bounds

VGI is an evaluation tool for a fixed document corpus. Independent `vgi-rs`: do not add its crates, engines, or CI to the product workspace. Credentials still come from `avrag-rs/.env` when an embedding HTTP call is in scope.

M0 in-scope: `corpus`, `token-batch`, `zvec-spike`.

M1 in-scope: `embed`, `retrieval` (vector path + read + rg), `eval`. Do not treat official 26.4%/59.7% as a pass line.

M2 in-scope: `retrieval` lexical path (passage-level BM25, tantivy) + RRF hybrid surface (`search`/`eval --mode`). Gate: official-equivalent BM25 oracle (bm25s lucene stand-in, frozen calibration) then hybrid ≥ best single path on Challenge-20. Still out: tree, MCP, sandbox, field boost, self-built postings.

A second vector index `qwen-flash` (bailian `qwen3.7-text-embedding-flash`, 128k windows, 256-d fp16) is allowed beside bge-m3. Do not overwrite `window_vectors` or `zvec-docs`.
