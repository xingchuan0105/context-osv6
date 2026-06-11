# 归档索引：已被 ADR-0007 取代或部分取代的文档

> **Coding agent**：实现 ReAct 披露、orchestrator、skill 簇、tool 按需传递时，**只以** [ADR-0007](../adr/0007-react-phased-context-disclosure.md) **为准**。本页所列文档仅供历史对照，**不得**作为实现依据。

**权威顺序**：ADR-0007 > ADR-0006/0005 仍有效章节 > 本归档列表中的文件。

---

## 完全归档 / 退役（勿读入 agent 上下文）

| 文档 | 原因 |
|------|------|
| [progressive-disclosure-framework.md](progressive-disclosure-framework.md) | v4 PLAN/EXECUTE 状态机；已由 ReActLoop + ContextAssembler 取代 |
| [../adr/0006-unified-agent-loop.md](../adr/0006-unified-agent-loop.md) | 早期稿 |
| [../../.handoff-0006-stage1.md](../../.handoff-0006-stage1.md) | 会话 handoff，含过时 `native_tools` |
| [../../.handoff-0006-all-stages.md](../../.handoff-0006-all-stages.md) | 同上 |
| [../../prompts/skills/rag-plan/SKILL.md](../../prompts/skills/rag-plan/SKILL.md) | 退役 planner |
| [../../prompts/skills/search-plan/SKILL.md](../../prompts/skills/search-plan/SKILL.md) | 退役 planner |
| [../../prompts/skills/chat-plan/SKILL.md](../../prompts/skills/chat-plan/SKILL.md) | 退役 planner |
| [../../prompts/skills/rag-eval/SKILL.md](../../prompts/skills/rag-eval/SKILL.md) | 退役；Rust evaluator 承担 |
| [../../prompts/skills/search-eval/SKILL.md](../../prompts/skills/search-eval/SKILL.md) | 同上 |
| [../../prompts/skills/rag-memory-mgmt/SKILL.md](../../prompts/skills/rag-memory-mgmt/SKILL.md) | 退役；PG history |
| [../../prompts/skills/session-summary/SKILL.md](../../prompts/skills/session-summary/SKILL.md) | 产品废弃 session_summary |

## 部分废止（仅部分章节过时）

| 文档 | 仍可读 | 以 ADR-0007 为准的议题 |
|------|--------|------------------------|
| [../adr/0006-unified-agent-loop-revised.md](../adr/0006-unified-agent-loop-revised.md) | ReAct+Synthesis 动机、Loop 骨架 | `native_tools`、`disclosure.rounds`、skill 累积 Index 行为 |
| [../architecture-review-2026-06.md](../architecture-review-2026-06.md) | 产品/架构观察 | `format_hint` 硬注入、`chat-plan` 格式路由 |
| [../../prompts/skills/chat/SKILL.md](../../prompts/skills/chat/SKILL.md) | 对话行为主体 | `session_summary` 输入描述需删除 |
| [../../prompts/skills/rag-citation-format/SKILL.md](../../prompts/skills/rag-citation-format/SKILL.md) | — | 契约迁至 `rag-system` §5 |
| [../../prompts/skills/url-citation-format/SKILL.md](../../prompts/skills/url-citation-format/SKILL.md) | — | 契约迁至 `search-system` §5 |

## ADR-0007 核心决策速查

- 无 `native_tools` → `tool_pool` + 按需 schema + messages 内 tool trace
- 无 `session_summary` → PG messages only
- Orchestrator 五段式；RAG/Search **§5 citation 契约**
- 检索簇：codegen / search / memory；Synthesis：writing + format（Agent 自选）
- 退役：rag-plan、search-plan、chat-plan、rag-eval、search-eval、rag-memory-mgmt

最后更新：2026-06-08（ADR-0007 v0.5）
