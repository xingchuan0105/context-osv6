Status: extracted from AGENTS/CLAUDE for progressive disclosure. AGENTS.md links here.

# Rust target / resource policy (WSL)

**Default: local `target/` per workspace** (Cargo default). Do **not** point every worktree at one shared multi-10G target unless you explicitly opt in.

| | Default | Opt-in shared |
|--|---------|----------------|
| Where | `avrag-rs/target`, `frontend_rust/target` | `~/.cache/context-osv6/target/...` |
| Enable | nothing (after deactivate) | `bash scripts/activate-rust-cache.sh --shared-targets [--migrate-main-targets]` |
| Disable | `bash scripts/deactivate-rust-cache.sh` | — |
| Disk | per tree, easier to clean | one huge dir (can be 50G+) |
| Concurrency | safer for multi-session / agents | one cargo at a time only |

## Avoid OOM / disk thrash

* Cap compile jobs: `avrag-rs/.cargo/config.toml` has `[build] jobs = 2`. Override with `CARGO_BUILD_JOBS=N` or gitignored `local-machine.toml`.
* Prefer sccache for cross-tree reuse (`rustc-wrapper`), not hard-shared `target/`.
* Do not stack concurrent full `cargo test` / `cargo build` against the same tree.
* Hygiene: `bash scripts/rust-disk-hygiene.sh check` (then prune carefully). Shared cache after deactivate is optional to delete.

**product-dev-up:** uses `${AVRAG_DIR}/target` by default (not the shared cache).
