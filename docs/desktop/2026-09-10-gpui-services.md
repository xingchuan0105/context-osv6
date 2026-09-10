# GPUI D2 本机服务接线与验收

## 本批结果

已实现服务面板、分阶段连接和按本次进程记录退出清理。45 项单测/隔离 HTTP 测试、1 项真实 API 只读探测及 Windows 构建通过。新版已交给用户操作，面板原生交互与受管环境 S2/S6 真机门仍待验，D2 不标记整体完成。

用户授权本批 25–40 分钟修改后验证，并确认关闭旧 GPUI 窗口。没有使用 Computer Use、调用付费模型、停止既有后端或部署。

## 用户路径与实现

- 聊天页右上角“本机服务”打开面板，返回聊天保留输入实体、历史与草稿。此入口已记入 PRODUCT_IA 的 J7，不占用工作区主导航。
- 面板展示实际 API 地址、健康状态和本机会话。受管环境另显示 PG/Redis 端口、worker 进程、原生工具、配置及日志目录；端口可达不等于 pgvector 扩展或迁移已验证。
- 启动按数据栈、产品迁移/API/worker、本机会话推进，显示阶段、错误和重试入口。原生路径无需 Docker；API 已运行而 worker 缺失时，会尝试补齐 worker。
- 显式配置 API 的验收模式仅连接现有服务，不启动数据栈。面板说明其数据库与后台任务由启动环境管理，并提供运行目录入口；断开会话和退出保留外部服务。
- 受管启动前后记录 PID 文件和可执行文件身份，只清理本次新增且仍匹配的进程。产品先停、数据服务后停；产品停止失败则保留后续组件供重试。PostgreSQL 使用 `pg_ctl -m fast`，不按安装目录扫杀进程。
- 关闭窗口先取消聊天，再等待串行服务操作和清理结果；失败保持窗口，成功才退出。Host 销毁另执行同一清理作为退出兜底。

`desktop_gpui/src/services.rs` 负责宿主编排，`service_view.rs` 负责面板，`runtime.rs`/`main.rs` 负责事件和窗口接线。共享 `desktop/core` 拆出仅认证的 `connect_local_session`，加入 `runtime_lease.rs` 的进程归属清理，并修正 `local_product.rs` 的实际 URL 端口、HTTP/JSON 健康和 worker 就绪判断。旧 Tauri 的整体退出路径未替换，业务 API、后端和 Tauri UI 未修改。

## 验证证据

| 层级 | 结果 |
|---|---|
| 共享 desktop-core lib | 24 通过；新增实际 URL 端口、健康响应、已有进程不被认领、身份变化不停止、产品停止失败后保留数据服务 5 项 |
| GPUI lib | 16 通过；聊天、取消、代次、Markdown 和标题回归 |
| 原 Tauri 共享流测试 | 4 通过 |
| 隔离 HTTP 旅程 | 1 通过；保留登录/历史/标题/工作区隔离断言，增加只连接独立 API、断开后 API 可用、健康失败不触发本机启动 |
| 真实 API 只读探测 | 1 通过；实际 127.0.0.1:18082 健康，连接模式且无受管进程。未建立测试会话、写数据或调用模型 |
| Windows check / build | 通过，jobs=2；最终 check 2.06 秒、build 9.06 秒。已有 ts-rs 和 desktop-core 未使用函数警告保留 |
| 原生用户交互 | 待用户操作，不以构建和进程启动替代 |

日志在 `C:\dev\context-osv6\desktop_gpui\target\acceptance\tauri-shared\`：`shared-core.log`、`gpui-and-original-stream.log`、`local-session-history.log`、`d2-live-probe.log`、`d2-ui-check.log`、`d2-ui-build.log`。

程序为 `C:\dev\context-osv6\desktop_gpui\target\debug\desktop-gpui.exe`，SHA-256 `A915C6CA58202F34616F8E2B020F9C606ACE590D91522E6E4AB01094926F254E`。独立启动脚本打开 PID 46220，API 18082 仍为 PID 37708。代码关系图随本批更新，仅提交本任务文件。

## 下一验收门

后续更新：用户选择无需 Computer Use、无需逐项人工点验。下列第 1 项的面板交互/草稿、第 2 项的断开重连及 GPUI 关闭回调已由 [无头 UI 套件](2026-09-10-gpui-headless-acceptance.md) 自动验证；操作系统文件管理器/显卡像素不在该套件内。第 3 项仍为独立进程 E2E 待验。本节保留初始验收安排，当前状态以无头验收记录为准。

1. 用户查看面板、刷新状态、打开运行目录；输入草稿后往返聊天与面板，确认草稿保留。
2. 独立连接模式下断开再连接，确认历史恢复；关闭窗口后 API 保留。窗口仍由用户操作。
3. 另在具有最新 API/worker/migrate 和便携数据栈的受管隔离环境，实测 S2 启动/迁移、失败恢复及 S6 进程清理，覆盖启动中退出与启动超时。当前测试不替代这些实际进程旅程；macOS/Linux 同样未验。

第 3 项通过前保持 D2 待验，不推进 D3 或宣布可移除 Tauri。
