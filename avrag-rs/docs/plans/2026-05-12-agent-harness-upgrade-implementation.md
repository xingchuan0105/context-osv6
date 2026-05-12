# Agent Harness 升级实施计划 (2026-05-12)

> **状态：已废弃**
> **原因**：产品定位调整，Agent Harness 升级已全部回撤（commit `f8407c1`）
> **历史**：本文记录原计划的实施步骤，保留作参考。

---

## 回撤说明

原计划在 `2026-05-12-agent-harness-upgrades.md` 中设计的三项升级：
1. 真 tool-use 循环（AgentLoop）
2. 滑动窗口替换 session_summary
3. Skill 按需加载

因产品定位调整，核心聚焦知识库检索+网络检索，Agent 协作由用户自选的 Claude Code/Hermes 提供，故全部回撤。

---

## 已回撤的代码

| 文件 | 状态 |
|------|------|
| `crates/app/src/agents/agent_loop.rs` | 已删除 |
| `crates/app/src/agents/tool_registry.rs` | 已删除 |
| `crates/app/src/agents/rag_tools.rs` | 已删除 |
| `crates/llm/src/client.rs` (tool 相关方法) | 已恢复 |
| `crates/app/src/agents/chat_agent.rs` | 已恢复为旧版 |
| `crates/app/src/agents/rag_agent.rs` | 已恢复为旧版 |
| `crates/app/src/agents/web_search_agent.rs` | 已恢复为旧版 |
| `docs/agent-harness-chat-tools.md` | 已删除 |

---

## 保留的参考结论

### Skill 两层加载机制
```
Layer 1: 启动时扫描 skills/ 目录，把 name + description 注入 system prompt
Layer 2: 模型调用 load_skill("pdf") 时返回完整内容
```
可用于优化当前 system prompt 体积，避免一次性塞入所有 skill 内容。

### 三层滑动窗口
```
Layer 1 (Hot):  最近 N 轮完整保留
Layer 2 (Warm): 上一段窗口摘要
Layer 3 (Cold): 更早的只保留关键事实
```
可用于替换当前 `session_summary` 单点摘要，改善长会话 token 控制。

---

## 废弃的内容

- AgentLoop 共享循环驱动
- AgentToolRegistry 工具注册表
- 多 Agent 协作（队友、消息总线、任务认领）
- 后台任务管理（BackgroundManager）
- 关闭协议 / 计划审批
