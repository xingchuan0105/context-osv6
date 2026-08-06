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

```bash
# 安装依赖
pnpm install

# 开发模式（联动 Next.js dev server）
pnpm tauri dev

# 构建桌面应用
pnpm tauri build
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
