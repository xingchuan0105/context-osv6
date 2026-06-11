---
name: memory
description: "Load when the user refers to earlier conversation beyond the 3 turns runtime already injected, asks about past preferences or prior decisions, or needs continuity that recent turns do not cover. Skip for self-contained questions answerable from the current turn and default injected history. Do NOT load merely because the message contains pronouns — Pre-Loop already resolves those into resolved_query."
disclose_at: retrieve
atomic: false
applicable_modes: [rag, search, chat]
---

## 核心指令

**Runtime 已注入**：最近 3 轮用户原文（`[prior_user_query]`）+ Pre-Loop 写入的 `resolved_query`（检索用，messages 仍见原话）。

**按需调取**（本簇）：
1. 更早历史 → `conversation_history_load`（超出保底 3 轮时）
2. 长期画像/偏好 → `user_profile_load`（跨会话偏好、专业领域、表达风格时）

1. 检索默认使用 `resolved_query`；回答面向用户原始措辞
2. 消解后仍歧义 → 澄清，不臆造实体
3. 需要指代规则细节时加载 `reference/anaphora.md`

## Reference 路由表

| 文件 | 内容 |
|------|------|
| `reference/anaphora.md` | 指代消解边界（runtime 已做基础消解） |

## 禁止

- 禁止无视服务端已展开的 `resolved_query` 重新猜测实体
- 禁止在歧义未解时编造指代对象
- 禁止把「记忆」窄化为仅指代消解——近轮由 runtime 注入，更早历史与画像由本簇工具调取
