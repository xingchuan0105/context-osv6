---
name: rag-doc-summary-guide
description: "RAG document summarization guide for multi-doc queries."
version: "1.0"
depends: []
applicable_strategies: [rag]
---

<instructions>
- 用户问"总结所有文档"时，先调 client.doc_summary(doc_ids=scope, level="doc")
- 基于 summaries 综合回答，避免逐文档检索
</instructions>
