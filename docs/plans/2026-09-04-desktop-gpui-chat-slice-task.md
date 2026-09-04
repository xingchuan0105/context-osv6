# 桌面 GPUI 第一刀：夹具 → 共用 reducer → 窗口

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-04 |
| 状态 | **第一刀代码已通**（夹具测试绿；`cargo check --features ui` 绿）。窗口/IME 未在本机点验 |
| 权威 | [ADR-0011](../adr/0011-rust-web-gpui-desktop.md) |
| 下一棒入口 | [`2026-09-04-rust-web-gpui-desktop-handoff.md`](2026-09-04-rust-web-gpui-desktop-handoff.md) |
| 前置 | Gate 0 已改为观察（charter 19:40），不挡本切片 |

## 做

- `reduce_chat_event` 下沉 `web-sdk`；`web-ui` 只再导出。
- 新独立工程 `desktop_gpui/`：夹具收束到 `ChatTurnState`，GPUI 窗口画用户气泡。
- Windows IME / 真流 / 登录：下一刀。

## 不做

- 不改生产 `tauri.conf.json`、不删 `frontend_next`、不部署。
- 不并 workspace，不写第二套 `ChatEvent`。
- 不往 `TauriIpcTransport` 加功能。
