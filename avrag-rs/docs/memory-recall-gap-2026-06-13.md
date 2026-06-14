# 记忆按需检索空壳 GAP（2026-06-13）

> **状态**：已确认（代码级核实）。待排期修复。
> **范围**：Agent 多轮记忆的「按需调阅更早历史」链路，以及已删中间层（L2 session summary）的残留死代码。
> **关联**：
> - `prompts-memory-doc-profile-optimization-2026-06-10.md`（记忆分层重构 + `resolved_query` 一等列）
> - `e2e-codegen-memory-tests-design-2026-06-10.md`（记忆 E2E 测试设计，§3.3 / Case S3）
> - `superpowers/specs/2026-05-28-e2e-analysis-framework-design.md:230`（曾零散提及 "Missing `conversation_history_tag` in tool catalog"，本文档系统化记录）
> - `query-library-design-2026-06-14.md`（**交互侧解法**：用户主动组织的"提示词库"前端设计，对应 §6 选项 3）

---

## 1. 一句话结论

Agent「按 tag 定向调阅历史」这条能力是**断的**：写标签的工具是空壳、底层写库逻辑零调用、标签提示词已废弃。`message_tags` 表在生产中**永远为空**，因此带 tag 的定向召回恒返回空，实际只剩「拉最近 N 条原文」这一条纯数量召回——而最近 3 轮本就默认注入，"按需调阅"的增量价值因此被架空。

---

## 2. 断裂链（逐环证据）

按"写入 → 存储 → 读取 → 暴露"四个环节核查，**写入侧三处全断**：

### 2.1 写标签工具的 `execute` 是空壳

`crates/app-chat/src/agents/skills/builtin/conversation_history.rs:212-228`，`ConversationHistoryTag::execute` 只统计 `operations` 数量后原样返回，**从不调用任何持久化方法**：

```212:228:avrag-rs/crates/app-chat/src/agents/skills/builtin/conversation_history.rs
    async fn execute<'a>(&self, args: &Value, _ctx: &'a ExecutionContext<'a>) -> ToolResult {
        let operations = args
            .get("operations")
            .and_then(|v| v.as_array())
            .map(|arr| arr.len())
            .unwrap_or(0);

        ToolResult {
            tool: self.id().to_string(),
            version: self.version().to_string(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({
                "operation_count": operations,
            })),
            trace: None,
        }
    }
```

注意它返回 `status: Ok` —— agent 会以为打标签成功了，实际什么都没发生。

### 2.2 底层写库逻辑零生产调用方

`crates/storage-pg/src/lib_impl/repository_conversation_memory.rs:100` 的 `apply_tag_operations`（真正 `INSERT/DELETE message_tags` 的事务实现）功能完整，但**全仓搜索零调用方**（除自身定义与历史 plan 文档外无引用）。即写库能力存在，却没有任何代码路径接到它。

### 2.3 标签提示词已废弃，工具注册却残留

- 工具仍在 registry 注册：`crates/app-chat/src/agents/skills/builtin/mod.rs:22` `register(Box::new(ConversationHistoryTag))`。
- 但其 SKILL 提示词位于 `prompts/deprecated/atomic-tools/conversation-history-tag/SKILL.md`（**deprecated 目录**）。
- memory 簇对外暴露的工具清单 `prompts/clusters/memory/SKILL.md` 只列了 `conversation_history_load` 与 `user_profile_load`，**未列 `conversation_history_tag`**。

结论：agent 在记忆场景下基本不会被告知"可以打标签"，即便被告知并调用，也命中 2.1 的空壳。

### 2.4 读取侧真实，但无数据可读

`crates/app-chat/src/agents/skills/memory_dispatch.rs:26` → `load_history_by_tags`（`repository_conversation_memory.rs:21`）是真实查询，带 tag 时按 `message_tags` JOIN 过滤。因 2.1–2.3，被过滤的标签从不存在，故**带 tag 调用恒空**；不带 tag 时退化为 `ORDER BY id DESC LIMIT n` 的最近 N 条。

### 2.5 表结构就绪

`migrations/0034_conversation_memory.up.sql` 已建 `message_tags` 表及索引。基础设施齐备，唯独无人写入。

---

## 3. 设计意图 vs 实际行为

| 维度 | 设计意图 | 实际行为 |
|------|----------|----------|
| agent 标注关键历史 | `conversation_history_tag` 打 tag 供日后定向召回 | 调用恒成功假象，`message_tags` 不落库 |
| 按主题/相关性调阅 | `conversation_history_load(tags=[...])` 定向取回 | 恒返回空 |
| 兜底全量分析 | `conversation_history_load()` 不带 tag | ✅ 可用，最近 N 条原文倒序 |
| 增量价值 | 取回 3 轮窗口之外的**相关**历史 | 仅能取回更早历史的**最近若干条**，无相关性定向 |

实际召回既非「关键词检索」也非「语义检索」，而是退化的「时间倒序 + 数量上限」。

---

## 4. 影响面

- **产品**：宣传中的"agent 智能调阅相关历史"在生产不成立；长会话里第 4 轮以前的内容只能靠时间倒序碰运气取回，无法按主题定位。
- **token 浪费 + 误导**：若 agent 听信注册描述去打标签，会生成无效的 tag 操作 payload，消耗 token 且自以为成功。
- **测试盲区**：`e2e-codegen-memory-tests-design-2026-06-10.md` 的 Case S3 用 `conversation_history_load` **不带 tag**（`{"limit":20}`）断言非空——恰好绕过 tag 路径，**无法暴露本 gap**。需要补一条「打 tag → 带同 tag 取回应非空」的端到端断言才能守住。

---

## 5. 关联残留：L2 session summary 死代码

中间层（每 N 轮生成会话摘要）的**触发逻辑已移除**：`crates/app-chat/src/chat/service_postprocess.rs:172` 的会话后持久化只调用 `maybe_update_structured_profile`（L3 dream），不再生成 L2 摘要。但以下脚手架残留：

- `crates/chatmemory/src/lib.rs`：`Layer2Summary` 结构、`ChatMemory::update_summary`、`load()` 中 `layer2: summary.map(...)` 读取。
- `crates/rag-core-ports/src/chat_persistence.rs:156` `update_session_summary` trait 方法 + PG 实现（`repository_sessions_jobs.rs:70`）+ adapter 转发（`pg_chat_persistence.rs:288`）。
- `chat_sessions.summary` 列与 `build_rag_session_context(messages, summary)` 仍接收 summary 入参。

由于无写入方，`summary` 恒为空，读取链路是空转。属可清理死代码（与本 gap 同源，故一并记录；清理与否独立决策）。

---

## 6. 修复选项（backlog，未排期）

按工作量从小到大，互不冲突：

1. **诚实下线**：移除/隐藏 `conversation_history_tag` 注册，避免误导 agent；保留不带 tag 的最近 N 条召回。成本最低。
2. **接线闭环**：`ConversationHistoryTag::execute` 接到 `apply_tag_operations`，把 tag 工具补进 memory 簇 `SKILL.md`，并补 §4 的端到端测试。让定向召回真正可用。
3. **改范式（另议）**：若判定"agent 自动打 tag"本身 ROI 不高，可放弃 tag 路线，改走用户侧主动组织——见 `query-library-design-2026-06-14.md`（自动「提示词库」：自动存 user query + 模糊搜索 + 点击插入光标，把"复用历史表达"的控制权交还用户）。
4. **清理 L2 残留**：移除 §5 列出的 session summary 死代码与 `summary` 入参/列（或确认保留作为未来扩展位）。

> 注意：选项 2 与选项 3 路线互斥，需先定方向再动手。

---

## 7. 复现 / 验证方法

- 静态：`rg "apply_tag_operations" avrag-rs/` 仅见定义，无调用方即为本 gap。
- 运行：任一会话连发 ≥4 轮后，调 `conversation_history_load(tags=["<任意>"])`，返回 `message_count == 0`；不带 tag 则返回最近 N 条。
- DB：查 `SELECT count(*) FROM message_tags`，生产恒为 0（测试 `replay.rs` 路径除外）。
