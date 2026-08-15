# Windows 桌面客户端 产品对齐 + 真机验收 交接

| 字段 | 内容 |
|------|------|
| **日期** | 2026-08-14 |
| **状态** | Handoff |
| **范围** | PR-4（Layer A/B，G1–G6）+ PR-5（spec 翻转）+ G5 重索引 + Windows 真机验收 |
| **结论** | 产品对齐**已落地并提交**；Windows 真机 **L1 全绿**；**D-rag-full 产品路径跑通，但 RAG 检索覆盖度不足（1 个 agent-lane bug，未闭环）** |

## 1. 已提交（5 个 commit）

| commit | 内容 |
|--------|------|
| `30249aca` | PR-4 Layer A：桌面聊天走 `avrag-api` `execute_stream`（删 legacy `run_desktop_chat`/`llm-config.json`）；G1 从 secret 构造 `LlmClient`；`BYOK_MASTER_KEY` 持久化；退役 `/setup`/抽屉 llm/embedding/diagnostic |
| `979f4d88` | PR-4 Layer B（G2–G6）：确定性本机 owner（owner==user）；`write_client_env` 写 `AVRAG_ENABLE_RAG=true`+`AVRAG_EMBEDDING_DIM=1024`；G4 从 `purpose=embedding/rerank` secret 构造 client（API+worker）；`restart_local_product` IPC |
| `0873574b` | PR-5：翻转 `config-gap`/`chat-unconf`/`chat-dead-endpoint` 到对齐后行为；新增 `chat-rag.spec.ts`（D-rag-full，opt-in） |
| `7a34139e` | G5：`reindex_local_documents` IPC + providers panel「重新索引本机文档」按钮 |
| `559c042d` | **真机验收修复**（见 §3） |

## 2. Windows 真机验收结果

**L1 默认套件（无 key）— ✅ 4 passed / 1 skipped（21.9s）**

| spec | 结果 |
|---|---|
| `chat-dead-endpoint` | ✅ dummy `:9` secret 经本机 API 快速失败 |
| `chat-unconf` | ✅ 无 llm secret → `LLM client is not configured` |
| `config-gap` | ✅ provider secret 可见 + 聊天走本机 API |
| `nav-upload` | ✅ 建库 + IPC 上传 + ingest `Completed` |
| `chat-rag` | ⏭️ skip（无 key） |

**真 key `D-rag-full` — ⚠️ 产品路径通，检索覆盖度不足**

- G4 ✅：worker 用 SiliconFlow secret 成功向量化（`text embedding done vectors=1`）。
- 聊天 ✅：`capabilities=["rag"]` → Lead+Workers（`retrieve_strategy="lead_workers"`）完整执行，re-brief 1 次。
- ❌ 检索 `coverage="insufficient"`：`bm25 raw_hit_count=1 hydrated_hit_count=1` 但 short_sac `n_hits=0`、最终 `citations: []`。

## 3. 真机验收发现并修复的真实 bug（`559c042d`）

1. **确定性 owner 必须 owner==user**：`user_provider_secrets.owner_user_id` FK → `users.id`，个人账号模型 owner==user，派生不同 uuid 会违反外键。
2. **worker 文本向量索引 gate 在 embedding client**：否则 `AVRAG_ENABLE_RAG=true` + 无 secret 时 ingest 报 `index_embedding` 错；现优雅跳过向量。
3. **modes/prompts 打包**：`avrag-api` 按 CWD 找 `modes/*.yaml` + `prompts/*.md`；hotswap 现拷到 exe 旁。
4. **run.sh**：`DESKTOP_E2E_LLM=1` 从 `.env` 映射真 key + `--grep`；`l0.ps1` 的 `S-desktop-env` 断言翻成 `AVRAG_ENABLE_RAG=true`。
5. **D-rag-full spec**：选 source + 开「知识库」chip 再提问（RAG 需显式选源）。

## 4. 未闭环：RAG 检索命中被丢弃（agent-lane bug）

> **⚠️ 修正（2026-08-15）：本节 knockout 假设已证伪，勿沿此线索。** 真根因是 Windows 构建里沙箱 bridge 为编译期存根（`#[cfg(not(unix))]` 直接报错）→ 桌面端每次 SaC codegen 必败 → 零检索 → `n_hits=0`；`KNOCKOUT_HARD_SUPPRESS` 是编译期 `false`，knockout 路径本就是 no-op。修正后的完整诊断与修复见 [`2026-08-15-windows-sandbox-bridge-port-handoff.md`](2026-08-15-windows-sandbox-bridge-port-handoff.md) §0。以下为被证伪的原始记录，仅作历史保留。

**症状**：`retrieve_bm25_stage` 明确命中 1 条（`raw_hit_count=1 hydrated_hit_count=1`，chunk 有正文），但 short_sac 装配出的 EvidencePack `n_hits=0` → `coverage=insufficient` → 无引用。

**诊断进度（已排除 + 已定位）**：

- ✅ 已加紧回环单测：`cargo test -p agent-loop --lib evidence_from_tool_results` → 绿。证明 `evidence_from_tool_results`（ToolResult→EvidenceItem）**抽取本身正常**。
- ✅ 逐读排除了 `cut_top_k` / `adaptive_k`（单命中 k=1）/ `finalize_evidence_package` / `adjacent_merge_shortlist_longlist`——都不丢单 chunk。
- 🎯 收敛到：short_sac 的 SaC 循环里，`RuntimeBridge::call` 捕获的 ToolResult 进入 `worker_state.tool_results` 前，经 `iteration_codegen.rs` → `ko.align_tool_results_no_count` → `helpers/knockout.rs::strip_suppressed_only` 按 `chunk_id` 过滤「已看过的 chunk」，被剔后 `data.chunks` 变空。
- ⏳ **未坐实**：`KNOCKOUT_HARD_SUPPRESS` 开关与 knockout 时序（默认关则此假设不成立，需另找）。

**建议下一刀**：在 `record_bridge_evidence` / `align_tool_results_no_count` 打 `[DEBUG]` 日志（`data.chunks.len()` + knocked 集合），再跑一次 D-rag-full 坐实剔除点；修法是证据包改用「未 knockout」的原始捕获，而非过滤后的。

## 5. 关键运行事实（交接给下一个接手人）

- **sidecar 只 build-if-missing**：`scripts/stage-desktop-sidecars.sh` 的 `ensure_built` 在二进制已存在时**不重编**（`STAGE_BUILD=1` 语义是「缺失才 build」）。改了 avrag-rs 侧 crate 后必须**手动 force**：`cargo build --release --target x86_64-pc-windows-gnu -p avrag-api -p avrag-worker`，再拷进安装树。真机验收就栽在这：旧 `avrag-api.exe` 里还是旧的 fail-fast。
- **modes/prompts 必须与 exe 同目录**：`avrag-rs/{modes,prompts}` 拷到 `%LOCALAPPDATA%\Context-OS Client\`（hotswap 的 `copy_runtime_assets` 已做）。
- **Windows frontend 副本非 git worktree**：`C:\dev\context-osv6\frontend_next` 是独立拷贝，spec/POM 改动要**手动 cp** 同步（`e2e/specs/desktop-client/*` + `e2e/pom/desktop-workbench.ts`）。
- **真 key 验收命令**：
  ```bash
  DESKTOP_E2E_YES=1 DESKTOP_E2E_WIN_FRONTEND='C:\dev\context-osv6\frontend_next' bash scripts/desktop-e2e/run.sh l1   # 默认
  DESKTOP_E2E_YES=1 DESKTOP_E2E_LLM=1 DESKTOP_E2E_GREP='D-rag-full' DESKTOP_E2E_WIN_FRONTEND='...' bash scripts/desktop-e2e/run.sh l1   # 真 key
  ```
- 真 key 从 `avrag-rs/.env` 静默映射（`AGENT_LLM_*`→`DESKTOP_E2E_LLM_*`、`EMBEDDING_*`→`DESKTOP_E2E_EMBED_*`），值不进日志。

## 6. 未提交

- `avrag-rs/crates/agent-loop/src/react_loop/run_lead_workers.rs`：诊断时加的两个单测（`evidence_from_tool_results_extracts_lexical_hit` + `_skips_non_ok_or_empty_text`），是有效的 seam 回归测试，可随 §4 的修复一起提交。
- PR-0–PR-3 的 E2E harness 仍有一批未提交：`nav-upload.spec.ts`、`webview.ts`、`external-browser.ts`、`playwright.desktop-client.config.ts`、`seed-legacy-llm.ps1`、`backup-appdata.ps1`、`markitdown-wsl.cmd` 等（这些是 PR-0–PR-3 的测试编排，未纳入上面的产品对齐 commit）。

## 7. 下一站

1. **闭环 RAG 检索覆盖度 bug**（§4）——坐实 knockout 剔除点并修，让 `D-rag-full` 的 citation 断言跑绿。
2. **parser provisioning**：`markitdown-wsl.cmd` 仍是 WSL 开发机专属 shim，发布安装树需自带 parser（独立发布工作，未纳入）。
3. 收口 PR-0–PR-3 未提交的 E2E harness 文件。
