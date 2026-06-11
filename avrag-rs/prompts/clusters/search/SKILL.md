---
name: search
description: "Load when a search-mode query needs decomposition into sub-queries, vertical selection (web vs news), multi-source verification, or credibility labeling. Skip for single straightforward keyword lookups that one web_search call can answer."
disclose_at: retrieve
atomic: true
applicable_modes: [search]
---

## 核心指令

### 垂直选择

| 场景 | 垂直 | 说明 |
|------|------|------|
| 通用事实、教程、产品信息 | `web` | 默认 |
| 近期新闻、事件、股价 | `news` | 时效性强 |
| 需全文核对 | `web_search` 后再 `web_fetch` | 仅当摘要不足 |

### 查询分解

1. 比较类（A vs B）→ 并行子查询各搜一侧
2. 多实体（三人成就）→ 每实体独立子查询
3. 时间敏感 → 子查询含年份/「latest」

### 可信度标注

- **高**：政府、标准组织、原始论文、官方文档
- **中**：主流媒体、知名技术博客、维基（作起点非终点）
- **低**：论坛、无署名页面、营销稿 → 必须标注「来源可信度有限」
- 多源冲突 → 并列呈现差异，不静默合并

简单查询可直接 `web_search`；复杂路径请求本簇后按上策略执行。

## 禁止

- 禁止在 Search 模式使用 codegen / SDK 代码块
- 禁止编造来源或夸大可信度
- 禁止忽略多源冲突而不标注
