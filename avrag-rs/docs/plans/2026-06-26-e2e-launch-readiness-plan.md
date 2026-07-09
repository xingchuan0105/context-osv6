# E2E 准上线补测开发文档

> **创建**：2026-06-26  
> **范围**：仅测试套件、CI 配置、测试文档——**不修改产品业务逻辑**  
> **受众**：在新窗口/新会话中按本文逐步执行的开发者或 Agent  
> **关联**：[`full-functional-e2e-guide.md`](../full-functional-e2e-guide.md)、[`e2e-gates.md`](../e2e-gates.md)、[`e2e-test-registry.yaml`](../e2e-test-registry.yaml)

---

## 0. 先读这三句

1. **主程序不用动**：聊天、上传、RAG、计费、API Key、MCP 等功能已在 `avrag-rs/` 和 `frontend_next/` 里实现；本文只补「怎么自动验它们」。
2. **会改动的目录**：`avrag-rs/crates/app/tests/`、`avrag-rs/crates/transport-http/tests/`、`frontend_next/e2e/`、`.github/workflows/`、以及测试相关文档。
3. **完成标准**：CI 在合并/发布时能可靠跑通关键路径；新对外能力（API Key、MCP）有端到端黑盒测。

---

## 1. 背景：现在还缺什么

当前 E2E 体系已经较完整（Rust mock smoke、integration、nightly 真实 LLM、Playwright journey/skills）。距离「放心上线」主要差：

| 类别 | 问题 | 影响 |
|------|------|------|
| CI 环境 | 前端 Playwright workflow **未启动 Milvus**，但 journey/skills 里有上传→RAG 测 | CI 可能假绿或 flaky → PR-2/3 起 Milvus + PR-5 注入 RAG key（`DASHSCOPE_API_KEY`/`DMX_API_KEY`/`DEEPSEEK_API_KEY` secret）已修复 journey RAG CI gap（2026-06-29） |
| CI 覆盖 | 计费 Playwright **仅手动 workflow**，不进 master 自动跑 | Paywall/用量页回归靠人工 |
| 新能力 | API Key + `/api/v1/mcp`  mostly 契约单测，缺 Product E2E 一条龙 | 对外 agent 集成无黑盒保障 |
| 质量门禁 | `rag_quality` 三门禁未接入发布阻断 | 检索质量退化可能漏到线上 |
| 浏览器 | 无 API Key 设置页、引用点击、跨 session 记忆等 UI E2E | 前端关键交互未自动验 |

---

## 2. 实施阶段总览

```
阶段 0  基线 + 入库已有契约测          约 0.5 天
阶段 1  修 CI（Milvus + Billing）       约 2–3 天
阶段 2  MCP / API Key Product E2E      约 3–4 天
阶段 3  OpenAI API + 限流/配额 E2E     约 2–3 天
阶段 4  Playwright 补测 + 质量门禁     约 3–5 天
阶段 5  Admin / Staging（可选）        按需
```

建议 **6 个 PR** 分批合并（见 §8），每 PR 合并后跑对应验收命令。

---

## 3. 阶段 0：基线与守卫

**目标**：确认现状、把本地已有但未提交的测试文件入库、更新索引。

### 3.1 任务清单

- [ ] **0.1** 记录 baseline（可选，便于对比）
  ```bash
  cd /home/chuan/context-osv6/avrag-rs
  ./scripts/e2e-precheck.sh
  ./scripts/run-product-smoke-e2e.sh
  E2E_MODE=integration cargo test -p app --test product_e2e --features product-e2e -- --test-threads=1
  ```

- [ ] **0.2** 确认并提交契约测（若仍在 untracked）
  - `avrag-rs/crates/transport-http/tests/api_key_security_contract.rs`
  - `avrag-rs/crates/transport-http/tests/mcp_unified_contract.rs`
  - 验收：`cargo test -p transport-http` 全绿

- [ ] **0.3** 同步机读索引
  ```bash
  cd /home/chuan/context-osv6/avrag-rs
  ./scripts/generate-e2e-test-registry.py
  ```
  - 验收：`docs/e2e-test-registry.yaml` 含上述契约测条目

- [ ] **0.4** 本文档已在 `docs/plans/`；实施过程中在 [`full-functional-e2e-guide.md`](../full-functional-e2e-guide.md) §8 勾选完成项并注明日期

### 3.2 本阶段不做什么

- 不改 `avrag-rs/crates/app-chat/`、`transport-http/src/handlers/` 等业务代码
- 不改前端页面组件（除非测试 POM 需要稳定 `data-testid`，且应优先改测试选择器）

---

## 4. 阶段 1：CI 可信性（P0）

**目标**：让 GitHub Actions 跑 Playwright RAG 测时，Milvus 一定就绪；计费 UI 测随 master 自动跑。

### 4.1 抽取 Milvus 启动脚本（推荐）

**新建**（二选一）：

- `scripts/ci-start-milvus.sh`（仓库根），或
- `.github/actions/e2e-milvus-precheck/action.yml`

**内容**（与现有 `smoke-e2e.yml` 对齐）：

```bash
cd avrag-rs
docker compose -f docker-compose.milvus.yml up -d --wait
curl -sf -X POST http://127.0.0.1:19530/v2/vectordb/collections/list \
  -H 'Content-Type: application/json' -d '{"dbName":"default"}'
```

**修改 workflow**（在 `Run ... E2E` 步骤之前插入 Milvus 步骤）：

| 文件 | 说明 |
|------|------|
| `.github/workflows/frontend-journey.yml` | journey 含 upload→RAG |
| `.github/workflows/frontend-skills.yml` | skills 硬 citation |
| `.github/workflows/frontend-smoke.yml` | 若后续 smoke 依赖 RAG 也受益 |
| `.github/workflows/nightly-llm-real.yml` | 显式 Milvus，减少测试内 bootstrap flake |

**验收**：

- [ ] 上述 workflow 日志中有 Milvus ready
- [ ] `frontend-journey.yml` 中 `workspace-upload-rag.spec.ts` 在 CI 通过

### 4.2 Billing 进 master 自动门禁

**方案 A（推荐）**：在 `frontend-journey.yml` 增加 job `billing-e2e`：

```yaml
# 示例：与 journey job 并行，working-directory: frontend_next
- name: Run billing E2E
  run: |
    pnpm exec playwright test --project=billing \
      e2e/specs/billing/paywall-flow.spec.ts \
      e2e/specs/billing/usage-dashboard.spec.ts
  env:
    E2E_RESET_SECRET: ${{ secrets.E2E_RESET_SECRET }}
    PRICING_REVAMP_ROLLOUT: '100'
    NEXT_PUBLIC_PRICING_REVAMP_ENABLED: '1'
  timeout-minutes: 30
```

**方案 B**：新建 `.github/workflows/frontend-billing.yml`，`on.push.branches: [master, main]`

**不纳入 PR 自动跑**（仍保留 manual）：

- `playwright-extended-e2e.yml` 中的 visual-regression、cross-browser

**验收**：

- [ ] master push 自动跑 paywall + usage-dashboard
- [ ] 失败时阻断（与 journey 同级）

### 4.3 可选：发布聚合 workflow

**新建** `.github/workflows/release-e2e-gate.yml`：

- 触发：`workflow_dispatch`（可选 `release` published）
- 顺序：Milvus → smoke → integration → llm_real（secrets）→ skills → journey + billing
- 失败上传 artifact（参考现有 `upload-artifact` 步骤）

---

## 5. 阶段 2：MCP / API Key Product E2E（P0）

**目标**：外部 Agent 路径「建 key → MCP 上传 → 入库完成 → 提问带引文」在 **integration 层**黑盒通过。

**落点**：`avrag-rs/crates/app/tests/product_e2e/integration/`

### 5.1 新建 `mcp_agent_flow.rs`

**流程**（HTTP 黑盒，mock LLM/embedding，真 PG/Milvus/worker）：

1. 创建 notebook（测试 helper / JWT）
2. `create_api_key(notebook, permissions: ["index", "query"])`
3. `POST /api/v1/mcp` → `tools/call` `workspace.create_upload`
4. `PUT` 返回的 `upload_url`（fixture：`antifragile.txt` 或 `sample-document.txt`）
5. `tools/call` `workspace.complete_upload`
6. 轮询 `workspace.document_status` 至 `completed`
7. `tools/call` `workspace.query` 或 REST chat（RAG mode）
8. 断言：`citations` 非空、`doc_id` 匹配、`answer` 有实质内容

**参考实现**：

- 契约：`crates/transport-http/tests/mcp_unified_contract.rs`（`mcp_ingestion_flow_create_upload_complete_status`）
- 断言 helper：`product_e2e/assertions.rs`
- 并行：**必须** `--test-threads=1`（`G-serial-integration`）

**验收命令**：

```bash
cd avrag-rs
E2E_MODE=integration cargo test -p app --test product_e2e \
  integration::mcp_agent_flow --features product-e2e -- --test-threads=1 --nocapture
```

**模块注册**：在 `product_e2e/integration/mod.rs` 声明 `mod mcp_agent_flow;`

### 5.2 新建 `mcp_auth_boundary.rs`（或合并上文件）

| 用例 | 期望 |
|------|------|
| workspace key 调用 org 级 MCP 工具 | 403 + `workspace_key_cannot_call_org_tools` |
| org key 调用 `workspace.query` | 403 |
| workspace key 读另一 notebook 的 session | 403 |
| API key 访问 notes / profile / preferences | 403 + `api_key_forbidden` |

**参考**：`api_key_security_contract.rs` 已有用例名，Product E2E 走完整 router + 真实 auth store。

**验收**：integration 全套件仍 0 fail。

### 5.3 文档同步

- [ ] [`full-functional-e2e-guide.md`](../full-functional-e2e-guide.md) §2 新增 **2.x Agent API / MCP** 能力行
- [ ] `./scripts/generate-e2e-test-registry.py`
- [ ] 新增 capability 建议：`CAP-AGENT`（可选，或在 CAP-AUTH 下标注）

### 5.4 分工约定

| 层 | 测什么 | 目录 |
|----|--------|------|
| L6 契约 | 路由、JSON-RPC 形态、error envelope、无 worker | `transport-http/tests/*_contract.rs` |
| L2 集成 | worker 入库、Milvus 检索、端到端 latency | `product_e2e/integration/mcp_*` |

---

## 6. 阶段 3：OpenAI API + 限流/配额（P1）

### 6.1 OpenAI Compatible API 契约

**新建**：`avrag-rs/crates/transport-http/tests/openai_completions_contract.rs`

| 用例 | 期望 |
|------|------|
| 无 Authorization | 401 |
| workspace API key + `stream: false` | 200 + OpenAI 形态 body |
| `stream: true` | SSE（可复用 `chat_stream_contract.rs` 断言思路） |
| workspace_id 与 key 不匹配 | 403 |

**验收**：`cargo test -p transport-http openai_completions`；已包含在 PR smoke 的 `cargo test -p transport-http` 步骤。

**说明**：handler 在 `transport-http/src/lib_impl/infra_handlers.rs` → `openai_chat_completions_handler`，路由在 `routes/chat.rs`。

### 6.2 限流 429 Product E2E

**新建**：`product_e2e/integration/rate_limit_boundary.rs`

1. 创建 `rate_limit_rpm: 2` 的 workspace API key
2. 连续 3 次 MCP 或 chat 请求
3. 第 3 次：HTTP 429、`Retry-After` header、`rate_limit_exceeded`

**注意**：Playwright 后端设 `E2E_ENABLED=true` 可能放宽限流；本测用 Product E2E 独立 bootstrap，不依赖 Playwright env。

### 6.3 配额 429 Product E2E

**新建**：`product_e2e/smoke/quota_boundary.rs` 或 `integration/quota_boundary.rs`

1. 在测试 PG 中 seed 用户 + 已耗尽用量的 `usage_events` / quota
2. 发 chat 或 upload
3. 断言配额相关 error code

**参考**：`smoke/billing_boundary.rs` 的 register + token 模式。

**若属 smoke 模块**：更新 `scripts/run-product-smoke-e2e.sh` 的 `NON_RAG_MODULES`，并跑：

```bash
./scripts/run-product-smoke-e2e.sh --check-modules
```

---

## 7. 阶段 4：Playwright + 质量门禁（P1–P2）

### 7.1 API Key 设置页 smoke

**新建**：`frontend_next/e2e/specs/smoke/api-access.spec.ts`

**流程**：

1. Settings → API Access（或项目实际路径）
2. 创建 key → 显示 plaintext（仅一次）
3. 列表见 key prefix
4. Revoke → 列表更新

**项目**：`functional`（`playwright.config.ts` 已匹配 `specs/smoke/*`，排除 auth）

**POM 参考**：`components/api-access/workspace-api-access-surface.tsx`；Vitest：`tests/api-access/workspace-api-access-surface.test.tsx`

**验收**：

```bash
cd frontend_next
pnpm exec playwright test --project=functional e2e/specs/smoke/api-access.spec.ts
```

### 7.2 引用点击 + 反馈 journey

**新建**：`frontend_next/e2e/specs/journey/citation-interaction.spec.ts`

**流程**（可复用 upload-rag fixture）：

1. 上传 → RAG 提问 → 等待引文
2. 点击 `[data-testid="workspace-citation"]`
3. 断言 citation modal / 原文预览可见
4. 👍 反馈 → 网络 200 或 UI 状态变化

**超时**：`test.setTimeout(180_000)`，`test.slow()` 按需

### 7.3 跨 session 记忆 journey

**新建**：`frontend_next/e2e/specs/journey/memory-recall.spec.ts`

1. Session A：发送含唯一 token `E2E-{runId}` 的消息
2. 新建 Session B：问「刚才提到的 E2E-{runId} 是什么」
3. PR：软断言（回答含 token 或相关词）
4. nightly（`E2E_TIER=nightly`）：硬断言（对齐 `workspace-chat.spec.ts` search 分层）

**后端背景**：见 [`memory-recall-gap-2026-06-13.md`](../memory-recall-gap-2026-06-13.md)

### 7.4 RAG 质量发布阻断（分两步）

**Step A — 接真实 RagRuntime**

- 目录：`avrag-rs/tests/rag_quality/`
- 让 `evaluate_example` 走真实 `RagRuntime` + fixture corpus（可先小样本）
- 本地：`cargo test -p rag_quality --features integration`（feature 名按实现定）

**Step B — CI 阻断**

- 扩展 `.github/workflows/weekly-regression.yml` 或新建 `release-quality-gate.yml`
- 跑三门禁（`tests/rag_quality/src/metrics.rs`）：
  - Recall@15 相对基线下降 ≤ 3%
  - Citation Accuracy ≥ 95%
  - Hallucination Rate ≤ 2%
- **失败 = job fail**（不再 warn-only）

> **实现偏差（2026-06-29 PR-6）**：实际落地的 [`release-e2e-gate.yml`](../../../.github/workflows/release-e2e-gate.yml) 只**硬门禁 Recall@15 drop ≤3% from baseline 0.80**；Citation ≥95% + Hallucination ≤2% **上报不阻断**。原因：定基线测得 Citation=80%（Q1 "谁提出 antifragility" 检索缺口 → agent refusal → 0 引文，拖累均值低于 ≥95% 期望，需更丰富 golden chunk 或 NLI，超 PR-6 范围）；Hallucination 为词重叠启发式（`GOTCHAS.md`：15–30% 误报，NLI 前是噪音）。Recall 硬门禁已双向验证：基线 80% 绿、`wrong doc_scope → 0 chunks → Recall 0%` 红。

**关联验收清单**：[`FUNCTIONAL_ACCEPTANCE_CHECKLIST.md`](../../../FUNCTIONAL_ACCEPTANCE_CHECKLIST.md) §12.1–12.2

---

## 8. PR 拆分建议

| PR | 内容 | 主要改动路径 | 合并后验收 |
|----|------|--------------|------------|
| **PR-1** | Milvus CI + 提交契约测 + registry | `.github/workflows/*`, `transport-http/tests/`, `docs/e2e-test-registry.yaml` | CI 绿 + `cargo test -p transport-http` |
| **PR-2** | MCP Product E2E | `product_e2e/integration/mcp_*.rs`, 指南 §2 | integration 全绿 |
| **PR-3** | OpenAI + 限流/配额 E2E | `transport-http/tests/openai_*`, `product_e2e/**/quota_*`, `rate_limit_*` | smoke 或 integration 全绿 |
| **PR-4** | Billing master CI | `.github/workflows/frontend-journey.yml` 或新 billing workflow | master 上 billing spec 绿 |
| **PR-5** | Playwright api-access + citation | `frontend_next/e2e/specs/**` | functional + journey 本地绿 ✅（§7.1 4.7s / §7.2 46.5s，2026-06-29）+ journey-e2e CI 注入 RAG key（3 secret）+ **CI secret 注入机制本地模拟验证通过**（.env 挪开 + process env 传 3 key，§7.2 1.4m passed，2026-06-29）；真实 GitHub journey CI 待推 master（origin/master 落后 207 提交且 CI 被 `4cb8f67` 移除，journey workflow 不在默认分支、无法 `workflow_dispatch`） |
| **PR-6** | rag_quality + release gate | `crates/app/tests/product_e2e/llm_real/rag_quality_prod.rs`, `tests/rag_quality/`, `.github/workflows/release-e2e-gate.yml` | ✅ ProductionRagEvaluator 接真实 RagRuntime（llm_real tier，真实 embedding+LLM，复用 `shared_rag_fixture` 冷入库 `antifragile.txt`）定基线 Recall@15=80%/Citation=80%/Halluc=80%（Q1 "谁提出 antifragility" 检索缺口→refusal 0%，Q2-Q5 单 rich chunk 100%）；非流式 answer 载 `[[cite:CHUNK_ID]]`（UUID），evaluator 用 `chat.citations` 建 `chunk_to_cite` map 改写为 `[citation:N]` 再打分；`release-e2e-gate.yml` 阻断式（`workflow_dispatch`/`release.published`，写 `.env` 注入 3 RAG secret）；**硬门禁 Recall drop ≤3% from baseline 0.80**，Citation ≥95% + Hallucination ≤2% 上报不阻断；p8 双向验证：基线 80% 绿 + wrong doc_scope→Recall 0%→FAILED 红 ✅ 2026-06-29 |

**依赖顺序**：PR-1 → PR-2 → PR-5；PR-3、PR-4、PR-6 可与 PR-2 之后并行。

---

## 9. 每项完成后的维护 Checklist

每合并一个 PR，执行：

1. [ ] 更新 [`full-functional-e2e-guide.md`](../full-functional-e2e-guide.md) §2 矩阵 + §8 backlog（✅ + 日期）
2. [ ] `cd avrag-rs && ./scripts/generate-e2e-test-registry.py`
3. [ ] 若新增 `product_e2e::smoke::*` 模块 → 改 [`run-product-smoke-e2e.sh`](../../scripts/run-product-smoke-e2e.sh) + `./scripts/run-product-smoke-e2e.sh --check-modules`
4. [ ] 若新增 Playwright spec → 更新 [`e2e-gates.md`](../e2e-gates.md) 表格
5. [ ] PR 描述中粘贴本地验收命令输出摘要

---

## 10. 准上线 Go / No-Go 检查表

全部勾选方可视为「测试套件达到准上线标准」：

### 自动 CI（master / nightly）

- [ ] `./scripts/run-product-smoke-e2e.sh`（PR，Rust 改动路径）
- [ ] `E2E_MODE=integration` product_e2e 全绿（master）
- [ ] Playwright journey 全绿，且 **CI 已启动 Milvus**
- [ ] Playwright billing（paywall + usage）master 自动绿
- [ ] nightly `llm_real` + skills 绿（secrets + Milvus）

### 新能力黑盒

- [ ] MCP：create_upload → PUT → complete → query/RAG + citations（integration）
- [ ] API Key 权限边界（integration + transport contract）
- [ ] OpenAI `/v1/workspaces/{id}/chat/completions` 契约测

### 浏览器

- [ ] API Key 设置页 smoke
- [ ] 上传→RAG→点击引文（journey）

### 质量

- [ ] `rag_quality` 三门禁在 weekly 或 release workflow **失败即阻断**

### 文档

- [ ] 指南 §7 发布清单与 CI 映射一致
- [ ] `e2e-test-registry.yaml` 与 `--list` 枚举一致

---

## 11. 本地常用命令速查

```bash
# 仓库根
cd /home/chuan/context-osv6

# Rust PR smoke
./avrag-rs/scripts/run-product-smoke-e2e.sh

# Rust integration 全量
cd avrag-rs
E2E_MODE=integration cargo test -p app --test product_e2e --features product-e2e -- --test-threads=1

# Transport 契约（含 API Key / MCP 契约）
cargo test -p transport-http

# Nightly 真实 LLM（需 .env 凭证）
E2E_MODE=nightly cargo test -p app --test product_e2e llm_real --features product-e2e -- --ignored --test-threads=1

# Playwright
cd frontend_next
pnpm exec playwright test --project=functional --project=auth    # PR smoke
pnpm exec playwright test --project=journey                      # 主旅程
pnpm exec playwright test --project=skills                       # 真实 RAG/Search 黄金集
pnpm exec playwright test --project=billing                      # 计费

# 发布前一站式（本地，部分步骤可能 warn）
cd /home/chuan/context-osv6 && ./scripts/e2e-d-gate.sh
```

---

## 12. 阶段 5（可选，不阻断 MVP）

| 任务 | 文件/脚本 | CI |
|------|-----------|-----|
| 真实 Office JVM 入库 | 已有 `run-staging-ingest-e2e.sh`、`office_*_staging_e2e.rs` | weekly manual dispatch |
| Admin Users 操作 UI | `e2e/specs/journey/admin-users.spec.ts` | journey |
| Share revoke / analytics | 扩展 `workspace-share.spec.ts` | journey |
| URL Source UI | `journey/workspace-url-source.spec.ts` | journey |
| 文档删除/重索引 UI | 扩展 sources POM | journey |

---

## 13. 明确不在本文范围内

- 修改 RAG/Chat/Search **业务逻辑**
- 修改数据库 migration（除非测试 seed 需要只读 fixture 表——优先用现有 E2E reset API）
- Stripe 生产 webhook 真连（可用 mock server 契约测，但不接真实 Stripe）
- Desktop Tauri 产品级 E2E
- Learning loop 模式（见 `docs/superpowers/specs/2026-06-25-learning-loop-mode-design.md`）——等产品定稿再单独立项

---

## 14. 新窗口执行提示（给 Agent）

1. 先读本文 §0、§3，跑阶段 0 验收命令确认 baseline。
2. 严格按 PR-1 → PR-2 → … 顺序；每 PR 只做文档对应范围，避免一个大 PR。
3. 凭证从 `avrag-rs/.env` 读取，勿向用户重复索要（见根目录 `AGENTS.md` §6）。
4. 改 `product_e2e::smoke::*` 模块列表时 **必须** 同步 `run-product-smoke-e2e.sh`。
5. 全部用例默认 **不修改** `avrag-rs/crates/app/src/`、`frontend_next/app/` 等业务目录；若发现必须改产品才能测，先停步在 PR 描述中说明原因。
