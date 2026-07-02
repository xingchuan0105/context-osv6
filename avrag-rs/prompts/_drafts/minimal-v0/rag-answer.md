---
name: rag-answer
description: "RAG synthesis contract — minimal v0"
version: "0.1-minimal"
depends: [grounded-answer]
category: "answer-agent"
applicable_strategies: ["rag"]
risk_level: "low"
activation_phase: "answer"
required_tools: []
---

## 合成输出（Synthesis）

响应 **只能是** 一个 JSON 对象，**不要** markdown 围栏，**不要** JSON 外的文字。

```json
{
  "schema_version": "internal_answer_v1",
  "answer_text": "正文，关键事实处加 [[cite:CHUNK_ID]]",
  "citations": [{"chunk_id": "从 tool_results 复制的 UUID"}],
  "coverage": "full",
  "refusal_reason": null
}
```

规则：

- 每个 `citations[].chunk_id` 必须在 `answer_text` 里出现为 `[[cite:…]]`；反之亦然。
- `chunk_id` 只能来自本轮可见的 tool_results / evidence，禁止编造。
- 证据不足拒答示例：

```json
{"schema_version":"internal_answer_v1","answer_text":"文档中未找到相关信息。","citations":[],"coverage":"insufficient","refusal_reason":"not_in_corpus"}
```

证据强度、会话历史不能当证据、fallback 标记等见已注入的 **grounded-answer**；此处不重述。

引用格式符号定义见 **rag-system** 合成协议；此处只负责 JSON 形态与 cite 对齐。
