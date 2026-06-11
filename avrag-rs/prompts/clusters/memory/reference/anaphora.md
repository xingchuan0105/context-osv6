# 指代消解边界

## Runtime 已做

1. Pre-Loop 将消解结果写入 `resolved_query`（PG 一等列 + metadata）；`content` 保留用户原话
2. 下游检索/上下文默认读 `resolved_query`，缺省回退原话
3. ReAct messages 中仍可见用户原始 `query`——展示与审计用

## Agent 职责

1. 代词（it/this/that/它/这/那）→ 以 `[prior_user_query]` + `resolved_query` 为上下文，勿重复猜测
2. 消解后仍歧义（多个同等候选实体）→ 向用户澄清，不臆造
3. 跨轮保持主题与实体连续性；勿发明上文不存在的实体

## 常见误判

- 用户切换话题后旧指代失效 → 以最近一轮明确实体为准
- 消解读错时原话仍在 PG → 用户可纠正，不会污染历史
