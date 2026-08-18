# Windows 桌面客户端 E2E PR-3 交接

| 字段 | 内容 |
|------|------|
| **日期** | 2026-08-14 |
| **状态** | Handoff |
| **范围** | PR-0 至 PR-3 已实现并验证；PR-4 产品对齐尚未开始 |
| **结论** | 默认 `l1` 现在能跑通 shell+ingest + 配置鸿沟探针；配置鸿沟仍是 **GAP**，不是全量问答 |

## 当前状态

PR-0 至 PR-3 的实现已落地，Windows 真机验证通过：

```text
DESKTOP_E2E_YES=1 \
DESKTOP_E2E_WIN_FRONTEND='C:\dev\context-osv6\frontend_next' \
bash scripts/desktop-e2e/run.sh l1
```

最后结果为 `4 passed`：

1. `chat-dead-endpoint.spec.ts`：死 endpoint 快速失败。
2. `chat-unconf.spec.ts`：无 `llm-config.json` 时显示未配置文案。
3. `config-gap.spec.ts`：抽屉回读、provider secret 可见、聊天仍走 legacy。
4. `nav-upload.spec.ts`：建库留在 WebView，IPC 上传并 ingest `Completed`。

## 已实现文件

### 测试编排

- `scripts/desktop-e2e/run.sh`
- `scripts/desktop-e2e/l0.ps1`
- `scripts/desktop-e2e/backup-appdata.ps1`
- `scripts/desktop-e2e/seed-legacy-llm.ps1`
- `scripts/desktop-e2e/markitdown-wsl.cmd`
- `scripts/desktop-e2e/README.md`

### Windows Playwright

- `frontend_next/playwright.desktop-client.config.ts`
- `frontend_next/e2e/pom/desktop-workbench.ts`
- `frontend_next/e2e/specs/desktop-client/webview.ts`
- `frontend_next/e2e/specs/desktop-client/external-browser.ts`
- `frontend_next/e2e/specs/desktop-client/helpers.ts`
- `frontend_next/e2e/specs/desktop-client/nav-upload.spec.ts`
- `frontend_next/e2e/specs/desktop-client/chat-unconf.spec.ts`
- `frontend_next/e2e/specs/desktop-client/chat-dead-endpoint.spec.ts`
- `frontend_next/e2e/specs/desktop-client/config-gap.spec.ts`

### 产品代码

- `desktop/src-tauri/src/commands/api.rs`：`upload_bytes` 与上传 URL 白名单。
- `desktop/src-tauri/src/lib.rs`：WebView 外链守卫、`upload_bytes` 注册、`RunEvent::Exit` 清理。
- `desktop/src-tauri/src/app_nav.rs`：`tauri.localhost` 与静态导出 URL 规则。
- `desktop/src-tauri/src/commands/lifecycle.rs`：产品、数据面、作用域清扫。
- `desktop/src-tauri/src/commands/win_cmd.rs`：Windows 进程树与无控制台辅助。
- `desktop/src-tauri/src/commands/local_product.rs`：TCP health、迁移环境、原生停止路径。
- `frontend_next/lib/runtime/tauri-ipc.ts`：`chat_stream` reject 时保留结构化错误。

## 关键运行事实

- L1 使用 `CONTEXT_OS_STATE_HOME=%TEMP%\cos-e2e-<runid>\state` 隔离数据树。
- WebView2 CDP 端口默认 `19322`，使用独立 `WEBVIEW2_USER_DATA_FOLDER`。
- L1 启动前备份 `local_user.json`、`local_session.json`、`llm-config.json`，teardown 后恢复。
- L1 启动时注入：
  - `E2E_ENABLED=true`，绕过本机 API 的 60 RPM 固定窗口限流。
  - 临时 `BYOK_MASTER_KEY`，使 provider-secret 路由能加密 dummy secret。
  - `MARKITDOWN_BIN` 指向 WSL-backed `markitdown-wsl.cmd`。
- `seed-legacy-llm.ps1` 写入：

```json
{
  "provider": "custom",
  "base_url": "http://127.0.0.1:9",
  "api_key": "e2e-not-a-real-key",
  "model": "e2e-dummy",
  "timeout_ms": 2000
}
```

## 配置鸿沟探针

`config-gap.spec.ts` 目前验证：

1. 临时 `BYOK_MASTER_KEY` 下，dummy provider secret 可 `PUT`。
2. `GET /api/v1/settings/provider-secrets` 返回 fingerprint，不返回明文。
3. 抽屉回读到 legacy `llm-config.json` 的字段。
4. 聊天仍走 legacy 死 endpoint，快速失败。

该测试描述的是 **GAP**，不是 PR-4 已完成。PR-4 落地后，这个断言必须在 PR-5 翻转：删除 `llm-config.json` 后，聊天应改走本机 API，而不是继续指向 `127.0.0.1:9`。

## 必须交接出去的 caveat

### markitdown 仍不是发布能力

当前 L1 的 ingest `Completed` 依赖 `markitdown-wsl.cmd`，它只适用于这个 WSL 开发机。正式 Windows 安装树仍未带 parser。发布门禁不能把这条 shim 当成产品 parser。

### BYOK_MASTER_KEY 仍是测试临时值

`write_client_env` 现在不写 `BYOK_MASTER_KEY`。L1 之所以能 upsert dummy provider secret，是因为 harness 临时注入了环境变量。PR-4 必须决定桌面客户端如何持久生成和保存这个 key。

### `E2E_ENABLED` 只应测试注入

不要把它写进正式 `client.env`，也不要依赖它规避生产限流。

### 改 frontend 后必须重建 shell

改 `frontend_next/lib/runtime/tauri-ipc.ts` 这类源码，只同步到 Windows 副本不会影响安装树。需要用：

```bash
SKIP_SIDECARS=1 LAUNCH=0 bash scripts/dev-windows-hotswap.sh hotswap
```

把新的静态导出重新打进 `Context-OS.exe`，再跑 L1。

## 已验证命令

```text
DESKTOP_E2E_YES=1 ... run.sh l1        4 passed
DESKTOP_E2E_YES=1 ... run.sh audit     ok
cargo test -p avrag-desktop --lib      35 passed
pnpm exec tsc --noEmit                 ok
pnpm vitest run tests/runtime/tauri-ipc.test.ts   8 passed
code-review-graph update               ok
```

## 下一站

下一步是 PR-4 产品对齐，不是继续加 E2E：

1. 桌面聊天从 `run_desktop_chat` 改为本机 `avrag-api` `execute_stream`。
2. 删除 legacy `llm-config.json` 作为聊天真相。
3. 实现 G1-G6：无平台 key 时从 secret 构造 client、RAG/env/restart/reindex。
4. 将 `BYOK_MASTER_KEY` 纳入桌面正式状态。
5. 解决 Windows 包的 parser provisioning。

PR-4 合入后，PR-5 再翻转 `config-gap.spec.ts`，加入 opt-in 真 LLM 和 `D-rag-full` 断言。
