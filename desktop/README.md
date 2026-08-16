# Desktop Client - Tauri 2 桌面壳

基于 Tauri 2 的桌面客户端，复用 `frontend_next` 静态资源 + `avrag-rs` Rust 核心。

## 架构

```
┌──────────────────────────────────────────────┐
│  Next.js 静态资源 (frontend_next → out/)        │  只做展示与交互
└───────────────┬──────────────────────────────┘
                │  Tauri WebView 加载 frontendDist=out/
┌───────────────▼──────────────────────────────┐
│  Tauri 2 桌面壳 (desktop/)                      │  窗口 / 权限 / 系统集成 / 安全存储
└───────────────┬──────────────────────────────┘
                │  进程内调用 or sidecar IPC
┌───────────────▼──────────────────────────────┐
│  本地 Rust 核心 (复用 avrag-rs/crates/*)         │  配置 / 任务编排 / 检索 / 流式处理
└──────────────────────────────────────────────┘
```

## Coding Agent（MCP / CLI）

本机 `avrag-api` 暴露与云端同源的 **HTTP MCP**（`POST /api/v1/mcp`）。  
**stdio 包装：** `context-os-mcp` — 转发到本机网关；`context-os-mcp --check` 探活。  
**薄 CLI：** `context-os status|ingest|ask|sources`（同鉴权；`share` 故意拒绝，走 UI）。

```bash
# 构建（两个 bin：context-os-mcp + context-os）
cd avrag-rs && cargo build -p context-os --release

# Stage 到 desktop/runtime/bin/（与 api/worker 一起）
bash scripts/stage-desktop-sidecars.sh   # STAGE_BUILD=1 可自动编译
```

Claude Code / Codex 配置片段、能力矩阵与 P1 缺口见：

- 仓库：[`docs/desktop/LOCAL-CLIENT-MCP-CLI-AGENT-ACCESS.md`](../docs/desktop/LOCAL-CLIENT-MCP-CLI-AGENT-ACCESS.md)
- Wire 契约：[`frontend_next/public/docs/api-access-for-agents.md`](../frontend_next/public/docs/api-access-for-agents.md)

## 开发

### 推荐：Windows 本机调试（不必反复打 NSIS）

| 场景 | 做法 | 耗时量级 |
|------|------|----------|
| **日常改壳 / 启动栈 / 退出收摊** | WSL：`bash scripts/dev-windows-hotswap.sh shell-only` → 覆盖已安装目录的 `Context-OS.exe` 并启动 | 数分钟（只编 desktop） |
| **改 API/Worker sidecar** | `SKIP_FRONTEND=1 bash scripts/dev-windows-hotswap.sh hotswap` | 视 cargo 增量 |
| **改前端 UI** | **Windows 上**装 Node + Rust 后：`cd desktop && pnpm tauri dev`（`devUrl`→`localhost:3000` 热更） | 秒级 HMR |
| **发版安装包** | 才跑 `bash scripts/build-windows.sh` + `package-desktop-release.sh` | 数十分钟 |

**一次安装 + 多次热替换（当前 WSL 交叉编主路径）：**

```bash
# 1) 只装一次 setup.exe（带齐 runtime/pgsql、redis）
# 2) 改 desktop/src-tauri 后：
bash scripts/dev-windows-hotswap.sh shell-only
# 等价于：编 Context-OS.exe → 拷到 %LOCALAPPDATA%\Context-OS Client\ → 启动

# 同时更新 avrag-api/worker + MinGW DLL：
SKIP_FRONTEND=1 SKIP_SIDECARS=0 bash scripts/dev-windows-hotswap.sh hotswap
```

**纯 Windows 原生 dev（最舒服的 UI 调试）：**

```powershell
# 一次性工具链（管理员 PowerShell 可选）:
#   winget install OpenJS.NodeJS.LTS Rustlang.Rustup Microsoft.VisualStudio.2022.BuildTools
#   npm.cmd install -g pnpm
#   # Build Tools 勾选 "使用 C++ 的桌面开发" / VCTools

# 从 WSL 同步源码到盘符（勿在 UNC \\wsl$\ 上跑 npm/cargo）:
#   wsl -d Ubuntu -- bash /home/chuan/context-osv6/scripts/sync-windows-dev.sh

cd C:\dev\context-osv6\desktop
pnpm install
cd ..\frontend_next
pnpm install
cd ..\desktop
pnpm tauri dev
```

或一键准备 PATH（不自动起 dev）：

```powershell
powershell -ExecutionPolicy Bypass -File \\wsl$\Ubuntu\home\chuan\context-osv6\scripts\dev-windows-env.ps1
```

`tauri dev` 走 `beforeDevCommand` 起 Next dev server，**不打 NSIS**；本机数据面仍由壳 IPC `ensure_local_stack` 拉起（先装过一次客户端即可复用 `%LOCALAPPDATA%\Context-OS Client` 的 PG/Redis）。

**Windows 注意：**

1. **不要**在 `\\wsl$\...` UNC 下跑 npm/cargo；用 `C:\dev\context-osv6`（`sync-windows-dev.sh`）。
2. **Smart App Control** 若开启，会拦截 `cargo` 编译出的 `build-script-build`（`os error 4551`）。请关闭：  
   设置 → 隐私和安全性 → Windows 安全中心 → 应用和浏览器控制 → **智能应用控制 → 关闭**（可能要重启）。  
   关闭前请用 WSL **热替换**调试壳层。
3. `desktop` 在 Windows 上若 `pnpm` 装依赖报 reparse/UNKNOWN，可改用 `npm install`（已验证可用）。

### 旧入口

```bash
# 安装依赖
pnpm install

# 开发模式（联动 Next.js dev server）— 优先在 Windows 主机跑
pnpm tauri dev

# 构建桌面应用 / 发版
pnpm tauri build
# 或仓库标准交叉编：
bash scripts/build-windows.sh
```

## 目录结构

```
desktop/
├── src-tauri/          # Tauri Rust 代码
│   ├── Cargo.toml      # Rust 依赖配置
│   ├── tauri.conf.json # Tauri 配置
│   └── src/
│       └── main.rs     # 主进程入口
├── package.json        # Node.js 依赖
└── README.md
```
