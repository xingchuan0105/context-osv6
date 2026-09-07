# Subtex prompt assets（A 线目录插件）

Subtex MCP 工具面与约定模板的 LLM-facing 文案。加载方（W2 起）：`transport-http` `tools/subtex.rs` 以 `include_str!` 载入，Rust 侧不硬编码任何面向模型的句子。产品边界见 PRD `docs/plans/2026-09-06-directory-plugin-prd.md`，波次见 `docs/plans/2026-09-06-subtex-m1-dev-plan.md`。

| 文件 | 加载方 | 用途 |
|------|--------|------|
| `convention-draft.md` | `subtex.init` / `subtex.convention_draft` | 约定建议稿骨架（Agent 审阅后合并进 `AGENTS.md` 的 Subtex 管理段落） |
| `correction-draft.md` | `subtex.correction_draft` | 一次纠偏后管理段落的更新稿骨架 |
| `tools/<tool>.md` | `transport-http` `mcp/catalog.rs`（`include_str!`） | 7 个 `subtex.*` 工具的描述文案：`init` / `status` / `convention-draft` / `correction-draft` / `search` / `outline` / `transcribe` |

Authoring rules（root `AGENTS.md`）：第三人称环境陈述（返回什么 / 何事为真），不带指令腔；`{…}` 占位符由运行时以目录事实替换；不放入任何真实语料实体名。
