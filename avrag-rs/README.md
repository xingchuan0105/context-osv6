# avrag-rs

`avrag-rs` 是 `context-osv6` 的 Rust 工作区

正式前端：

* `../frontend\_next/` 是当前正式前端实现，负责承接 Rust API、SSE、citation lookup、正文内联 citation 与图片块渲染。
* `../frontend\_rust/` 是历史 Rust 前端工程；`avrag-api` 不再提供 Leptos SSR fallback。


文档：

* `docs/README.md` 是文档索引（现行权威与已取代文档的对照）。
* `docs/runbooks/local-dev.md` 是本地开发 runbook。
