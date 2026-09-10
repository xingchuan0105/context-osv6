# GPUI D2 受管进程自动验收

## 当前状态

Windows D2 的自动验收门通过：共享核心、Tauri 复用及 GPUI 无头 UI 共 60 项通过；真实 PostgreSQL/Redis 数据栈 4 步和迁移/API/worker/本机会话完整旅程 8 步通过。启动失败、真实超时、并发连接归属及退出清理均由程序自动判定，无需逐项人工确认。D2 Windows 门关闭，三平台及完整客户端对等仍按清单待验。

本批已获用户授权 30–50 分钟自动验证。全程不使用 Computer Use、不要求逐项人工确认、不调用模型、不停止或刷新既有服务。

实际 Windows 后端构建耗时 72 分 59 秒，超出估计，主要用于 DuckDB C++ 重编及 MinGW 最终链接；构建退出码为 0。构建期间持续报告进度，未为缩短等待中断服务或改用旧 worker。完整进程验收随后用时 146.52 秒。

## 修正

- 原生启动、生成的 client.env、迁移 DSN、API 地址和状态探测共享 CLIENT_PG_PORT / CLIENT_REDIS_PORT / CLIENT_API_PORT，默认端口保持 5433 / 6380 / 18080。
- 配置面板显示实际运行目录的配置和迁移路径，不再把安装或隔离环境指向开发仓库；数据库连接展示运行角色，凭据继续脱敏。
- 去除超时后丢弃 spawn_blocking 句柄的行为。原生命令在数据栈或产品启动各自的 90 秒期限内运行；超时会停止并回收命令进程，宿主等待启动结束后登记进程，再执行退出清理。
- PostgreSQL 在 spawn 成功时立即写宿主 PID 记录，不等待 postmaster.pid 出现；API/worker/Redis 写入 PID 失败时停止新建进程。退出先停产品、再停止 PG/Redis，仅处理本次启动且身份仍匹配的进程。
- Windows 进程归属增加父进程核对，同一宿主内的连接串行执行；worker 状态核对实际 PID 和可执行文件，空壳 PID 文件与其他进程不再被当成存活的 worker。原生产品启动失败不进入 WSL/Git bash 分支。

## 自动入口

从源仓库执行，先同步 Windows 共享源码：

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/sync-windows.ps1
pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/accept-tauri-shared.ps1 -WithHttpFixture -WithHeadlessUi
pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/accept-managed.ps1 -DataPlaneOnly
pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/accept-managed.ps1
```

最后一项也可通过共享入口的 `-WithManagedProcesses` 调用。它要求当前 Windows API/worker/migrate 构建产物和便携 PG/Redis；端口占用即失败。每次创建 `target/acceptance/managed/gpui-managed-<uuid>`，保存步骤、测试日志、二进制及源码哈希和机器判定结果。保留数据库作为排查证据，但测试结束必须没有进程残留；既有服务/客户端主进程 ID、路径和创建时间必须不变。PostgreSQL 的短生命周期会话子进程不计作服务重启。

完整旅程执行真实迁移失败→产品不启动→部分启动清理→删除故障迁移→迁移及本机会话成功→第二宿主并发连接并退出后不停止已有服务→所有本次进程停止→真实 SQL 睡眠超过启动期限且此时关闭宿主→进程清理→再次启动及身份/密钥恢复。故障 SQL 仅写入该次测试目录，不改仓库迁移。

## 已验证证据

| 层级 | 结果 | 证据 |
|---|---|---|
| 共享核心 | 30 通过，包括子进程超时回收、双输出管道、过期期限不启动、无效 worker PID | `C:\dev\context-osv6\desktop_gpui\target\acceptance\tauri-shared\shared-core.log` |
| GPUI lib / Tauri 流 / 隔离 HTTP | 16 + 4 + 1 通过；其他 opt-in 用例不计入通过数 | 同目录 `gpui-and-original-stream.log`、`local-session-history.log` |
| GPUI 无头 UI | 9 通过，无失败或忽略 | `C:\dev\context-osv6\desktop_gpui\target\acceptance\headless\gpui-ui-632ea20e7a4a45579eda36ee6963e7c3\result.json` |
| 真实数据栈 | 1 条完整测试、4 个步骤通过，最终复验 24.42 秒；无残留，16 个既有进程身份保持 | `C:\dev\context-osv6\desktop_gpui\target\acceptance\managed\gpui-managed-3905ec42d71d4273bbb050a6a4669c5e\result.json` |
| Windows UI 编译检查 | `cargo check --locked --features ui` 通过，8.35 秒；未替换正在运行的客户端 | `C:\dev\gpui-d2-ui-check.log` |
| Windows 后端构建 | API/worker/migrate 通过，jobs=2，72 分 59 秒；保留既有警告 | `C:\dev\gpui-d2-backend-build.log` |
| 产品进程 E2E | 1 条完整测试、8 个步骤通过，测试本体 139.29 秒、脚本全程 146.52 秒；无残留，6 个既有主进程身份保持 | `C:\dev\context-osv6\desktop_gpui\target\acceptance\managed\gpui-managed-0ce6e14de5d348ae85a0d99124dc8466\result.json` |

最终无头结果的 14 个源码哈希、完整进程结果的 11 个源码哈希均与源仓库核对一致。共享核心的 30 项中包含 1 个供子进程测试调用的夹具入口；数据栈与完整进程两条 opt-in 测试单独记录，不计入 60 项。

完整进程结果的 `steps.json` 记录：

1. 全新数据目录初始化成功；注入失败迁移后 API/worker 未启动。
2. 部分启动的 PG/Redis 及 PID 记录全部清理。
3. 删除故障迁移后真实迁移成功，API 健康、worker 实际输出 heartbeat、本机会话建立。
4. 启动期间发起的第二个 Host 连接恢复同一账户；其退出保留首个 Host 的服务。
5. 主动停止清理所有本次产品与数据进程。
6. 迁移执行 `pg_sleep(120)`，启动中销毁 Host；实际到达 90 秒期限后回收迁移及全部受管进程。
7. 删除超时迁移并重新启动，账户 ID 与 JWT/BYOK/upload 三份密钥保持。
8. 销毁 Host 后端口全部关闭，PID 文件清除，脚本另核对无进程残留。

本记录不替代 GPU 像素、系统输入法、macOS/Linux、安装升级或真实模型验收。现有原生窗口和后端保持运行，测试程序使用独立路径；D3 工作区/文件/笔记尚未开始。
