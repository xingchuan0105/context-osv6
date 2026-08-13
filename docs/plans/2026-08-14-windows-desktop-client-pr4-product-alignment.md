# Windows 桌面客户端 PR-4 产品对齐（Layer A）

| 字段 | 内容 |
|------|------|
| **日期** | 2026-08-14 |
| **状态** | In progress |
| **类型** | 产品设计 + 实现（不是 E2E 测试） |
| **上游** | `plans/2026-08-13-windows-desktop-client-e2e-journey-design.md`（KD1/G1–G6/Q2/Q6）、`plans/2026-08-14-windows-desktop-client-e2e-pr3-handoff.md` |
| **结论** | 本文跟踪 G1–G6 的产品对齐；**本波（Layer A）只做聊天对齐**，RAG 翻开（G2–G6）留 Layer B。 |

## 1. 已拍板（本次会话确认）

| # | 决策 | 取值 |
|---|------|------|
| D1 | 分层 | **Layer A 先做聊天对齐**；RAG 翻开（G2–G6 + reindex）留 Layer B |
| D2 | Q2（providers 是否立刻取代旧配置面） | **立刻退役**：删 `llm-config.json` 作为聊天真相，删 `/setup` LLM 预设 + 抽屉 `llm`/`embedding`/`diagnostic` 三 tab，BYOK 只走 `/settings?tab=providers`（canonical，PRODUCT_IA §2） |
| D3 | `BYOK_MASTER_KEY` 持久化 | **写进 `client.env`**（与 `JWT_SECRET` 同文件，`write_client_env` 生成） |

## 2. 现状（已对代码，非目标态）

- WebView `useChatStream` → `transport.streamChat` → `isTauri()` → `streamChatViaIPC` → Tauri `chat_stream` → `run_desktop_chat` → `load_llm_config(%APPDATA%/com.contextos.desktop/llm-config.json)` → `LlmClient::complete`（单次、非流式、无工具、无 RAG）。
- REST 已走 `api_call` → 本机 `:18080`；**只有聊天被旁路**。
- `bind_byok_client(None, secret)` 返回 `None`（`app-chat/.../unified/mod.rs:127` 的 `(c, _) => c`）→ 无平台 `AGENT_LLM_*` 时，种了 secret 也拿不到 client（G1）。
- `PgProviderSecretStoreAdapter::from_env` 在无 `BYOK_MASTER_KEY` 时 fail-closed → `provider_secrets = None`；`write_client_env` 今天不写它。
- 聊天 SSE 端点 = `POST /api/v1/chat`（`transport-http/routes/chat.rs` + `handlers/chat.rs`），body `{...ChatRequest, stream:true}`，`Accept: text/event-stream`，Bearer token，SSE `data` 即 `ChatEvent` JSON。
- PNA 约束：WebView（`tauri.localhost`）不能直接 `fetch` `127.0.0.1:18080`（上传同因走了 `upload_bytes`），所以聊天必须保留 IPC 边界，由 Tauri 进程代理本机 API。
- `/settings?tab=providers` 在 `(app)` 路由组、包 `AppShellGate`（`components/desktop/AppShellGate`），桌面可达；providers panel 走 `restRequest` → IPC `api_call`，本机 API 已有 secrets 路由。

## 3. Layer A（本波实现）

### A1 — G1：无平台 client 时从 resolved secret 构造 `LlmClient`

**文件**：`avrag-rs/crates/app-chat/src/agents/unified/mod.rs`

把 `bind_byok_client` 从「只在已有 client 上 overlay」改为「无 client 但有 secret 时**构造**」：

```rust
fn bind_byok_client(
    client: Option<LlmClient>,
    byok: Option<&app_core::ResolvedProviderSecret>,
) -> Option<LlmClient> {
    match (client, byok) {
        (Some(c), Some(secret)) => Some(c.with_user_credentials(
            secret.api_key.clone(),
            secret.base_url.clone(),
            secret.model_hint.clone(),
        )),
        (None, Some(secret)) => llm_client_from_secret(secret),
        (c, None) => c,
    }
}
```

`llm_client_from_secret` 从 `ResolvedProviderSecret`（`api_key`/`base_url`/`model_hint`/`provider`）构造 `avrag_llm::ModelProviderConfig`：

- `base_url` / `model_hint` 任一为空 → `None`（secret 不完整，fail-open 回平台路径，与现有一致）。
- `api_style = ApiStyle::OpenAi`（BYOK secret 一律 OpenAI-compatible 单路由，与 legacy `LocalLlmConfig::inferred_api_style` 一致；不写多方言）。
- `timeout_ms = 120_000`、`dimensions=None`、`enable_thinking/None`、`enable_cache/None`、`rpm_limit/tpm_limit=None`。
- 用 `LlmClient::new(config)`（无 pool；单 key）。

`Chat`/`Rag`/`Search` 三分支已经 `bind_byok_client(...)`，无需改调用点。加单测覆盖 `(None, Some(secret)) → Some` 与 `(None, None) → None`、secret 缺 base_url/model → `None`。

### A2 — 桌面聊天改走本机 API `execute_stream`（删 legacy 路径）

**文件**：`desktop/src-tauri/src/commands/chat_stream.rs`、`chat.rs`、`api.rs`

`chat_stream` 命令不再 `run_desktop_chat`，改为 SSE 代理到本机 `avrag-api`：

1. 复用 `parse_chat_request_id` / `session_id_from_request` / license gate（v0.2.0 恒 true，保留）。
2. `POST {product_api_base_url}/api/v1/chat`，body = 收到的 request JSON（frontend 已带 `stream:true`），`Accept: text/event-stream` + Bearer token（沿用 `api_call` 的 token 透传）。
3. 用 `reqwest::Response::bytes_stream()` 增量读 body，手工解析 SSE（`event:`/`data:` 行、空行分帧、跳过注释/keep-alive），把每个 `data` JSON 反序列化为 `contracts::chat::ChatEvent`，经现有 `emit_chat_event(app, request_id, &event)` 重发到 `chat://{request_id}`。
4. 取消：`chat_cancel` 的 `cancel` flag 在帧间检查；并在 drop 时 abort reqwest（用 `tokio::select!` 或 `cancel` 轮询 + 中断发送方）。
5. 连接失败 / 非 2xx → 沿用 `error_events` 发一个 `ChatEvent::Error`，文案与现状 `api_call` 的 `service_unavailable` 对齐（“Local product API not reachable…”）。

**删除**：`chat.rs` 的 `run_desktop_chat`、`stream_llm_response`、`LLM_NOT_CONFIGURED`、`load_llm_config` 引用；`chat_stream.rs` 的 `run_desktop_chat` 导入。保留 `chat_event_channel` / `parse_chat_request_id` / `session_id_from_request` / `query_from_request` / `error_events` / `LICENSE_REQUIRED`（仍被代理与 license gate 使用）。`desktop_placeholder_events` 若无引用一并删。

### A3 — `write_client_env` 持久化 `BYOK_MASTER_KEY`

**文件**：`desktop/src-tauri/src/commands/native_stack.rs`

`write_client_env` 生成（若 `byok.key` 不存在）或读取一个 32 字节随机 key，以 base64 写入 `client.env` 的 `BYOK_MASTER_KEY=`。与 `jwt.secret` 同模式：首次 ensure 生成后持久到 `rt` 树下的小文件（如 `byok.key`），后续 ensure 复用，避免重启换 key 导致已加密 secret 无法解密（G6 前置：**key 必须稳定**）。`client.env` 里的 `BYOK_MASTER_KEY` 值 = base64(key)。

- 本地 API/worker 进程读 `client.env` → `PgProviderSecretStoreAdapter::from_env` 成功 → `provider_secrets` 生效（配合 A1，聊天可用 BYOK）。
- 不写进 `AGENT_LLM_*`（平台 key 仍是可选，Layer B 再议）。

### A4 — 退役旧配置面（Q2）

**前端**：

- `frontend_next/lib/desktop/tauri-llm.ts` → 拆分：LLM 相关（`LocalLlmConfig` 等类型 + `get/set/test/diagnose/list_available_models` + `mergeLlmConfigPatch`/`executeRepairAction`/`repairActionLabel`）**删除**；本机运行时（stack/docker/product/session 的类型与函数）**搬到** `frontend_next/lib/desktop/tauri-local.ts`。
- `frontend_next/lib/desktop/llm-presets.ts` → 删除。
- `frontend_next/app/(desktop)/setup/page.tsx` → 删除（或改为 `<DesktopOnlyGate>` 内跳 `/settings?tab=providers`；倾向删除，由路由重定向处理）。
- `frontend_next/components/desktop/LLMDiagnosticPanel.tsx` → 删除。
- `frontend_next/components/desktop/DesktopSettingsDrawer.tsx` → 删 `llm`/`embedding`/`diagnostic` 三 tab 及其状态/处理器；`tabs` 只留 `stack`/`license`；加一个指向 `/settings?tab=providers`（in-app）的入口（“模型 Provider →”）。
- `frontend_next/components/desktop/ClientLocalSessionBootstrap.tsx` → 改从 `tauri-local.ts` import。
- 删 `frontend_next/tests/desktop/tauri-llm.test.ts`、`llm-presets.test.ts`。

**Rust**：`desktop/src-tauri/src/commands/llm_config.rs` 删除；`lib.rs` 移除 `get/set/test/diagnose/list_available_models` 的注册与导入。

**/setup 路由**：确认无其它 `/setup` 引用后，加客户端重定向到 `/settings?tab=providers`（或复用现有 guard）。

## 4. Layer B（本波不做，记录门闩）

| # | 缺口 | 代码 | Layer B 动作 |
|---|------|------|-------------|
| G2 | `client.env` 无 `AGENT_LLM_*` | `write_client_env` | 平台 key 仍可选；桌面默认走 secret 构造（A1 已覆盖聊天） |
| G3 | RAG 每次 ensure 写回 `false` | `write_client_env` | upsert embedding 后写 `AVRAG_ENABLE_RAG=true`，ensure 不再盲目覆盖 |
| G4 | `enable_rag=true` 需 embedding client | `app-bootstrap` | 从 `purpose=embedding` secret 构造 `EmbeddingClient` |
| G5 | RAG-off 时 ingest 跳向量 | `build_worker_retrieval_data_plane` | 翻开后 reindex / re-upload |
| G6 | 开关 vs secret 鸡生蛋 | ensure/env | 规定 upsert embedding 后谁写 env、何时 restart api/worker + reindex |

Layer B 完成前，`D-rag-full` / `config-gap.spec.ts` 的 `PASS` 翻转仍是 PR-5。

## 5. 验证

- `cargo test -p app-chat --lib`（G1 单测）。
- `cargo test -p avrag-desktop --lib`（chat proxy / BYOK / 移除 legacy）。
- `cd frontend_next && pnpm exec tsc --noEmit` + `pnpm vitest run`（前端删改后）。
- `code-review-graph update`（结构变更后，同会话）。
- L1 Windows E2E 不在本波必跑（真实 Playwright/LLM 非 mid-wave 门）；PR-5 再翻 `config-gap.spec.ts`。

## 6. 回滚

- Layer A 是本产品 PR 的回滚单元：`revert` 本提交即回到 legacy 单次 complete 路径。
- `client.env` 的 `BYOK_MASTER_KEY` 是新增键，回滚后本地 API 不再解析 provider secrets，不影响其它功能。

## 7. 非目标 / 显式 defer

- **parser provisioning（markitdown shim）**：`markitdown-wsl.cmd` 仍是 WSL 开发机专属，不是发布能力。Windows 安装树的 parser 打包属独立发布工作，不在 Layer A。见 PR-3 交接 caveat。
- 不引入 LiteLLM；不写双聊天路径；不改云侧 secrets 表 schema。
- 不做 J3–J8 / 钱包 / 分享上云 / NSIS 卸载。

## 8. Layer B 实现（2026-08-14 追加，进行中）

**Q6 已拍板**：保存 SiliconFlow embedding/rerank key → **自动 force-restart** api/worker（RAG 即对新文档生效）；旧文档走**手动重索引**（不静默烧 embedding token）。

**目标流**：`/settings?tab=providers` 保存 embedding key → 前端触发 force-restart → 重启时 API/worker 从 secret 构造 embedding/rerank client → RAG 对新上传生效 → 旧文档 `POST /api/v1/documents/{id}/reindex`（已有端点）手动补齐向量。

### B1 — G2/G3：桌面不再硬写 RAG off

`native_stack.rs::write_client_env`：`AVRAG_ENABLE_RAG=false` → `true`，并写 `AVRAG_EMBEDDING_DIM=1024`（SiliconFlow bge-m3 原生 1024d，只作 schema 尺寸，不进请求 `dimensions`）。

理由：RAG 是否真正可用改由「能否构造出 embedding client」决定，而不是 env 开关。首启无 key → embedding 缺 → RAG 优雅降级（不 fail-fast），与 G5 语义一致。

### B2 — G4：从 secret 构造 embedding/rerank client

- `app-core/provider_secret_domain.rs` 增 `ResolvedProviderSecret::to_llm_config()`（OpenAI 方言、`dimensions=None`、`timeout_ms=120_000`），复用 G1 的构造逻辑。
- API bootstrap（`app-bootstrap/src/lib.rs`）：把 `provider_secrets` 建到 `rag_runtime` 之前；resolve `purpose=Embedding` / `Rerank`（owner=config 的 bootstrap owner，`workspace_id=None` 账户默认）；`embedding = platform config.embedding OR secret`；**去掉 fail-fast**（无 embedding → `rag_runtime=None`，不崩启动）；reranker 同 platform OR secret。
- worker bootstrap（`bins/worker/src/lib.rs` + `runtime_support.rs`）：resolve `purpose=Embedding` secret → `embedding_client = platform OR secret`；`retrieval_data_plane` 仍由 `enable_rag`（现恒 true）门控。

### B3 — G6：保存后 force-restart

- 新增 IPC `restart_local_product`（或 `ensure_local_product` 加 `force`），强制 stop→ensure，绕过「已 healthy 即返回」快路径。
- `frontend_next/lib/desktop/tauri-local.ts` + providers panel：保存 `purpose=embedding|rerank` 成功后调用该 IPC。

### B4 — G5：手动重索引

- 复用 `POST /api/v1/documents/{id}/reindex`；前端在设置面板给「重新索引本机文档」入口（遍历 docs 逐个 reindex），标注会消耗 embedding token。

### 实施状态（2026-08-14）

**已落地并验证：**

- **D1 确定性本机 owner**（Q「G4 身份对齐」拍板）：`native_stack.rs::local_identity_uuids()` 用 `compute_device_id()` 派生 owner/user（`Uuid::new_v5`），写进 `client.env` 的 `NEXT_PUBLIC_DEV_OWNER_USER_ID` / `NEXT_PUBLIC_DEV_USER_ID`；本机注册带 `local:true`，`register_user` 用 config owner（`RegisterUserInput.owner_user_id/user_id`），仅当 `NEXT_PUBLIC_DEV_OWNER_USER_ID` 存在时生效（云不会误触发）。
- **B1**：`write_client_env` 写 `AVRAG_ENABLE_RAG=true` + `AVRAG_EMBEDDING_DIM=1024`。
- **B2/G4**：`ResolvedProviderSecret::to_llm_config()`；API + worker bootstrap 从 `purpose=Embedding/Rerank` secret 构造 client（platform OR secret），无 embedding 时 `rag_runtime=None` 优雅降级。
- **B3/G6**：`restart_local_product` IPC + providers panel 保存 embedding/rerank 后自动 force-restart。

**待办（PR-5）：**

- B4 的「重新索引本机文档」批量 UI 入口（端点已存在）。
- `config-gap.spec.ts` 翻 PASS + `D-rag-full`（需真 key + Windows）。

验证：`cargo test -p app-core --lib`、`-p app-bootstrap --lib`、`-p app-chat --lib`、`-p avrag-desktop --lib`；`cargo build -p avrag-worker`、`-p transport-http`；`pnpm tsc`；`code-review-graph update`。真实 RAG 走通仍留 PR-5 `D-rag-full`（需真 key + Windows）。
