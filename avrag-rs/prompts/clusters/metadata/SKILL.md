---
name: metadata
description: "Load when the user asks about the workspace's document inventory (which docs exist, who authored them, what genres/domains are present), or when you need doc_ids for doc_profile/doc_summary but don't want to dense_search first to discover them. Skip if a single dense_search is enough to surface the needed doc_ids."
disclose_at: retrieve
atomic: false
applicable_modes: [rag]
---

## 1. 何时请求本簇（C1）

输出 `{"skill_request": ["metadata"]}` 触发下一轮加载本簇。适用场景：

- 用户问"工作区里都有哪些文档"/"都有哪些作者"/"文档都是什么类型"
- 需要全局文档概览（多文档对比、统计）
- 需要 `doc_ids` 调 `doc_profile`/`doc_summary`，但不想先 `dense_search` 摸索
- `dense_search` 返回 0 chunk，怀疑是 query 不匹配——可以先看 metadata 了解文档主题再换 query

**不适用**：单文档内的具体内容查询——直接 `dense_search`/`lexical_search` 更高效。

## 2. 加载后你会看到什么（C2/C4）

本簇加载时，runtime 会注入 `<docscope_metadata>...</docscope_metadata>` 包裹的 **JSON**，包含**工作区所有文档**的元数据（全量，不能按 doc_id 子集请求）：

```json
{
  "documents": [
    {
      "doc_id": "doc-001",
      "filename": "thesis_y_refrigeration.docx",
      "docname": "Y冷冻设备公司营销策略研究",
      "language": "zh",
      "domain": "business",
      "genre": "thesis",
      "era": "contemporary",
      "author": null,
      "publication_date": null
    },
    {
      "doc_id": "doc-002",
      "filename": "huawei_ipd_370_activities.xlsx",
      "docname": "华为 IPD 流程 370 个活动",
      "language": "zh",
      "domain": "engineering",
      "genre": "manual",
      "era": "contemporary",
      "author": null,
      "publication_date": null
    }
  ],
  "profile": {
    "languages": ["zh"],
    "domains": ["business", "engineering"],
    "genres": ["thesis", "manual"],
    "eras": ["contemporary"]
  }
}
```

**字段说明**：
- `documents[]`：每个文档一条，含 `doc_id`/`filename`/`docname`/`language`/`domain`/`genre`/`era`/`author`/`publication_date`（`author`/`publication_date` 可能为 `null`）
- `profile`：去重聚合的 `languages`/`domains`/`genres`/`eras` 列表，用于快速判断工作区整体特征

**注意**：`domain`/`genre`/`era` 是服务端分类枚举，可能为 `"unknown"`——这意味着服务端没能识别该维度，不要把它当有效信息。

## 3. 拿到 doc_ids 后能做什么（C3）

从 `documents[]` 提取目标文档的 `doc_id` 后：

```python
# 拿 TOC / sections / metadata
profile = await client.doc_profile(doc_ids=["doc-001"])

# 拿文档级或章节级摘要
summary = await client.doc_summary(doc_ids=["doc-001"], level="doc")
summary = await client.doc_summary(doc_ids=["doc-001"], level="section")
```

`doc_profile`/`doc_summary` **必须传 doc_ids 非空**——服务端不会自动用 `doc_scope` 注入。详见 codegen skill 的"doc_scope 注入的不对称性"。

## 4. 加载边界（C4/C5）

- **全量加载**：本簇一旦请求，注入的是工作区所有文档的元数据，不能按 doc_id 子集请求。若工作区文档很多，JSON 会比较大——但这只是一次性注入，后续轮次不会重复注入（`already_disclosed` 去重）
- **round 0 不可直接用**：round 0 默认只加载 `codegen` cluster。必须先输出 `{"skill_request": ["metadata"]}`，**本轮不执行检索**，下一轮本簇 body + docscope_metadata 才会注入

## 5. 禁止

- 禁止在未加载本簇的情况下假设 `docscope_metadata` 已注入——它只在 `{"skill_request": ["metadata"]}` 后的下一轮出现
- 禁止把 `profile` 里的 `"unknown"` 当作有效分类信息
- 禁止把 `author: null`/`publication_date: null` 当作"文档无作者/无日期"的证据——只代表服务端未识别
