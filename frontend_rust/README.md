# frontend_rust

`frontend_rust/` 是 `context-osv6` 当前正式前端实现。

它负责：
- 对接 `avrag-rs` Rust backend
- 聊天 SSE
- citation lookup
- 正文内联 citation 渲染
- 多模态图片块内联渲染

## 本地检查

```bash
cargo check -p web-ui
```

## 说明

- `../frontend/` 仍保留为 legacy/reference Next.js 前端。
- 新的前端能力默认优先落在这里。
