---
description: Thin shell — follow repo-root ../AGENTS.md (authoritative) for this workspace.
---

**Follow [`../AGENTS.md`](../AGENTS.md) (repo root) — it is authoritative for `avrag-rs` too.** The rules that bite most often here:

- **Prompts:** LLM-facing prose lives only under `prompts/**/*.md`; never inline in Rust. Layout: `prompts/README.md`; loop assets: `prompts/loop/README.md`.
- **Product rules:** T1–T8 and execute/workspace/org rules in `../docs/agent/product-apps.md` are non-negotiable.
- **Credentials:** read `.env` (+ `.env.example`) and reuse configured values silently; never re-ask.
- **Verify:** targeted `cargo test -p <pkg> --lib`; respect `jobs=2`, never stack full `cargo test` runs (`../docs/agent/rust-resources.md`).
