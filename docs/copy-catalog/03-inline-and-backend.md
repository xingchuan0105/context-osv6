# 内联中英 & 后端硬编码线索

## 前端 `locale === "zh-CN" ? … : …`（已迁移至 i18n）

> 状态（2026-08-06）：原 **106 处** 内联已全部迁入 `frontend_next/lib/i18n/messages/*.ts`（键见 `01-ui-i18n-full.md`）。
> 组件内不再出现产品文案三元；剩余 `locale` 判断均为**合理保留**：

| 保留项 | 位置 | 原因 |
|--------|------|------|
| `✓ ` 勾选标记 | `components/account-menu.tsx:208` | 语言选择器标记，非文案 |
| `typeLabel.zh/en`、`modelLabel.zh/en` | `settings-providers-panel.tsx` FIXED_ROWS | 行内双语数据结构（字典层） |
| `copy.zh / copy.en` 读取 | `components/admin/i18n/labels.ts` | admin 组件独立字典的读取层（copy.ts） |
| `Intl.NumberFormat / DateTimeFormat / RelativeTimeFormat` | `settings-shared.ts`、`share-center-utils.ts`、`settings-billing-panel.tsx` | 需要 locale 代码（`zh-CN`/`en`），非文案 |
| `join("、" vs " / ")` | `workspace-sources-pane.tsx` | 分隔符格式逻辑 |

> 已顺带修复：
> - `dashboard-surface.tsx` 的 `"\u6765\u6e90"` 字面转义 → i18n 键（源码可读性 + 统一）
> - `dashboard-utils.ts` 的 `\u4e2a\u6765\u6e90` 转义 → i18n 键
> - `chat-message-list.tsx` / `web-sources-modal.tsx` 的「{n} 个来源」→ 单复数模板键
> - `citation-renderer.tsx` 引用 aria 标签 → 模板键

## 邮件服务

- 文件：`avrag-rs/crates/app-bootstrap/src/services/password_reset.rs`
- 函数：`send_reset_email` / `send_workspace_invite_email` / `send_plain_email`
- 邀请信：主题/正文中文模板（`is_zh` 参数当前常 true）
- **状态**：另行立项（P7 未纳入本轮）

## 通知 / 系统消息硬编码（英文居多，另行立项）

| 文件 | 字符串 |
|------|--------|
| `avrag-rs/crates/transport-http/src/middleware.rs` | "Share chat rate limit exceeded for this visitor." |
| `avrag-rs/crates/transport-http/src/handlers/workspaces/notes.rs` | "Workspace not found"（×6） |
| `avrag-rs/crates/transport-http/src/handlers/workspaces/analysis.rs` | "Workspace not found" |
| `avrag-rs/crates/transport-http/src/lib_impl/auth/profile.rs` | "Password update failed" / "Password changed" |
| `avrag-rs/crates/storage-pg/src/lib_impl/errors_and_mappers.rs` | "Document uploaded but no previewable text was extracted." |
| `avrag-rs/crates/llm/src/section_index.rs` | "Document title: Title" |
| `avrag-rs/crates/ingestion/src/chunker.rs` | "Document uploaded but no previewable text was extracted." |
| `avrag-rs/crates/app-chat/src/chat/service.rs` | "Share query exceeds max length of {max_chars} characters." |
| `avrag-rs/crates/app-chat/src/chat_private/mod.rs` | "Balance needed" |
| `avrag-rs/bins/worker/src/sources.rs` | "Document ingestion completed" / "Document ingestion failed" |

## 其它

| 源 | 说明 |
|----|------|
| `avrag-rs/crates/agent-loop/src/progress/labels.rs` | 短进度标签 |
| `frontend_next/content/legal/` | 法律 MDX |
| `frontend_next/public/legal/` | LICENSE / third-party |
| `frontend_next/public/docs/` | 帮助 md |
