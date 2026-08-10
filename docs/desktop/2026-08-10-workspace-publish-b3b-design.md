# Workspace Publish B3b — 本地 → 云端设计门禁（ADR-0010 §5）

- **日期**: 2026-08-10  
- **状态**: 设计门禁（实现未开全量；地基已有）  
- **依据**: `docs/adr/0010-share-service-business-model.md` §5 / §10 B3b–B4  
- **非目标本波**: 增量 dirty 同步（B4）、LiteLLM、桌面买断  

## 1. 问题

ADR-0010：**本地建库免费**；**分享对外**需云端可服务副本。  
现状：

| 能力 | 状态 |
|------|------|
| 云端 `DocumentIndexBatch` **写入** | 已有（Milvus/pgvector `replace_document_index`） |
| `RetrievalExportPort` + pgvector impl | **已有**（含 vector 读回） |
| 包格式常量 `workspace_publish_bundle_v1` | **已有** |
| HTTP `POST …/publish` + 分片上传 | **无** |
| 桌面打包 UI / 进度 | **无** |
| 身份映射 local→cloud Owner | **未产品化** |
| 开 `share_enabled` 时要求 ready 云副本 | **未强制**（本地可假分享） |

因此 v0.2.0 客户端可本机用，但**不能**诚实承诺「本机库一键上云分享」。

## 2. 目标切片（B3b 最小可交付）

```text
已登录云账号的 Owner
  → 本机选 Workspace
  → 导出 bundle（manifest + per-doc index.batch + meta）
  → 分片上传云 API
  → 云端校验 embedding 指纹
  → PG 元数据 upsert + replace_document_index
  → publish_status = ready + last_published_at
  → 此后才允许 share_enabled=true（本地源）
```

### 2.1 Bundle（v1）

与 ADR §5.5 对齐：

```text
WorkspacePublishBundle v1
├── manifest.json   # cloud_user_id, local_workspace_id, embedding_model_id,
│                   # vector_dim, schema_version, doc counts, content hashes
├── docs/{doc_id}/meta.json
├── docs/{doc_id}/index.batch.jsonl  # zstd optional
└── docs/{doc_id}/assets/...         # optional first ship: skip assets
```

### 2.2 API 草图（云端）

| Method | Path | 说明 |
|--------|------|------|
| POST | `/api/v1/workspaces/{id}/publish/sessions` | 创建上传会话；返回 `upload_id` + 分片大小 |
| PUT | `/api/v1/workspaces/{id}/publish/sessions/{upload_id}/parts/{n}` | 分片 body |
| POST | `/api/v1/workspaces/{id}/publish/sessions/{upload_id}/commit` | 校验指纹 → 导入 |
| GET | `/api/v1/workspaces/{id}/publish/status` | `never\|publishing\|ready\|dirty\|failed` |

鉴权：Bearer 云用户；写入 `owner_user_id` = 该用户。

### 2.3 桌面侧

- 调用本机 `RetrievalExportPort`（pgvector）组包  
- 进度事件：pack → upload → commit → ready  
- 失败：展示指纹不匹配 / 余额不足（若 commit 触发平台 embedding 重嵌分支）

### 2.4 产品闸

- 本地 Workspace：`share_enabled` 前检查 `publish_status == ready`（或「纯云建库」跳过）  
- UI：分享面板无 ready 时 CTA「先发布到云端」而非假开链接  

## 3. 依赖顺序

1. manifest + fingerprint 字段落 PG（migration）  
2. commit 导入路径（复用 `replace_document_index`）  
3. 分片会话存储（对象存储或临时盘）  
4. 桌面 export 命令 + UI  
5. 分享闸接线  

## 4. 验证

- 单测：export round-trip 向量 dim 一致  
- E2E：本机 1 文档 → publish → 云端 share chat 命中同一答案要点  
- 负例：错误 `vector_dim` → commit 拒绝  

## 5. 刻意不做（本设计）

- 双向实时协作编辑  
- 无登录匿名 Publish  
- 重嵌默认路径（首版要求指纹一致，失败即拒）  

## 6. 与 v0.2.0 关系

| 包 | 交付 |
|----|------|
| **0.2.0** | 免费客户端 + 本机栈 + 云端 BYOK/分享（已在云） |
| **下一 MINOR**（建议 0.3.0） | B3b Publish E2E |

## 变更

| 日期 | 说明 |
|------|------|
| 2026-08-10 | 初稿；与 activate/buy 退役同波登记 |
