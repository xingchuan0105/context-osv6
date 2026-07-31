# Loop runtime messages (model-visible observations)

Injected into the agent message list (or used as fixed user-facing fallback lines).
**Not** cluster skills; **not** in `PromptRegistry`. Loaded by `agent-loop` via `include_str!`.

## Voice

Third-person **what happened / what is true**. No “please / you must”. Hard gates (if any) live in code; these files report facts. See root `AGENTS.md`.

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
| `format-hint-*.nudge.md` | Table pattern mismatch hints in code |
| `retrieval-summary.tmpl.md` | Per-round retrieve counts + alias/truncation detail (`{call_count}`, `{total_chunks}`, `{detail}`) |
| `synthesis-repair.nudge.md` | Invalid synthesis JSON candidate (non–prose_only paths) |
| `synthesis-prose-repair.nudge.md` | prose_only code-only answer repair (one round) |
| `contract-violation-*.md` | User-facing format failure fallbacks |
| `degraded-no-evidence-*.md` | User-facing empty-evidence fallbacks |
| `partial-evidence-insufficient.md` | Short partial-evidence line |
| `retrieval-failed-final.nudge.md` | Degraded final turn when host still uses empty-evidence path |

## Retired (not product-injected)

`../deprecated/loop-legacy/no-chunk-*.md` — host no-chunk continue/grace. Kept for unit tests via `prompt_assets` only.

Placeholders: `{n_blocks}`, `{n_skipped}`, `{call_count}`, `{total_chunks}`, `{detail}`, `{tool}`, `{body}`.
