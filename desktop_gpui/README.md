# desktop_gpui

全平台 GPUI 桌面 UI。独立 Cargo 工程，不并入 `avrag-rs`。协议与状态只走 `contracts` + `web-sdk` reducer。权威：[ADR-0011](../docs/adr/0011-rust-web-gpui-desktop.md)。下一棒：[交接](../docs/plans/2026-09-04-rust-web-gpui-desktop-handoff.md)。

现网发货仍是 `desktop/` Tauri + Next。本目录第一刀：夹具 → 共用 reducer → 窗口里画出用户气泡。不接登录、许可、本地栈、更新。

```bash
# 无窗口（约 15s）
cargo test --offline 2>/dev/null || cargo test

# 打开夹具预览窗（首次拉 gpui 可能 5–15 分钟）
cargo run --features ui
```
