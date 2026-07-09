# Workspace 全量命名决策（2026-07-09）

## 产品拍板

| # | 决策 |
|---|------|
| 1 | HTTP **只保留** `/workspaces/*`，**删除** `/notebooks/*` 双挂 |
| 2 | **删除** `frontend_rust`（产品不维护第二前端） |
| 3 | **存储也迁移**：`notebooks`→`workspaces`，`notebook_id`→`workspace_id`，`notebook_members`→`workspace_members` |
| 4 | 无旧客户端；未上线，**不做长期 alias 兼容**（API/JSON/代码公开层一律 workspace） |
| 5 | 记忆/偏好/画像：仅 Chat + WebSearch 体验增强（另波） |
| 6 | Agent 继续以 Loop 为平台演进（另波） |
| 7 | Admin 要真架构（另波）；测试分册（另波） |
| 8 | **AppState 产品路径只认 Bound**（W2）：`docs/chat/admin_api/admin_ops/share/prefs/billing_api` |

## 迁移

- SQL：`avrag-rs/migrations/0055_workspace_rename.{up,down}.sql`
- 应用层：SQL 字符串与 Rust 字段随 0055 对齐；contracts/前端生成物统一 workspace

## 明确不做

- C4 合并 Capability/Skill/Tool
- 为旧 notebook API 保留双路径
