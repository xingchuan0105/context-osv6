# Loop runtime messages

**Primary role:** model-visible **observations** injected into the agent message list.  
**Not** cluster skills; **not** in `PromptRegistry`. Loaded by `agent-loop` via `include_str!`.

## Channels (2026-08-10)

| Channel | Path | Audience |
|---------|------|----------|
| **Model** | `prompts/loop/*.md` observations / repair / C5 | LLM only |
| **Disaster user prose** | `prompts/loop/disaster/*` | User bubble **only** when format gate or token ceiling leaves no legal model draft — full replacement, **never** a host footnote on a draft |
| **Forbidden** | ~~evidence-missing-disclosure~~ / ~~verify-ceiling-disclosure~~ | Removed: host must not append diagnostics to `answer` |

See root `AGENTS.md` §「User channel」and `docs/engineering/2026-08-10-harness-llm-user-channel-philosophy-diagnosis.md` §17.

## Voice

Third-person **what happened / what is true** for model observations. No “please / you must”. Hard gates live in code. Disaster lines are short **human** product copy (librarian tone), not run diagnostics.

## Naming: `codegen-*` files

Filenames such as `codegen-no-output.nudge.md` refer to the **sandbox execution implementation** (historical “codegen bridge”), **not** the product skill id.

| Product skill / capability | Loop observation family |
|----------------------------|-------------------------|
| `knowledge-base` skill + KB capability | `codegen-*.md` sandbox observations |
| `search` skill + 联网 capability | same sandbox family when `client.web` / `fetch` runs |

## Files (live) — model channel

| File | When used |
|------|-----------|
| `blocks-skipped.nudge.md` | Extra code blocks in one turn (`{n_blocks}`, `{n_skipped}`) |
| `budget-pace-over-baseline.tmpl.md` | Soft pace when `round > baseline_rounds` (`{round}`, `{baseline}`, `{max_rounds}`, `{remaining_rounds}`) — not a hard stop |
| `budget-pace-near-ceiling.tmpl.md` | Soft pace when `remaining_rounds ≤ 1` — last hard retrieve slot(s) |
| `budget-exhausted-final.nudge.md` | C5 rounds exhausted + had retrieval attempt (prose + SELECTED / `[[web:n]]`) |
| `budget-exhausted-final-tokens.nudge.md` | C5 token ceiling + had retrieval attempt |
| `budget-exhausted-final-no-attempt.nudge.md` | C5 rounds exhausted + **no** retrieval tool attempt |
| `budget-exhausted-final-tokens-no-attempt.nudge.md` | C5 token ceiling + **no** retrieval tool attempt |
| `budget-exhausted-carryover.tmpl.md` | Last successful tool payload (`{tool}`, `{body}`) — only when attempt exists |
| `codegen-no-output.nudge.md` | Empty sandbox round |
| `codegen-sandbox-error.nudge.md` | Sandbox error recovery facts (`{n_fail}`, `{n_max}` consecutive threshold) |
| `evidence-index.tmpl.md` | Per-round expand/card/stub counts |
| `claim-notes.tmpl.md` | P1″ cumulative claim board |

| `working-set-trimmed.nudge.md` | Near-round expanded bodies demoted under char budget |
| `history-cleared.nudge.md` | Older retrieval observation bodies stubbed |
| `evidence-reread.tmpl.md` | Synthesis-time EWS recency reread (`{items}`) |
| `codegen-untrusted-prefix.nudge.md` | Untrusted tool-output prefix |
| `native-tools-closed.tmpl.md` | Closed native model surface rejection |
| `format-hint-*.nudge.md` | Table pattern mismatch hints in code |
| `retrieval-summary.tmpl.md` | Per-round retrieve counts + `{detail}` |
| `lead-plan-context.tmpl.md` | Lead 规划上下文（`{caps_rag}`, `{caps_search}`, `{workspace_note}`, `{doc_scope_note}`, `{doc_lines}`） |
| `task-brief.tmpl.md` | Lead→Worker 简报 JSON（`{brief_json}`） |
| `evidence-pack.tmpl.md` | Worker→Lead pack JSON（`{pack_json}`；宿主 PackGate 后） |
| `retrieval-worklog.tmpl.md` | 检索工作日志投影（`{query}`, `{log_lines}`；源自 run 事件日志的 surface 事件） |
| `lead-workers-handoff-synthesis.tmpl.md` | Lead+Workers 检索结束→合成环境事实（`{n_packs}`） |
| `rebrief-wave.tmpl.md` | 宿主结构补检索波次（`{rebrief_used}`, `{channels}`） |
| `rag-worker-sac.tmpl.md` | RAG Worker 短程 SaC 环境（`[rag_worker_sac]`） |

## Lead+Workers system 拼装（非 loop 文件，索引）

| 路径 | 角色 |
|------|------|
| `system/agent-base.md` | 会话公共层（身份 / 用户信道 / BASE） |
| `system/lead-base.md` | Lead（规划+grounded 合成） |
| `system/worker-sandbox.md` | Worker 沙箱精简基座 |
| `clusters/lead/SKILL.md` | Lead 合成/Brief 细则 |
| `workers/{rag,web}/SKILL.md` | 通道 Worker |
| `pipeline/lead-plan.system.md` | Lead 规划 JSON LLM |
| `retrieval-summary-detail-*.tmpl.md` / `*-nudge.md` | `{detail}` fragments |
| `synthesis-repair.nudge.md` | Invalid synthesis JSON (non–prose_only paths) |
| `synthesis-prose-repair.tmpl.md` | prose_only final-form repair; `{violation_detail}` |
| `final-answer-feedback-*.md` | Per-rule `{violation_detail}` (incl. `provider-protocol` for DSML) |
| `partial-evidence-insufficient.md` | Short partial-evidence line (salvage path) |
| `retrieval-failed-final.nudge.md` | Empty-evidence final-turn observation (model) |
| `evidence-missing.nudge.md` | L2 证据闸：已调用、无 answer-grade |
| `evidence-missing-no-client.nudge.md` | L2：尚未有检索侧调用 |
| `required-action-missing-*.tmpl.md` | L2.5 必做动作 |
| `selected-protocol.nudge.md` | 合成前 SELECTED 形态说明 |
| `user-facing-closeout.nudge.md` | verify fail 到顶且仍有 token：模型收束轮观察 |
| `synthesis-rerender.tmpl.md` | 证据池重渲染轮 |
| `verify-fail-synthesis.tmpl.md` / `verify-fail-retrieve.tmpl.md` | verify 回环 |
| `verify-empty-advice.md` / `verify-empty-evidence.md` | verify 占位 |
| `verify-draft-under-revision.tmpl.md` | 回合成时注入 `{draft}` |

## Files (live) — disaster user prose only

| File | When used |
|------|-----------|
| `disaster/format-exhausted.md` | Format gate exhausted (draft+repair+rerender) or illegal out-bound after closeout/token ceiling |
| `disaster/no-evidence.md` | No-evidence path disaster (rag / dual) |
| `disaster/search-no-evidence.md` | No-evidence path disaster (search) |
| `disaster/default.md` | Generic disaster |

## Retired (deleted 2026-08-10)

- `evidence-missing-disclosure.md` / `evidence-missing-disclosure-no-attempt.md` — host footers on answers  
- `verify-ceiling-disclosure.md` — host ceiling footer  
- `contract-violation-*.md` / `degraded-no-evidence-*.md` — superseded by `disaster/*`

## Retired (legacy tests)

`../deprecated/loop-legacy/no-chunk-*.md` — host no-chunk continue/grace. Kept for unit tests via `prompt_assets` only.

Placeholders (loop): `{n_blocks}`, `{n_skipped}`, `{call_count}`, `{total_chunks}`, `{detail}`, `{tool}`, `{body}`, `{violation_detail}`, `{action}`, `{n_fail}`, `{n_max}`, `{expanded}`, `{cards}`, `{stubs}`, `{expand_chars}`, `{pool_aliases}`, `{lines}`, `{n}`, `{max}`, `{aliases}`, `{n_aliases}`, `{new_aliases}`, `{seen_aliases}`, `{parts}`, `{draft}`, `{advice}`.
