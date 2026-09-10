# GPUI 桌面对等清单 (GPUI_PARITY_CHECKLIST)

> D0 开工建立（权威计划 §7）。逐项记录 Windows / macOS / Linux 的实现、验证、产物与结果。
> 不照抄旧版号或旧“可选”结论；机器或签名条件不足的项标记 `待验`，不判整体完成。
> D1 只是中途里程碑，不用于宣布可删 Tauri。

状态图例：`未开始` / `抽库完成` / `实现` / `验证(win/mac/linux)` / `产物` / `待验(原因)` / `N/A(理由)`

## D0 宿主抽库（desktop/core，宿主无关）

| # | 能力 | 状态 | 证据 |
|---|---|---|---|
| D0.1 | 跨块中文 UTF-8 / CRLF / 坏帧 / 取消流解析（`SseDecoder` 复用） | 完成 (b3ce4ad7) | core 单测 + Windows 点验 |
| D0.2a | Windows 进程/控制台工具（win_cmd：CREATE_NO_WINDOW、kill tree、scoped kill） | 抽库完成 (本次) | core 编译；真机点验待 GD1 |
| D0.2b | 秘密文件权限（secret_fs：unix 0600 / Windows DACL） | 抽库完成 (本次) | 同上 |
| D0.2c | runtime 目录发现（install / AppData / monorepo / env 覆盖） | 抽库完成 (本次) | 单测 |
| D0.2d | native PG+Redis ensure/stop（initdb / 角色供给 / pgvector / client.env） | 抽库完成 (本次) | 单测 + Tauri 调库回归 |
| D0.2e | client.env 生成（jwt/byok/upload 签名、身份 uuid、解析器、云中继注入参数化） | 抽库完成 (本次) | 单测 |
| D0.2f | docker 探测与安装引导（非 Windows） | 抽库完成 (本次) | 单测 |
| D0.2g | 本机产品生命周期（avrag-migrate 迁移 → api/worker spawn、pid、健康、停止、scoped sweep） | 抽库完成 (本次) | 单测 + Tauri 调库回归 |
| D0.2h | 本地 B2C 会话（凭据文件、登录/注册、会话持久化） | 抽库完成 (本次) | 单测 |
| D0.2i | Tauri 命令薄包装（local_host.rs）+ HostError 转换 | 完成 (本次) | src-tauri check + 29 tests |

## D0.3 documents / REST / 上传 / 本地目录接口抽库

| # | 能力 | 状态 | 证据 |
|---|---|---|---|
| D0.3a | documents/reindex 命令逻辑抽库（token 由宿主注入） | 完成 (本次) | core tests (extract shapes) + src-tauri 薄包装回归 |
| D0.3b | api_call/upload_bytes 代理抽库（REST 代理、上传安全边界、zstd 导出解码、超时分级） | 完成 (本次) | core tests 7 例（上传 URL 拒绝远端/错路径/错端口、zstd 往返、路径归一） |
| D0.3c | 上传范围、失败恢复、目录与本地服务不可达行为（HostError 503 映射 + loopback/端口匹配断言） | 完成 (本次) | 同上测试 |

## D0.4 cloud session / Publish / 深链抽库

| # | 能力 | 状态 | 证据 |
|---|---|---|---|
| D0.4a | cloud session（登录/登出/token 刷新/relay 凭据）抽库 | 未开始（当前 render_relay_env 由 host 注入） | — |
| D0.4b | Publish 状态机抽库 | 未开始 | — |
| D0.4c | 深链验证、updater/单实例平台 adapter 边界（D5 明确） | 未开始 | — |

## D1 GPUI 真聊天（GD1：Windows 真机）

| # | 能力 | 状态 | 证据 |
|---|---|---|---|
| D1.1 | gpui-kit 依赖锁定 rev / 窗口 / composer / 中文 IME（真实输入无重复字） | 验证(win)：构建通过；原生窗口、composer 及中文 IME 候选/组字经用户确认 | [2026-09-09 D1](../plans/2026-09-09-gpui-chat-task.md)；[09-10 用户窗口验收](2026-09-10-windows-big-object-build.md) |
| D1.2 | 共享 reducer 流式显示、local session 真 API、停止、历史（独立验收 API 127.0.0.1:18082） | 验证(win)：真实回答、流式、停止保留正文、停止后续聊、切换会话恢复均经用户确认 | [共享套件验收](2026-09-09-gpui-tauri-suite-acceptance.md) 31 项通过；[09-10 真实接口及用户窗口证据](2026-09-10-windows-big-object-build.md)。不包含客户端重启恢复或服务端模型取消的验证 |
| D1.3 | Markdown 阅读、历史标题及选中状态；中文加粗、列表起始编号、正文选择/复制 | 验证(win)：主体验及三项 Markdown 余项复验均经用户确认 | [阅读与标题验收](2026-09-10-gpui-markdown-titles.md)；修复提交 09b7466b，40 项自动测试及 Windows 构建通过，用户反馈“已检查，全部正常” |

2026-09-10：D1 Windows 基础聊天、阅读与标题修正验收通过。截图发现的中文括号旁加粗及列表起始编号余项已修正，40 项自动测试和 Windows UI 编译检查、构建通过；用户确认加粗、编号和正文选择/复制全部正常，本批余项关闭（[本批证据](2026-09-10-gpui-markdown-titles.md)）。完整视觉与客户端对等未验收；D2–D6 状态不变，Tauri 保留。

## D2–D5 桌面对等

| # | 能力 | 状态 |
|---|---|---|
| D2 | 栈状态、启动迁移、进程/日志、退出收摊 UI（S0–S6，不依赖预装 Docker） | 验证(win)：共享/无头 60 项通过；真实 PG/Redis 4 步及迁移/API/worker/本机会话 8 步通过，S2/S6 启动失败恢复、超时中退出、进程归属和清理自动门关闭。无需逐项人工确认；GPU 像素、系统文件管理器与 macOS/Linux 待验。[接线证据](2026-09-10-gpui-services.md) · [无头 UI](2026-09-10-gpui-headless-acceptance.md) · [受管进程 E2E](2026-09-10-gpui-managed-acceptance.md) |
| D3 | Workspace、文件、入库、引用、会话与笔记 | 部分验证(win)：工作区总览/创建、作用域会话、文件上传/状态、引用原文、笔记及草稿保护已实现。52 项共享核心/聊天/HTTP、17 项无头 UI（含 8 项工作区旅程）及独立 Windows 生产 UI 构建通过；真实解析/嵌入/RAG、其余知识能力和 macOS/Linux 待验，不标记全量完成。[D3 任务](../plans/2026-09-10-gpui-workspace-task.md) · [自动验收](2026-09-10-gpui-workspace-acceptance.md) |
| D4 | BYOK、角色、云登录、钱包、Publish | 未开始 |
| D5 | updater、深链、单实例、浏览器打开、MCP/CLI | 未开始 |

## D6 三平台验收

| # | 能力 | 状态 |
|---|---|---|
| D6 | 同一 crate 的 Windows/macOS/Linux 构建、安装/升级/卸载、发布产物 | 未开始 |
