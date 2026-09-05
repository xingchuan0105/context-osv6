# D0.1 任务记录：桌面流解析修复与最小宿主库抽离 (`desktop/core`)

日期：2026-09-05
负责人：Agent / Solo Trunk
关联计划：[2026-09-05-development-execution-plan.md](2026-09-05-development-execution-plan.md) §6 (D0.1)
门禁目标：D0.1 切片完成

---

## 1. 任务背景与核心问题诊断

### 核心缺陷
在原有的 `desktop/src-tauri/src/commands/chat_stream.rs` 中：
- 网络分块读取代码为：`match resp.chunk().await { Ok(Some(chunk)) => buf.push_str(&String::from_utf8_lossy(&chunk)) }`；
- 当多字节中文 UTF-8 字符（3 字节）被 TCP 分包切断时（如前 2 字节在第 1 包，后 1 字节在第 2 包），`from_utf8_lossy` 会立即将前 2 字节判定为无效字节并替换为 `U+FFFD`（乱码），导致桌面中文流严重损坏。
- 同时存在历史废弃的许可门检查（`license_allows_chat` 恒为 true，ADR-0010 已确认免费客户端不再做激活拦截）。

### 解决方案
1. 创建平台与 UI 中立的最小宿主库 `desktop/core` (`desktop-core`)；
2. 复用 `web-sdk::SseDecoder`，利用其 `take_valid_utf8` 缓冲机制，实现无损的跨 chunk UTF-8 中文字符重组；
3. Tauri 桌面端调用 `desktop-core`，彻底删除原有手写且存在缺陷的解析代码与废弃的许可门；
4. 编写跨包 UTF-8、CRLF、坏帧、取消与单一终态的单元测试。

---

## 2. 执行步骤

| 次序 | 工作项 | 状态 | 产物与证据 |
|---|---|---|---|
| D0.1.1 | 建立 `desktop/core` crate 骨架与依赖配置 | 已完成 | `desktop/core/Cargo.toml`, `src/lib.rs` |
| D0.1.2 | 实现中立的 `stream_chat_events`，复用 `web-sdk::SseDecoder` | 已完成 | `desktop/core/src/chat_stream.rs` |
| D0.1.3 | 编写跨 chunk 中文、换行、坏帧与取消单元测试 | 已完成 | `desktop/core/tests/chat_stream_tests.rs` 4/4 passed |
| D0.1.4 | Tauri 接入 `desktop-core`，清理废弃许可门与手写解析代码 | 已完成 | `desktop/src-tauri/src/commands/chat_stream.rs` 简化重构 |
| D0.1.5 | 验证收敛、图谱更新与本地提交 | 已完成 | 图谱已更新 (12 files)，本地提交 `b3ce4ad7` (D0.1 达成) |
