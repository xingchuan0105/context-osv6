---
name: memory
description: "Load when the user refers to earlier conversation beyond the 2 prior user turns runtime already injected, asks about past preferences or prior decisions, or needs continuity that recent turns do not cover. Also load when the current query contains anaphora (pronouns, demonstratives, ellipsis) that reference entities from earlier turns — you are responsible for resolving them, the server no longer does it for you. Skip for self-contained questions answerable from the current turn and default injected history."
disclose_at: retrieve
atomic: false
applicable_modes: [rag, search, chat]
---

## 核心指令

**Runtime 已注入**：当前 query + 最近 **2** 条 prior user 原文（`[prior_user_query]`）。仅此而已——服务端不再做指代消解，也不再生成 `resolved_query`（ADR-0010）。

**你的职责**：看到代词、指示词、省略（"它"、"那位作者"、"这本书"、"about it"、"那个概念"）时，**主动调** `conversation_history_load` 拉取更早历史，自己消解指代。

**按需调取**（本簇）：
1. 更早或跨 session 历史 → `conversation_history_load`（`query` 用当前原话或提取的关键实体；默认 `scope=workspace`；近序 + 中文分词 FTS 混合检索）
2. 长期画像/偏好 → `user_profile_load`（跨会话偏好、专业领域、表达风格时）

## 指代消解流程（你负责）

1. 检查当前 query 是否含代词/指示词/省略
2. 若有，且 2 条 prior 历史不足以消解 → 调 `conversation_history_load`
3. 用拉到的历史里的**最近一条相关 user turn** 锚定实体
4. 消解后仍歧义 → 澄清，不臆造实体
5. 回答面向用户原始措辞，但检索可用消解后的实体词

## Reference 路由表

| 文件 | 内容 |
|------|------|
| `reference/anaphora.md` | 指代消解边界与典型模式 |

## 禁止

- 禁止假设服务端已替你消解指代——它没有
- 禁止在 2 条 prior 历史不足时跳过 `conversation_history_load` 直接臆造实体
- 禁止在歧义未解时编造指代对象
- 禁止把「记忆」窄化为仅指代消解——更早历史与长期画像也由本簇工具调取
