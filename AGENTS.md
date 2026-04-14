# AGENTS.md — 全栈 Rust 项目 AI 开发规范
# 本文件由 Codex 每次任务开始前必须完整阅读

---

## 🧭 项目环境事实（防命令漂移）

- 本项目运行环境以 **WSL Linux** 为主，Windows 侧通过映射盘访问同一份文件。
- 同一项目目录的双路径映射：
  - Windows 路径：`Z:\home\chuan\context-osv6`
  - WSL 路径：`/home/chuan/context-osv6`
- 当前前端工程位于：`/home/chuan/context-osv6/frontend_rust`（Windows 映射：`Z:\home\chuan\context-osv6\frontend_rust`）。
- `frontend_rust/Cargo.toml` 是 Rust workspace（members: `crates/web-sdk`, `crates/web-ui`）。
- 工程技术栈以 Rust 为主：Leptos（前端/SSR）、Axum + Tokio + Tower（服务端与中间件）。

## 🛂 命令执行门控（开始任务先对齐）

- 涉及 `cargo`/`rustup`/`rustfmt`/`clippy` 等 Rust 命令时，默认在 WSL 路径执行，不在 Windows 路径直接拼 Linux 命令。
- 从 PowerShell 进入 WSL 执行命令时，必须显式 `cd` 到 WSL 路径后再执行，避免路径翻译失败导致漂移。
- 禁止在同一条命令中混用 Windows 路径（`Z:\...`）和 Linux 路径（`/home/...`）。
- 推荐执行模板（PowerShell -> WSL）：
  - `wsl.exe -e bash -lc "cd /home/chuan/context-osv6 && <command>"`
  - `wsl.exe -e bash -lc "cd /home/chuan/context-osv6/frontend_rust && cargo test"`

---

## 📚 参考文档（必须优先查阅，禁止凭记忆生成代码）

### Rust 核心
- Rust 官方 Book：https://doc.rust-lang.org/book/
- Rust by Example：https://doc.rust-lang.org/rust-by-example/
- Rust 异步编程：https://rust-lang.github.io/async-book/
- Rust API 文档搜索：https://docs.rs

### 后端
- Axum 官方文档：https://docs.rs/axum/latest/axum/
- Tokio 文档：https://docs.rs/tokio/latest/tokio/
- Tower 中间件：https://docs.rs/tower/latest/tower/
- graph-flow 官方文档：https://docs.rs/graph-flow/0.5.0/graph_flow/（注意检查版本号）
- graph-flow crates.io：https://crates.io/crates/graph-flow
- 官方示例仓库：https://github.com/a-agmon/rs-graph-llm
- Rig（LLM 集成）：https://docs.rs/rig-core

### 前端
- Leptos 官方 Book：https://book.leptos.dev
- Leptos API 文档：https://docs.rs/leptos/latest/leptos/
- leptos-shadcn-ui 组件：https://github.com/cloud-shuttle/leptos-shadcn-ui
- leptos_sse 文档：https://docs.rs/leptos_sse
- leptos-use 工具库：https://leptos-use.rs
- Stylance CSS Modules：https://docs.rs/stylance

### 样式
- Tailwind CSS 文档：https://tailwindcss.com/docs（仅用于小型原型）
- CSS 变量规范：参考本仓库 src/styles/tokens.css
