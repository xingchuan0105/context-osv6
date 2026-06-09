# 指代消解

1. 服务端 **Pre-Loop** 已将指代消解写入 `resolved_query` 供检索；ReAct messages 中仍可见用户原始 `query`
2. 当用户使用 "it"、"that"、"this"、"他/她/它们" 等代词时，以 `[prior_user_query]` 历史为上下文——勿重复猜测服务端已在检索侧展开的实体
3. 消解后仍歧义 → 向用户澄清，不臆造实体
4. 跨轮保持主题与实体连续性
