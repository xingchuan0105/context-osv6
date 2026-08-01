---
name: metadata
description: "Load when the user asks which documents exist in the workspace, authors/types overview, or when you need document ids for doc_profile/doc_summary without a prior content search. Skip if one dense search already surfaces the needed docs."
disclose_at: retrieve
atomic: false
applicable_modes: [rag]
---

## 何时加载本说明

在回复中输出**整段** JSON（不要夹其它字）：

```json
{"skill_request": ["metadata"]}
```

下一轮会注入知识库文档清单。适用于：

- 用户问知识库有哪些文档、作者、类型
- 需要全局概览（多文档对比、统计）
- 需要 `doc_id` 再调 `doc_profile` / `doc_summary`，又不想先做内容检索
- 内容检索回传为空，想先看清单再换查询词

单文档内的具体内容：直接用 `client.dense` / `client.lexical` / `client.grep` 通常更合适。

## 加载后会看到什么

注入一段 `<docscope_metadata>…</docscope_metadata>` 包裹的 JSON，列出**当前知识库全部文档**的元数据（一次给全量，不能只要子集）。形状示例（虚构 id，非真实语料）：

```json
{
  "documents": [
    {
      "doc_id": "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
      "filename": "example-report.docx",
      "docname": "示例报告",
      "language": "zh",
      "domain": "business",
      "genre": "report",
      "era": "contemporary",
      "author": null,
      "publication_date": null
    }
  ],
  "profile": {
    "languages": ["zh"],
    "domains": ["business"],
    "genres": ["report"],
    "eras": ["contemporary"]
  }
}
```

- `documents[]`：每篇一条；`doc_id` 用于后续 `doc_profile` / `doc_summary`。
- `profile`：语言/领域/体裁等聚合列表，便于看知识库整体特征。
- `domain` / `genre` / `era` 可能为 `"unknown"`：表示系统未识别，不要当成有效标签。
- `author` / `publication_date` 为 `null`：只表示未识别，不能当成「文档一定没有作者/日期」。

## 拿到 doc_id 之后

```python
profile = await client.doc_profile(doc_ids=["aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"])
summary = await client.doc_summary(doc_ids=["aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"], level="doc")
```

`doc_ids` 可省略：省略时按当前知识库默认范围解析；只有要**收窄**到某几篇时才显式传入（用 `doc_id` 字符串，不是文件名）。

## 边界

- 本说明在 `skill_request` 的**下一轮**才注入清单；未加载前不要假设清单已在上下文中。
- 文档很多时 JSON 会较大；同一会话内通常只注入一次。
- 默认首轮只有检索代码说明；需要清单时再 `skill_request` metadata。
