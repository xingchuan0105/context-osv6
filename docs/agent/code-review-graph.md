Scope: code structure, relationships, search, and index maintenance. Entry point: [repository agent rules](../../AGENTS.md).

# Knowledge Graph (code-review-graph) — query **and** update

**code-review-graph is first-class code intelligence for this monorepo** (replaces graphify). Prefer it over ad-hoc repo-wide grep when the task is architectural, relational, or impact-oriented. The graph is a **maintained artifact**: reading it without refreshing after structural edits is incomplete work.

## Paths & setup

| Item | Value |
|------|--------|
| Project root | `/home/chuan/context-osv6` |
| Graph storage | `.code-review-graph/` (SQLite, gitignored; do **not** commit) |
| Ignore file | `.code-review-graphignore` at repo root (extra excludes on top of git-tracked files) |
| Windows map | `Z:\home\chuan\context-osv6\...` |

CLI binary: `code-review-graph` on `$PATH` (pipx), or `uvx --from code-review-graph code-review-graph`. MCP server name: `code-review-graph` (`code-review-graph serve`).

First use in a fresh clone: `code-review-graph build` (full parse; subsequent `update` runs are incremental, typically < 2 s).

## When to **query** (must call code-review-graph)

Use code-review-graph **before** large greps for any of:

* How does X work? / what calls Y? / architecture overview
* Module / crate / feature boundaries and neighborhoods
* Blast radius or "what breaks if we change Z?"
* Dependency chain between two concepts
* Review impact of uncommitted changes or a diff

**MCP workflow** (preferred): tools on the `code-review-graph` server — start with `get_minimal_context_tool` (~100 tokens), then drill in with `get_review_context_tool`, `get_impact_radius_tool` (blast radius of changed files), `query_graph_tool` (callers/callees/tests/imports/inheritance), `traverse_graph_tool` (BFS/DFS with token budget), `detect_changes_tool` (risk-scored review impact), `list_graph_stats_tool` (size/health), `get_architecture_overview_tool`, `get_hub_nodes_tool`, `get_bridge_nodes_tool`. If no graph exists yet, call `build_or_update_graph_tool` first.

**CLI equivalents** (when MCP is unavailable):

```bash
cd /home/chuan/context-osv6
code-review-graph build           # first-time full parse
code-review-graph status          # graph stats
code-review-graph detect-changes  # risk-scored impact of current diff
```

After graph hits, open the cited files with the editor/read tools. Use `semble` for semantic chunk search, `tgrep` for repo-wide pattern search, and `rg` / `grep` for one-shot exact strings. Structure and relationship questions still start with **code-review-graph**.

## Repo-wide pattern search (tgrep)

- Binary: `~/.local/bin/tgrep`; the trigram index lives in `.tgrep/` at the repository root.
- Run from the root: `tgrep "pattern" .`. Supported ripgrep-like flags include `-i`, `-F`, `-g`, `-t`, `-l`, and `--json`.
- A subdirectory path looks for that tree's own index. Use `--index-path .tgrep` or a root-relative glob such as `-g 'avrag-rs/**'` to query the repository index.
- After large tree changes, rebuild with `tgrep index .`, following the root time-cost rule. Use the on-disk index; do not run `tgrep serve` on this WSL because of memory use.
- Never commit `.tgrep/` or `.code-review-graph/`.

## When to **update** the graph (mandatory bookkeeping)

After you **finish** a change set that alters code structure, **update the graph in the same session** (do not leave a stale graph for the next agent turn):

| Change type | Update? |
|-------------|---------|
| New/removed modules, crates, packages, public APIs | **Yes** |
| Renames, moves, package splits/merges | **Yes** |
| Non-trivial call-graph or dependency changes | **Yes** |
| Pure comment/docs/typo, formatting-only, lockfile-only | No |
| Config-only with no code symbol change | Usually no |

**Command** (run from monorepo root):

```bash
cd /home/chuan/context-osv6
code-review-graph update    # incremental: re-parses changed files only
```

Notes:

* `code-review-graph update` is local AST parsing only (no LLM). Prefer it for routine maintenance.
* Output stays under `.code-review-graph/` (gitignored). Never stage it unless the user explicitly asks.
* If the graph looks stale or broken after a huge reshape, delete `.code-review-graph/` and run `code-review-graph build` for a clean rebuild.

## Session checklist

1. **Explore / design / impact** → query the graph first (MCP tools or CLI).
2. **Implement** surgical code changes.
3. **If structure changed** → `code-review-graph update` before claiming done.
4. **Verify** with `code-review-graph status` / `list_graph_stats_tool` that the graph is current and healthy.
