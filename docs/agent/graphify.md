Status: extracted from AGENTS/CLAUDE for progressive disclosure. AGENTS.md links here.

# Knowledge Graph (graphify) — query **and** update

**Graphify is first-class code intelligence for this monorepo.** Prefer it over ad-hoc repo-wide grep when the task is architectural, relational, or impact-oriented. The graph is a **maintained artifact**: reading it without refreshing after structural edits is incomplete work.

## Paths

| Item | Value |
|------|--------|
| Project root (preferred `project_path`) | `/home/chuan/context-osv6` |
| Default graph | `graphify-out/graph.json` (gitignored; do **not** commit) |
| Backend-focused graph (optional) | `avrag-rs/graphify-out/graph.json` |
| Windows map | `Z:\home\chuan\context-osv6\...` |

CLI binary: `graphify` on `$PATH`, or `uvx --from "graphifyy[mcp]" graphify` / Grok MCP server `graphify` (`python -m graphify.serve`).

## When to **query** (must call graphify)

Use graphify **before** large greps for any of:

* How does X work? / what calls Y? / architecture overview
* Module / crate / feature boundaries and neighborhoods
* Blast radius or "what breaks if we change Z?"
* Shortest path / dependency chain between two concepts
* PR / change impact against communities (MCP: `list_prs`, `get_pr_impact`, `triage_prs` when relevant)

**Grok MCP workflow** (required discovery path):

1. `search_tool` with query `graphify` (or the specific tool name).
2. `use_tool` with `tool_name` like `graphify__query_graph` / `graphify__god_nodes` / `graphify__shortest_path` / `graphify__get_neighbors` / `graphify__get_node` / `graphify__graph_stats` / `graphify__get_community`.
3. Always pass `project_path` when the session cwd is not the monorepo root, e.g. `"project_path": "/home/chuan/context-osv6"`.

**CLI equivalents** (when MCP is unavailable):

```bash
cd /home/chuan/context-osv6
graphify query "authentication flow"
graphify path "ConversationApp" "ToolCatalog"
graphify explain "WorkspaceApp"
```

After graph hits, open the cited files with the editor/read tools. Use `semble` for semantic chunk search; use **graphify** for structure and relations; use **grep** only for exact literal / exhaustive string checks.

## When to **update** the graph (mandatory bookkeeping)

After you **finish** a change set that alters code structure, **update the graph in the same session** (do not leave a stale graph for the next agent turn):

| Change type | Update? |
|-------------|---------|
| New/removed modules, crates, packages, public APIs | **Yes** |
| Renames, moves, package splits/merges | **Yes** |
| Non-trivial call-graph or dependency changes | **Yes** |
| Pure comment/docs/typo, formatting-only, lockfile-only | No |
| Config-only with no code symbol change | Usually no |

**Commands** (run from monorepo root unless work is entirely under `avrag-rs/` and that tree has its own graph):

```bash
cd /home/chuan/context-osv6
graphify update .
# After large deletions/refactors that shrink the graph:
graphify update . --force
# Optional: recluster + report only (existing graph.json):
graphify cluster-only .
```

Notes:

* `graphify update` is **code extract only** (no LLM). Prefer it for routine maintenance.
* If MCP stderr mentions pre-#1504 node IDs or stale IDs after a big reshape: `graphify extract --force` (or `update --force`) then re-query.
* Output stays under `**/graphify-out/` (gitignored). Never stage `graph.json` / `GRAPH_REPORT.md` unless the user explicitly asks.
* If both root and `avrag-rs` graphs exist and you only changed backend Rust, update `avrag-rs` (or root if that is the graph you queried). Prefer **one primary graph per task** and pass the same `project_path` for query + update.

## Session checklist

1. **Explore / design / impact** → query graphify first (with `project_path`).
2. **Implement** surgical code changes.
3. **If structure changed** → `graphify update .` (or `--force`) before claiming done.
4. **Verify** with a quick `graphify query` / `graph_stats` or MCP `graph_stats` that the new symbols appear or old ones are gone as expected.
