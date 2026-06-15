# 记忆按需检索 GAP（2026-06-13 → 2026-06-14 已修复）

> **状态**：已修复（2026-06-14）。Tag 路线已下线；改为 PG 近序 + jieba 分词 FTS 混合检索，默认 `notebook` scope（同工作区跨 session，不跨 notebook）。
> **保底注入**：当前 query + **2** 条 prior user（`MAX_PROMPT_HISTORY_TURNS = 2`）。

## 实现摘要

| 项 | 说明 |
|----|------|
| Migration | `0043_chat_message_fts.up.sql`：`search_tokens` + `search_vector` GIN；DROP `message_tags` |
| 分词 | `common::segment_for_fts` / `merge_search_tokens`（`jieba-rs`） |
| 检索 | `search_conversation_history`：近序候选 ∪ FTS 候选 → RRF 融合 |
| 工具 | `conversation_history_load`：`query` + `scope`（`notebook`/`session`）；移除 `conversation_history_tag` |
| 未做 | retrieve 阶段自动 memory search；跨 notebook |

## 历史背景（tag 空壳，已废弃）

此前 `conversation_history_tag` 不落库、`message_tags` 恒空，带 tag 的 load 恒失败。详见 git 历史本文件 2026-06-13 版。

## L2 session summary 残留

中间层触发与 `Layer2Summary` / `update_session_summary` / `chat_sessions.summary` 列已清理（migration `0044`）。
