# GPUI 复用 Tauri 测试套件验收

2026-09-09。用户明确要求利用 Tauri 客户端测试套件验收，批准 15–25 分钟宿主/API 定向验证；不启动或重启现有 Tauri、不调用付费模型、不部署。

## 结果及可复用边界

| 原套件 / 场景 | GPUI 使用方式 | 本批结果 |
|---|---|---|
| `desktop/core` lib | 直接执行原库测试；涵盖本地会话、路径、栈配置、上传安全与响应处理等现有断言 | 19 通过；其中运行时探测和配置单测不是冷启动实测 |
| `desktop/core/tests/chat_stream_tests.rs` | GPUI Cargo test target 直接引用原文件，不复制断言；在 GPUI 锁定依赖图上运行 | 4 通过：中文 UTF-8 跨块、CRLF、坏帧、提前取消 |
| GPUI 状态与 Host 适配 | 原 4 条 + 新 4 条；共用原流夹具和实际共享宿主 HTTP/SSE 实现 | 8 通过 |
| Tauri L0 `-AuditOnly` | 直接执行原脚本，检查既有安装树和端口归属 | `ok=true`；5433/6380/18080 均空闲，未启动窗口、未验证服务健康/会话/退出 |
| Tauri L1/L2/L3 WebView | 现有 `connectTauriPage` 绑定 CDP + tauri.localhost + DOM | 不适用于 GPUI，不伪造通过 |
| 升级、冷启动、资料与发布 | 会操作 Tauri 安装或属于 GPUI 后续能力 | 本批未执行 |

共 **31 个不同测试通过**（19 + 4 + 8）。默认测试批次显示 1 ignored，为独立 HTTP 用例；随后显式启用已通过，不把它重复计数。安装树只读审计不计入测试数。

## 新增证据

- 共享长文夹具通过真实 loopback HTTP/SSE → desktop-core → Host channel → 共用 reducer，在服务器尚未允许继续输出时已有正文；终态与原夹具一致，个人 capabilities 为空，会话归属保留。
- 上游返回配置错误时传到 Host，不无限等待。该测试模拟上游响应，不等于真实无密钥产品回归。
- 本机 API 不可达时传到 Host 错误；不是旧 Tauri 测试中修改 provider secret 后的真实 provider 故障场景。
- 受控 HTTP 服务验证 `Host.login` 建立并保存本地会话、个人列表过滤 workspace、历史完整读取、再次登录恢复。数据仅写随机临时目录，成功后删除本次生成的两个文件和空目录。
- 既有取消测试验证响应头尚未到达时连接可关闭；状态测试验证停止保留部分正文以及旧代次隔离。

首轮新流夹具封装遗漏 SSE 的 `event:` 字段，被共享解码器正确拒绝；修正测试 framing 后通过，未放宽产品解码器。

包含外部测试服务启动/停止的批处理曾被自动审批拒绝，返回 `blocked by policy`。随后将 HTTP 夹具放入 Rust 测试进程，单一 loopback 服务随用例结束，不启动外部应用。此方案已执行通过；没有借用测试脚本绕过此前 GPUI 原生应用启动限制。

## 复现

从源仓库同步后运行（Windows MSVC，jobs=2）：

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/sync-windows.ps1
pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/accept-tauri-shared.ps1 -WithHttpFixture
```

HTTP 用例独占 18180；端口占用会失败，不停止占用进程。脚本不启动/停止客户端，不修改现网 provider secrets、AppData 或安装树。运行前仍遵守 AGENTS.md 耗时确认。

日志归档：`desktop_gpui/target/acceptance/tauri-shared/`；含共享宿主、原流测试、GPUI/HTTP 日志及 `tauri-l0-audit.json`。代码关系图已更新。

## 尚未验收

GPUI 原生窗口、中文 IME、点击/焦点/滚动、真实本机 API 与模型、安装升级和三平台仍待验。Tauri 的 Playwright 不能直接验证 GPUI 原生视图；需要原生驱动和相同业务场景，不能以本批 31 项替代 D1/M1 真机验收。
