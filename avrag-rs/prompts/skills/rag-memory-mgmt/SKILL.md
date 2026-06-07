---
name: rag-memory-mgmt
description: "RAG memory management guide for recall and remember operations."
version: "1.0"
depends: []
applicable_strategies: [rag]
---

<instructions>
- client.recall(tags, limit)：加载历史消息
- client.remember(operations)：标记消息用于后续召回
- 涉及跨会话上下文时先 recall 再回答
</instructions>
