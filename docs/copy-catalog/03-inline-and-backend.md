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

- 文案：`avrag-rs/email/*.{zh,en}.txt`（`email/README.md`）
- 加载：`app-bootstrap` `services/email_copy.rs`（`include_str!` + `{placeholder}`）
- 发送：`password_reset.rs` → `send_reset_email` / `send_workspace_invite_email` / `send_plain_email`
- 重置密码：`lang` API 字段 → `MailLocale`（默认 zh）
- 邀请：`locale_zh`（share handler 仍常 true；可后续接用户偏好）
- **状态**：P7 邮件切片 **done**；站内通知见下表仍 open

## 站内通知（bell title/body）

- 文案：`avrag-rs/notifications/*.{zh,en}.txt`（`notifications/README.md`）
- 加载：`common::notification_copy`（默认 zh）
- 已接通：入库成功/失败、余额不足、密码变更、开启分享、订阅支付/到期、账单更新、对话降级
- **状态**：P7 站内通知切片 **done**

## 仍 open：API / 管道错误硬编码（非 bell）

| 文件 | 字符串 |
|------|--------|
| `transport-http/middleware.rs` | "Share chat rate limit exceeded for this visitor." |
| `transport-http/.../notes.rs` / `analysis.rs` | "Workspace not found" |
| `transport-http/.../profile.rs` | "Password update failed"（API 响应，非通知） |
| `storage-pg` / `ingestion` / `app-documents` | "Document uploaded but no previewable text was extracted." |
| `app-chat/.../service.rs` | "Share query exceeds max length…" |
| `llm/section_index.rs` | "Document title: Title" |

## 其它

| 源 | 说明 |
|----|------|
| `avrag-rs/crates/agent-loop/src/progress/labels.rs` | 短进度标签 |
| `frontend_next/content/legal/` | 法律 MDX |
| `frontend_next/public/legal/` | LICENSE / third-party |
| `frontend_next/public/docs/` | 帮助 md |
