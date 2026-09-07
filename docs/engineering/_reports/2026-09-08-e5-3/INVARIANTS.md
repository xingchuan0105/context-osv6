# Chat-first 十二条不变量（E5.3 回归）

对照 `docs/plans/2026-09-05-w2-9-chat-first-gate2-task.md` §1。E5.3 只加呈现，不复制第二套画布。

| # | 不变量 | E5.3 后 | 证据 |
|---|---|---|---|
| 1 | 普通会话不暗建库 | 保持 | `chat-journey` W2.4 |
| 2 | Workspace 共用同一 ChatCanvas | 保持 | `ChatPage` 仍挂 `/chat` 与 workbench embed；W2.8 |
| 3 | 显式带入，不暗载工作区 | 保持 | `chat_canvas_lifecycle_tests` |
| 4 | 会话文件生命周期独立 | 保持 | `session_files_tests` / W2.4 |
| 5 | 最近会话区分归属 | 保持 | W2.8 列表 `[工作区名]` |
| 6 | 模型角色独立 | 保持 | W2.6 |
| 7 | Quick Chat BYOK 独立 | 保持 | W2.6 / `providers_tests` |
| 8 | Web 搜索正交 | 保持 | W2.5 |
| 9 | Scope 增量叠加 | 保持 | lifecycle tests |
| 10 | 完整聊天交互 | 保持并加编辑/停止锁 | W2.7 + E5.3 编辑/导航锁 |
| 11 | 受限上下文 | 保持 | `chat-journey` 不变量 11 & 12 |
| 12 | Snapshot/Evidence 不破坏会话 | 保持 | lifecycle 用例 15 |

结构：`ChatCanvasModel` 仍唯一；`reduce_chat_event` 仍唯一事件入口。
