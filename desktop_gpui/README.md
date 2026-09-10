# desktop_gpui

Context-OS 原生 GPUI 客户端，独立 Cargo 工程。共享 `desktop-core` 的本机会话和 HTTP/SSE 传输，以及 `contracts` / `web-sdk` 的会话协议、解码与 reducer；不引入第二套聊天接口。

当前切片：本地连接、个人/工作区会话和历史、逐段正文、多行输入、停止、Markdown 阅读，以及本机服务面板和受管启动/退出。工作区支持总览/创建、文件上传和处理状态、资料范围、引用原文、笔记增删改与草稿保护。个人聊天 capabilities 为空；个人本轮附件、云登录和其余 Tauri 对等能力尚未接入。现网 Tauri 仍继续发货，不能据此宣布 GPUI 全量对等。

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

增加 `-WithHeadlessUi` 可自动验证 GPUI 的真实视图、点击、输入、滚动和布局，使用 TestPlatform，不操作桌面。增加 `-WithManagedProcesses` 则验证真实隔离进程：冷初始化、迁移失败、修复重试、本机会话、其他宿主退出、启动超时及进程清理。

Windows D2 自动门已通过：共享/无头 60 项、真实数据栈 4 步、真实产品进程 8 步。范围与结果路径见 [受管进程验收记录](../docs/desktop/2026-09-10-gpui-managed-acceptance.md)；不代表 GPU 像素、系统集成或三平台安装验收。

Windows D3 首批自动门通过：52 项共享核心/聊天/HTTP 检查与 17 项无头 UI（其中 8 项工作区旅程），生产 `ui` 特性构建通过。详见 [工作区验收记录](../docs/desktop/2026-09-10-gpui-workspace-acceptance.md)；隔离 HTTP 结果不代表真实文档解析/嵌入或模型回答通过。

D3.1 上传提交恢复及笔记 PUT 接口修正后，最新自动门为 73 项共享/无头与 1 条真实 API 旅程（8 步）通过，见 [修复与真实验收记录](../docs/desktop/2026-09-10-gpui-upload-recovery-acceptance.md)。源仓库的 `desktop_gpui/scripts/accept-workspace-live.ps1` 复用已运行的 18082 独立验收 API 和 `session-current`，创建/清理自身测试对象，不启动服务、不上传有效文件、不调用模型；运行前仍需本批耗时授权。

当前程序运行时，可从源仓库执行 `pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/build-acceptance.ps1`。它用实际源码和锁文件生成独立名称的 Windows 验收 exe，复用编译缓存，核对原程序哈希不变，并写入构建结果与源码哈希；不要求关闭窗口，也不会自动启动新程序。脚本运行仍遵守本批耗时授权。

UI 收口已覆盖公共壳、聊天、工作区资料/笔记和服务面板，支持原生模态抽屉与本机明暗/折叠偏好。最新 78 项共享/无头测试通过，另有 24 张原生 DirectX 截图；范围和余项见 [界面验收记录](../docs/desktop/2026-09-10-gpui-ui-finish-acceptance.md)。同步后运行 `pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/render-previews.ps1` 可复现截图：使用独立测试特性程序、一个隐藏窗口和合成数据，自动写入 `target/acceptance/visual/`；不显示/操作桌面、不访问真实模型。普通 `ui` 程序不含该模式。

受管进程验收从源仓库执行 `pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/accept-managed.ps1`，使用已构建的 Windows API/worker/migrate 和便携 PG/Redis；`-BuildDir`、`-PortableRuntime` 可指定构建产物。默认选择 18190/15439/16389，端口已被占用则立即失败。每次生成独立目录，写入 `steps.json`、`tests.log` 和 `result.json`，核对既有进程身份和测试进程残留，不操作桌面或调用模型。`-DataPlaneOnly` 单独验证真实 PG/Redis、角色、扩展、写入及重启持久化，不需要产品 sidecar；这层通过不代表产品迁移/API/worker 已通过。
