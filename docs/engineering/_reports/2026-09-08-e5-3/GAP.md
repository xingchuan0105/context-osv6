# E5.3 gap 表关闭记录

对照 `docs/plans/2026-09-06-frontend-rust-ui-parity-plan.md` §3 E5.3 范围。

| 项 | 状态 | 证据 |
|---|---|---|
| hero 空态（品牌标题 + 模式提示 + 居中 composer） | 关闭 | `chat-hero` / `.chat-canvas.is-hero`；Playwright hero 用例 |
| composer 自动伸缩 + 拖拽手柄（ARIA slider） | 关闭 | `role=slider`，pointer 拖高，ArrowUp/Down；autosize 写 height |
| 代码块语言标签 + 复制按钮 | 关闭 | `decorate_code_blocks`；`chat-code-copy` 写剪贴板 |
| 图片 figure 卡 | 关闭 | `wrap_figures`；`chat-figure` |
| `tool_result` 工具卡 | 关闭 | Done `tool_results` → `tool-result-card` |
| web 来源计数按钮 + 弹窗 | 关闭 | `web-sources-button` / `workspace-web-sources-modal` |
| 流式光标 | 关闭 | `.chat-live-answer.is-streaming::after`（仅 Streaming） |
| 打字机平滑 | **豁免** | 见下 |
| 进度耗时计时 | 关闭 | `workspace-progress-elapsed`，格式对齐 Next `Ns` / `Nm Ns` |
| 自动滚动 + 「回到底部」 | 关闭 | follow-bottom Effect；`scroll-to-bottom` |
| 用户消息编辑进 composer | 关闭 | `edit-user-message` |
| degrade/guard 通知条 | 关闭 | `chat-degrade-notice`（Done `degrade_trace` / `guard_report.blocked`） |
| 会话栏 loading / error+retry / empty + 流式导航锁 | 关闭 | `session-loading` / `session-list-error` / `session-empty`；streaming 时 session item disabled |
| 移动端会话抽屉 | 关闭 | `chat-rail-toggle` + `is-rail-open`；≤47.9375rem |

## 豁免：打字机平滑

**理由：** Next 有独立 typewriter 显示缓冲。Rust 侧 token 已直写同一套 `ChatCanvas` 主气泡，再加队列会变成第二套呈现管线，违反 C7（一套 ChatCanvas）。可见对齐用流式光标（`.is-streaming::after`），`prefers-reduced-motion` 下关闭闪烁。

**决策人：** 本切片执行（solo trunk）。不另开 typewriter buffer。
