# M1 + M2 Contract Notes

当前阶段优先对齐以下接口与事件：

## REST

- `GET /health`
- `GET /ready`
- `GET /docs`
- `GET /openapi.json`
- `GET /api/v1/workspaces`
- `POST /api/v1/workspaces`
- `GET /api/v1/workspaces/{id}`
- `PUT /api/v1/workspaces/{id}`
- `DELETE /api/v1/workspaces/{id}`
- `POST /api/v1/workspaces/{id}/documents`
- `POST /api/v1/workspaces/{id}/sources/url`
- `GET /api/v1/documents`
- `GET /api/v1/documents/{id}/status`
- `GET /api/v1/documents/{id}/content`
- `GET /api/v1/documents/{id}/parsed-preview`
- `PUT /api/v1/documents/{id}`
- `DELETE /api/v1/documents/{id}`
- `DELETE /api/v1/workspaces/{id}/documents/{doc_id}`
- `POST /api/v1/documents/{id}/reindex`
- `POST /api/v1/chat`
- `GET /api/v1/chat/sessions`
- `POST /api/v1/chat/sessions`
- `GET /api/v1/chat/sessions/{id}`
- `GET /api/v1/chat/sessions/{id}/messages`
- `DELETE /api/v1/chat/sessions/{id}`
- `POST /api/v1/chat/citations/lookup`

## SSE

当前阶段支持以下事件：

- `start`
- `trace`
- `planner_complete`
- `rag_trace`
- `rag_sources`
- `token`
- `citations`
- `done`
- `error`

## Request Context

当前 API 默认接受本地开发上下文，并附加：

- 响应头 `x-request-id`

可选请求头：

- `x-request-id`
- `x-owner-user-id`
- `x-user-id`

若未提供租户与用户头，服务回退到本地开发默认 UUID。

## Upload / Reindex Runtime Behavior

- `POST /api/v1/workspaces/{id}/documents`
  - 只登记文档并返回 `upload_url`
  - PostgreSQL 模式下不直接生成 chunks
- `PUT /dev-upload/{document_id}`
  - PostgreSQL 模式下写入 `AVRAG_OBJECT_ROOT` 下的原始对象路径并入队 `ingestion_tasks`
  - Memory 模式下保留本地模拟摄取
- `POST /api/v1/documents/{id}/reindex`
  - PostgreSQL 模式下入队 `reindex_document`
  - Memory 模式下保留本地模拟状态流转

当前 Wave 0 worker 在 PostgreSQL 模式下会完成：

- 认领 `ingestion_tasks`
- 从对象根目录读取原始内容
- 写 summary chunk 与 body chunks
- 推进文档状态到 `completed`

## 明确降级

当前实现不执行真实 RAG 检索，`rag` 模式的回答由内存态文档占位上下文驱动，并返回：

- `degrade_trace`
- `planner_output`
- `mode_debug`

这样前端可以先完成调试面板、降级提示与引用渲染。
