# LLM guidance and product agent boundaries

Scope: product prompt assets, their callers, and the agent harness. These rules describe the product's LLMs; they do not direct the coding assistant's own tool use or delegation. Inherits [repository design principles](../../AGENTS.md).

## Prompt location

All LLM-facing instruction or guidance prose belongs in `avrag-rs/prompts/**/*.md`, never inline in Rust or other product code. Runtime code may load, substitute placeholders, and assemble authored assets.

- Capability copy: `prompts/capabilities/<id>/{contract.md,SKILL.md,reference/}`.
- Skill/orchestrator/synthesis assets: the families in the [prompt layout](../../avrag-rs/prompts/README.md), including clusters, agent guide, hints, and templates.
- Loop observations and repairs: `prompts/loop/*.md`, loaded through `react_loop/prompt_assets.rs`; see the [loop asset index](../../avrag-rs/prompts/loop/README.md).
- Tool stdout/retrieval JSON is observation data, not a place to author instructions. Placeholders such as `{n_blocks}` and `{tool}` may live in code.
- Never embed realistic-corpus/golden-set queries, answers, entity names, or evaluation numbers in prompts, loop code, or fixtures presented as product policy.

Exceptions to prompt authoring: pure control tokens, detector regex/keyword lists that are not injected as instructions, short UI progress labels, and machine-stable error codes. Text shown to a model as instructions or user-turn guidance belongs in `prompts/`.

## Host markers

Any tag injected into model context must first be registered in `avrag-rs/crates/agent-loop/src/react_loop/host_markers.rs`. Emitters reference the constant; detectors derive from the registry. The parity test rejects unregistered tags in `prompts/loop/*.md`.

## Voice: third-person observation

All model-facing guidance describes what happened or what is true in the environment, including capability/skill bodies, checks, loop nudges, synthesis repair, and format hints. Runtime reports state; the model decides the next action. Do not duplicate hard gates with a prose instruction layer.

| Prefer | Avoid |
|---|---|
| 本轮检索观察中仍未出现 answer-grade 命中。 | 禁止终答。请继续用 client 检索。 |
| 草稿里问题侧 A 有 observation 支撑；侧 B 仍未见命中。 | 请再写一个 code 块补检 B。 |
| 管道表中一行是一条记录；`total_hits` 是命中行数。 | 应/必须/不要/禁止 dedupe。 |
| 沙箱本轮 stdout/stderr 为空，且未发生 client.* 调用。 | 请检查代码路径并修复。 |

Few-shots use situation → observation → facts, not imperative steps. Pure machine tags and detector lists are outside this voice rule. User-facing disaster fallback copy lives in `prompts/loop/disaster/` and is not a host footnote.

## User channel

The LLM owns the user's main answer, including explanations, clarifications, and questions. The harness owns tool execution, observations, state, and telemetry.

- Do not append host observations, protocol fragments, disclosure/ceiling footnotes, or runtime diagnostics to the main answer.
- Handle failures inside the loop. At the ceiling, use an LLM closeout turn while tokens remain; only token exhaustion or exhausted format gates permit a narrow disaster replacement.
- Empty evidence, verification failure, and ceilings become telemetry/eval labels, not mirrored diagnostic text in the user's answer.
- Outbound format gates catch protocol leakage such as DSML and trigger repair or the disaster path. Do not parse provider-private tool protocols as product capabilities.

Rationale and failure cases: [harness/LLM/user-channel design, §17](../engineering/2026-08-10-harness-llm-user-channel-philosophy-diagnosis.md).

## Retrieval orchestration

Product requests whose `capabilities[]` contain `rag` and/or `search` use `LeadWorkers`. Chat and write-refine remain outside this retrieval orchestration contract.

| Role | Responsibility | Boundary |
|---|---|---|
| Lead | Resolve references, decompose Task Briefs, schedule Workers, adjudicate coverage, synthesize user prose and citations | No direct dense/web retrieval; obtain more evidence by re-briefing Workers |
| RAG Worker | Short SaC with dense/lexical/grep → `evidence_pack_v1` | No web calls or user final answer |
| Web Worker | Host search leaves, multiple queries and CRW → `evidence_pack_v1` | No dense/grep calls or user final answer |
| Host | Brief/PackGate structural checks, recomputed `tool_ok_count`, at most one re-brief, outbound format gate, progress Delegate | No semantic completeness veto, refusal keyword bars, or user-bubble footnotes |

- `task_brief_v1` is the Lead-to-Worker contract. `evidence_pack_v1` is the Worker-to-Lead contract; it has no self-reported grounding flag and is checked by the host PackGate.
- Worker Continue is an internal retrieval round; re-brief is a further dispatch, limited to one. Lead owns coverage, grounding, and whether/how to answer at synthesis time.
- No independent verify LLM, host multi-entity completeness scanner, or long-term single-brain KB/web union on this path.
- BASE tools (`weather_query`, `calculator`, `user_context`, etc.) belong to Lead or pure chat, not `preferred_source: rag|web`. A successful weather tool result supports weather statements without an EvidencePack requirement.
- DirectAnswer is available for chat/write-refine or a Lead decision of `base_tools`/`none`; retrieved user answers come from Lead synthesis. `HostWeb` direct-answer assembly is not a product path.
- `require_evidence` expresses Lead/skill intent to anchor key claims in observations/packs. Host counts successful tools; required retrieval with zero Ok produces `evidence_missing_continue`, a structural observation and Continue.
- Token/round budgets reserve capacity for Lead synthesis. Budget observations and internal pack tags stay outside the user's main answer.

Contract schemas, detailed mechanisms, and historical implementation progress: [Lead + Workers design](../plans/2026-08-11-lead-rag-web-workers-design.md). This guide states constraints, not completion or deployment status.
