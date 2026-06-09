---
name: search
description: "复杂搜索策略：查询分解、垂直选择与结果验证"
disclose_at: retrieve
atomic: true
applicable_modes: [search]
---

## 何时加载

- **Search 模式**检索轮；Round 0 可按需请求本原子簇
- RAG / Chat 不可用；Search 不走 codegen

## 核心指令

### 搜索策略

1. 分解复杂查询为多个子查询
2. 选择垂直领域：`web`（通用）或 `news`（新闻）
3. 必要时并行搜索多个子查询

### 结果验证

1. 比较多个来源的信息一致性
2. 优先官方、权威来源
3. 标注信息可信度

简单查询可直接 `web_search`；复杂路径请求本簇后按上策略执行。

## Reference 路由表

（原子簇：策略与验证规则均在正文，无独立 reference 文件。）

## 禁止

- 禁止在 Search 模式使用 codegen / SDK 代码块
- 禁止编造来源或夸大可信度
- 禁止忽略多源冲突而不标注
