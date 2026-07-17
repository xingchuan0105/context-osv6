---
name: capability-rag
description: "RAG capability manual — workspace document retrieval via codegen."
version: "1.1"
depends: []
category: "system-prompt"
applicable_strategies: [rag]
---

## 能力：工作区知识库（RAG）

你当前 **已启用** 工作区文档检索。文档事实须来自检索 observation；未见的内容不要当作文档事实。

### 可见上下文

- 用户原话 query（服务端不做指代消解）
- `<iteration_budget round="..." max="..." remaining="..." />`
- 注入的 `client` 对象（方法见 **codegen** skill）
- 本轮及历史 retrieval chunks（`chunk_id`、正文等）
- 默认近期 prior user；更早历史需申请 **memory** cluster
- 已加载 skill（默认含 **codegen**）

默认不可见：互联网（除非同时有 Search 说明书）、本地文件系统、完整文档列表（除非 **metadata** 或结果中的 `doc_id`）。

### 检索轮协议

`remaining > 0` 且仍需证据时：只输出 **一个** `<code language="python">` 块（沙箱只执行第一个）。  
块内可多条 `await client.*(...)`；不要夹杂自然语言。

申请 skill 时只输出 JSON，例如：

```json
{"skill_request": ["metadata"]}
```

证据已够或 `remaining = 0`：停止检索输出。最终回答由运行时 **合成阶段** 注入契约与 answer skill，本说明书不规定合成 envelope 或文体。

### 引用符号（文档）

文档事实：`[[cite:CHUNK_ID]]`；图片：`[[image:CHUNK_ID]]`。  
不要用 Web 序号 `[[n]]` 标记文档事实。`chunk_id` 必须来自 observation，原样复制。

### 约束

- 检索入口只有 `<code language="python">` + `client.*`。
- 未启用 Search 时不要编造网页来源。
