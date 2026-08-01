# Loop runtime messages (model-visible observations)

Injected into the agent message list (or used as fixed user-facing fallback lines).
**Not** cluster skills; **not** in `PromptRegistry`. Loaded by `agent-loop` via `include_str!`.

## Voice

Third-person **what happened / what is true**. No “please / you must”. Hard gates (if any) live in code; these files report facts. See root `AGENTS.md`.

## Naming: `codegen-*` files

Filenames such as `codegen-no-output.nudge.md` refer to the **sandbox execution implementation** (historical “codegen bridge”), **not** the product skill id.

| Product skill / capability | Loop observation family |
|----------------------------|-------------------------|
| `knowledge-base` skill + KB capability | `codegen-*.md` sandbox observations |
| `search` skill + 联网 capability | same sandbox family when `client.web` / `fetch` runs |

## Files (live)

| File | When used |
|------|-----------|
| `blocks-skipped.nudge.md` | Extra code blocks in one turn (`{n_blocks}`, `{n_skipped}`) |
| `budget-exhausted-final.nudge.md` | Budget exhausted final turn (SaC prose + SELECTED / `[[web:n]]`) |
| `budget-exhausted-final-tokens.nudge.md` | Same closing turn, token-ceiling variant (states token fact) |
| `budget-exhausted-carryover.tmpl.md` | Last successful tool payload (`{tool}`, `{body}`) |
| `codegen-no-output.nudge.md` | Empty sandbox round |
| `codegen-sandbox-error.nudge.md` | Sandbox error recovery facts |
| `codegen-untrusted-prefix.nudge.md` | Untrusted tool-output prefix |
| `native-tools-closed.tmpl.md` | Single fixed rejection for the closed native model surface (SDK method or superseded native tool name issued as a native tool call; error code `native_tools_closed`) |
| `format-hint-*.nudge.md` | Table pattern mismatch hints in code |
| `retrieval-summary.tmpl.md` | Per-round retrieve counts + alias/truncation detail |
| `synthesis-repair.nudge.md` | Invalid synthesis JSON (non–prose_only paths) |
| `synthesis-prose-repair.nudge.md` | prose_only code-only answer repair (one round) |
| `contract-violation-*.md` | User-facing format failure fallbacks |
| `degraded-no-evidence-*.md` | User-facing empty-evidence fallbacks |
| `partial-evidence-insufficient.md` | Short partial-evidence line |
| `retrieval-failed-final.nudge.md` | Degraded final turn when host uses empty-evidence path |

## Retired (not product-injected)

`../deprecated/loop-legacy/no-chunk-*.md` — host no-chunk continue/grace. Kept for unit tests via `prompt_assets` only.

Placeholders (loop): `{n_blocks}`, `{n_skipped}`, `{call_count}`, `{total_chunks}`, `{detail}`, `{tool}`, `{body}`.

`pipeline/table-supervision/obs-*.md` observations (`{sql}`, `{rows}`, `{table_id}`, …) are **not** loaded here: they are rendered by the `avrag-struct-supervision` `prompts.rs` mini engine (`include_str!` + key/block/pick substitution).
