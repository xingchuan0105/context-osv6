# frontend_rust

`frontend_rust/` 是 `context-osv6` 当前正式前端实现。

它负责：
- 对接 `avrag-rs` Rust backend
- 聊天 SSE
- citation lookup
- 正文内联 citation 渲染
- 多模态图片块内联渲染
- 前端正式样式架构：`design_tokens.css` + Plain CSS + Stylance CSS Modules

## 本地检查

```bash
cargo check -p web-ui
```

## 开发模式

高频 UI 开发默认使用 watch 模式，不再手工重建 `pkg/`：

```bash
cd /home/chuan/context-osv6/frontend_rust
cargo install cargo-leptos --locked   # 首次执行
bash scripts/dev-ui.sh
```

默认开发地址：`http://127.0.0.1:3000`

详细说明见 [docs/frontend_dev_loop.md](/home/chuan/context-osv6/frontend_rust/docs/frontend_dev_loop.md)。

## 说明

- `../frontend/` 仍保留为 legacy/reference Next.js 前端。
- 新的前端能力默认优先落在这里。
