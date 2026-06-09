---
name: memory
description: "跨轮指代消解与会话连续性"
disclose_at: retrieve
atomic: false
applicable_modes: [rag, search, chat]
---

## 何时加载

- 检索轮 ClusterIndex 披露；三 mode 均可用
- 用户消息含代词/指代，或需跨轮连续性时，加载 `reference/anaphora.md`
- 服务端 Pre-Loop 已将消解结果写入 `resolved_query`；ReAct messages 仍见原始 `query`

## 核心指令

1. 将 `[prior_user_query]` 历史视为指代上下文
2. 检索使用 `resolved_query`；回答面向用户原始措辞
3. 消解后仍歧义 → 澄清，不臆造实体

## Reference 路由表

| 文件 | 内容 |
|------|------|
| `reference/anaphora.md` | 指代消解规则与边界 |

## 禁止

- 禁止无视服务端已展开的 `resolved_query` 重新猜测实体
- 禁止在歧义未解时编造指代对象
