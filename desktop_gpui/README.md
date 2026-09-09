# desktop_gpui

Context-OS 原生 GPUI 客户端，独立 Cargo 工程。共享 `desktop-core` 的本机会话和 HTTP/SSE 传输，以及 `contracts` / `web-sdk` 的会话协议、解码与 reducer；不引入第二套聊天接口。

当前切片：本地连接、个人会话列表和历史、逐段正文、多行输入、停止。个人聊天 capabilities 为空；附件、工作区和云登录尚未接入。现网 Tauri 仍继续发货，不能据此宣布 GPUI 全量对等。

## Windows 开发

从源工作区执行 `pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/sync-windows.ps1`，只复制 GPUI 与必要共享源码到 `C:\dev\context-osv6`，不删除目标缓存。执行策略仅对该进程生效。

在 `C:\dev\context-osv6\desktop_gpui` 使用 MSVC：

```powershell
$env:CARGO_BUILD_JOBS = '2'
$env:CARGO_NET_GIT_FETCH_WITH_CLI = 'true'
cargo test --lib
cargo build --features ui
.\target\debug\desktop-gpui.exe
```

Windows 原生链路开发验收使用 `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/run-windows.ps1`（在 desktop_gpui 下运行）。脚本显式选择已安装的 Windows runtime 和状态目录，检查 API/worker/migrate/PG 文件；直接运行开发 exe 可能选择开发树中残留的 Linux 数据目录。`-RuntimeHome` 可指定完整 Windows 安装目录；不复制、覆盖或转换现有数据库。旧库若以 avrag 为 bootstrap 角色，当前角色初始化不支持原地降权，必须先完成独立数据迁移，不能据此宣布聊天链路通过。

每批运行前仍遵守根 AGENTS.md 的耗时确认规则。默认连接共享宿主配置的本机 API（18080），点击“连接本机服务”复用既有本地会话逻辑及 `com.contextos.desktop` 数据目录。它不是云登录，不默认连接 Web 验收 API 18091。不要把生产/云 API 配成开发用本机地址。

依赖锁定 `longbridge/gpui-kit` rev `d2304b9063b902fc7ac19b97a7cb5bf3e650b4f5`，使用该提交锁文件的 `gpui-pre 0.3.2`；gpui/component/platform 通过 kit facade 共用来源。升级需一起更新并验证锁文件。输入使用 `TextareaState`，不自写 IME。

本批范围与验证记录见 [D1 任务](../docs/plans/2026-09-09-gpui-chat-task.md)。Windows 输入/真实本机服务及 macOS/Linux 通过情况以该记录为准，编译不代表人工验收。

## 复用 Tauri 套件

同步后执行 `pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/accept-tauri-shared.ps1 -WithHttpFixture`。原 shared-core lib 和流测试直接复用，另有 GPUI Host 适配测试；HTTP 用例仅使用测试进程内的 loopback 夹具。完整范围见 [验收报告](../docs/desktop/2026-09-09-gpui-tauri-suite-acceptance.md)。
