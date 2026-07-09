# Desktop Client - 开发指南

## 项目概述

这是 AVRag 的桌面客户端，基于 Tauri 2 构建，复用现有 `frontend_next` 静态资源 + `avrag-rs` Rust 核心。

## 架构设计

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

## 关键设计决策

1. **传输接缝**：前端通过 `lib/runtime/transport.ts` 自动检测环境，Web 走 SSE，桌面走 IPC
2. **静态导出**：`BUILD_TARGET=desktop` 时切换为 `output: 'export'`
3. **客户端守卫**：`lib/runtime/client-guard.tsx` 替代 middleware.ts
4. **客户端 i18n**：`lib/runtime/client-i18n.tsx` 替代服务端 getRequestConfig
5. **Cargo workspace（M5 结论）**：`desktop/src-tauri` **不能**作为成员并入 `avrag-rs/Cargo.toml`（cargo 要求成员目录必须在 workspace 根之下）。采用**独立 workspace + 独立 `Cargo.lock`**，path 依赖 `../../avrag-rs/crates/*` 与 `../../contracts`。锁漂移由根 CI `desktop-check` job 兜底（M9）。`cargo update` 后若 `time 0.3.48` 导致 `cookie`/`tauri-utils` 编译冲突，需 pin 回 `time 0.3.47`。
6. **轻量依赖链（M5）**：desktop 仅依赖 `common`、`contracts`、`storage-local`；auth 类型用 `contracts::auth_runtime`。`CachePort` 经 `avrag-rag-core-ports` 暴露，**不**直接依赖 `avrag-rag-core`（避免拉入 llm/redis/code-interpreter）。

## 开发流程

### 开发模式

```bash
# 安装依赖
cd desktop && pnpm install

# 启动开发服务器（联动 Next.js dev server）
pnpm tauri dev
```

### 构建发布

```bash
# 构建前端静态资源
cd frontend_next && pnpm build:desktop

# 构建桌面应用
cd desktop && pnpm tauri build
```

### Workspace 说明（M5）

`desktop/src-tauri` **不能**并入 `avrag-rs` workspace（Cargo 要求 member 必须在 workspace 根目录之下）。当前方案：

- desktop 保持独立 `Cargo.toml` / `Cargo.lock`
- 依赖 avrag-rs crates 时使用相对 path（`../../avrag-rs/crates/*`）
- 修改 avrag-rs 侧 crate 后，在 `desktop/src-tauri` 执行 `cargo update -p <crate>` 同步锁
- CI 由根 `.github/workflows/` 的 `desktop-check` job 兜底（M9）

### 指定平台构建

```bash
# 使用构建脚本
./scripts/build-desktop.sh macos
./scripts/build-desktop.sh windows
./scripts/build-desktop.sh linux
```

## 目录结构

```
desktop/
├── src-tauri/          # Tauri Rust 代码
│   ├── Cargo.toml      # Rust 依赖配置
│   ├── tauri.conf.json # Tauri 配置
│   ├── icons/          # 应用图标
│   └── src/
│       ├── main.rs     # 主进程入口
│       └── lib.rs      # Tauri command 定义
├── package.json        # Node.js 依赖
└── AGENTS.md           # 本文件
```

## 阶段路线图

### 阶段 0：可行性验证 ✅

- [x] 创建 Tauri 2 桌面壳项目结构
- [x] 修改前端配置支持静态导出
- [x] 创建传输适配层
- [x] 创建客户端守卫和 i18n Provider
- [ ] 验证 WebView 兼容性（待测试）

### 阶段 1：静态导出打通

- [ ] 动态路由改造（方案 A）
- [ ] next-intl 客户端模式完善
- [ ] middleware 跳转改为客户端守卫
- [ ] 验证主链路可用

### 阶段 2：IPC 传输接缝

- [ ] 实现 Tauri command（chat_stream, api_call）
- [ ] 前端传输适配层切换到 IPC
- [ ] 验证关闭 localhost HTTP 后 UI 正常

### 阶段 3：本地数据栈

- [ ] MinIO → 本地文件系统
- [ ] Redis → 进程内缓存
- [ ] PostgreSQL / Milvus（按需）

### 阶段 4：打包与系统集成

- [ ] sidecar 打包
- [ ] 安全存储 token
- [ ] 自动更新
- [ ] 安装包生成

## 注意事项

1. **不要依赖 SSR**：桌面端是静态导出，不支持服务端渲染
2. **不要依赖 middleware**：使用客户端守卫 `ClientAuthGuard`
3. **不要依赖 cookies()**：使用客户端 i18n `ClientI18nProvider`
4. **传输分叉在 `lib/runtime/transport.ts`**：UI 组件不需要感知环境差异
5. **阶段 0 先走 localhost HTTP**：IPC 实现在阶段 2 完成

## 相关文件

- 设计文档：`docs/desktop-client-design-2026-06-11.md`
- 传输适配层：`frontend_next/lib/runtime/transport.ts`
- 客户端守卫：`frontend_next/lib/runtime/client-guard.tsx`
- 客户端 i18n：`frontend_next/lib/runtime/client-i18n.tsx`
- Next.js 配置：`frontend_next/next.config.ts`
