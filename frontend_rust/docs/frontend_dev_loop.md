# Frontend Dev Loop

## 目标

把 `frontend_rust` 的日常开发模式从“手工构建 `pkg/` + 静态服务”切换成“watch + 自动刷新 + API 透传”，并把样式主链切换到 `Plain CSS + Stylance CSS Modules`。

## 当前开发入口

在 WSL 下执行：

```bash
cd /home/chuan/context-osv6/frontend_rust
cargo install cargo-leptos --locked   # 仅首次需要
cargo install stylance-cli --locked   # 仅首次需要
bash scripts/dev-ui.sh
```

开发服务器默认地址：`http://127.0.0.1:3000`

## 后端依赖

`frontend_rust` 的 dev server 不直接承载业务 API，只代理 `/api/*` 到后端。

默认后端地址：`http://127.0.0.1:8080`

先启动后端：

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo run -p avrag-api
```

如果后端地址不是默认值，启动前端时覆盖：

```bash
AVRAG_BACKEND_URL=http://127.0.0.1:9000 bash scripts/dev-ui.sh
```

## 自动化链路

`scripts/dev-ui.sh` 会并行启动四条开发链路：

1. `cargo leptos watch`
   - 监听 Rust 代码
   - 自动重编前端 WASM
   - 自动刷新浏览器

2. `scripts/watch_design_tokens.py`
   - 监听 `crates/web-ui/src/styles/design_tokens.css`
   - 变更后自动执行 `scripts/sync_design_tokens.py`

3. `stylance`
   - 监听 `.module.css` 文件
   - 输出开发态样式到 `target/site/pkg/stylance.css`
   - CSS 变更无需触发 Rust 重新编译

4. `tailwindcss --watch`
   - 仅服务于尚未迁移的 legacy 页面
   - 输出兼容层样式到 `target/site/pkg/index.css`

## 样式分层约束

- 新页面和核心壳层：使用 Stylance CSS Modules
- `index.css`：仅保留 token、reset、共享基础样式、legacy 兼容层
- Tailwind：禁止在新代码中继续扩散

## 保留的验收链路

视觉验收和截图基线仍走当前静态产物链路：

- `frontend_rust/pkg`
- `http://127.0.0.1:4173/preview/*`

这条链路不用于高频开发，只用于稳定快照和回归测试。
