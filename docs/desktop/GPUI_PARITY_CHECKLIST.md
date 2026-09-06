# GPUI 桌面对等清单 (GPUI_PARITY_CHECKLIST)

> D0 开工建立（权威计划 §7）。逐项记录 Windows / macOS / Linux 的实现、验证、产物与结果。
> 不照抄旧版号或旧“可选”结论；机器或签名条件不足的项标记 `待验`，不判整体完成。
> D1 只是中途里程碑，不用于宣布可删 Tauri。

状态图例：`未开始` / `抽库完成` / `实现` / `验证(win/mac/linux)` / `产物` / `待验(原因)` / `N/A(理由)`

## D0 宿主抽库（desktop/core，宿主无关）

| # | 能力 | 状态 | 证据 |
|---|---|---|---|
| D0.1 | 跨块中文 UTF-8 / CRLF / 坏帧 / 取消流解析（`SseDecoder` 复用） | 完成 (b3ce4ad7) | core 单测 + Windows 点验 |
| D0.2a | Windows 进程/控制台工具（win_cmd：CREATE_NO_WINDOW、kill tree、scoped kill） | 抽库完成 (本次) | core 编译；真机点验待 GD1 |
| D0.2b | 秘密文件权限（secret_fs：unix 0600 / Windows DACL） | 抽库完成 (本次) | 同上 |
| D0.2c | runtime 目录发现（install / AppData / monorepo / env 覆盖） | 抽库完成 (本次) | 单测 |
| D0.2d | native PG+Redis ensure/stop（initdb / 角色供给 / pgvector / client.env） | 抽库完成 (本次) | 单测 + Tauri 调库回归 |
| D0.2e | client.env 生成（jwt/byok/upload 签名、身份 uuid、解析器、云中继注入参数化） | 抽库完成 (本次) | 单测 |
| D0.2f | docker 探测与安装引导（非 Windows） | 抽库完成 (本次) | 单测 |
| D0.2g | 本机产品生命周期（avrag-migrate 迁移 → api/worker spawn、pid、健康、停止、scoped sweep） | 抽库完成 (本次) | 单测 + Tauri 调库回归 |
| D0.2h | 本地 B2C 会话（凭据文件、登录/注册、会话持久化） | 抽库完成 (本次) | 单测 |
| D0.2i | Tauri 命令薄包装（local_host.rs）+ HostError 转换 | 完成 (本次) | src-tauri check + 29 tests |

## D0.3 documents / REST / 上传 / 本地目录接口抽库

| # | 能力 | 状态 | 证据 |
|---|---|---|---|
| D0.3a | documents/reindex 命令逻辑抽库（token 由宿主注入） | 完成 (本次) | core tests (extract shapes) + src-tauri 薄包装回归 |
| D0.3b | api_call/upload_bytes 代理抽库（REST 代理、上传安全边界、zstd 导出解码、超时分级） | 完成 (本次) | core tests 7 例（上传 URL 拒绝远端/错路径/错端口、zstd 往返、路径归一） |
| D0.3c | 上传范围、失败恢复、目录与本地服务不可达行为（HostError 503 映射 + loopback/端口匹配断言） | 完成 (本次) | 同上测试 |

## D0.4 cloud session / Publish / 深链抽库

| # | 能力 | 状态 | 证据 |
|---|---|---|---|
| D0.4a | cloud session（登录/登出/token 刷新/relay 凭据）抽库 | 未开始（当前 render_relay_env 由 host 注入） | — |
| D0.4b | Publish 状态机抽库 | 未开始 | — |
| D0.4c | 深链验证、updater/单实例平台 adapter 边界（D5 明确） | 未开始 | — |

## D1 GPUI 真聊天（GD1：Windows 真机）

| # | 能力 | 状态 | 证据 |
|---|---|---|---|
| D1.1 | gpui-kit 依赖锁定 rev / 窗口 / composer / 中文 IME（真实输入无重复字） | 未开始 | — |
| D1.2 | 共享 reducer 流式显示、local session 真 API、取消、历史（127.0.0.1:18080） | 未开始 | — |

## D2–D5 桌面对等

| # | 能力 | 状态 |
|---|---|---|
| D2 | 栈状态、启动迁移、进程/日志、退出收摊 UI（S0–S6，不依赖预装 Docker） | 未开始 |
| D3 | Workspace、文件、入库、引用、会话与笔记 | 未开始 |
| D4 | BYOK、角色、云登录、钱包、Publish | 未开始 |
| D5 | updater、深链、单实例、浏览器打开、MCP/CLI | 未开始 |

## D6 三平台验收

| # | 能力 | 状态 |
|---|---|---|
| D6 | 同一 crate 的 Windows/macOS/Linux 构建、安装/升级/卸载、发布产物 | 未开始 |
