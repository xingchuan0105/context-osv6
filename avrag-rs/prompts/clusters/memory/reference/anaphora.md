# 指代消解边界

## Runtime 注入（仅此）

1. 当前 query 原文
2. 最近 **2** 条 prior user 原文（`[prior_user_query]` 前缀）
3. 服务端**不再**做指代消解，不再生成 `resolved_query`（ADR-0010 废止 ADR-0008 的 Pre-Loop normalize）

## Agent 职责（你负责消解）

1. 看到代词（it/this/that/它/这/那）、指示词（这位/那位/这本书/那个概念）、省略（about it/谁写的）→ 检查 2 条 prior 是否够锚定实体
2. 不够 → 调 `conversation_history_load`（`scope=workspace` 默认）拉更早历史
3. 用最近一条相关 user turn 锚定实体
4. 消解后仍歧义（多个同等候选实体）→ 向用户澄清，不臆造
5. 跨轮保持主题与实体连续性；勿发明上文不存在的实体

## 常见误判

- 用户切换话题后旧指代失效 → 以最近一轮明确实体为准
- 2 条 prior 不足时跳过 history_load 直接猜 → 禁止；必须先拉历史
