# 验收金字塔稳定化计划（准部署 · 可分层定位）

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-10 |
| 状态 | **Done** — W0–W4 complete (2026-07-10) |
| 触发 | 现场 ingestion 病理 bug 未被默认金字塔识别；用户要求**泛化**为可稳定验收、准部署、可分层快速定位 |
| 约束 | Solo local trunk；**不**把全栈 E2E 塞进每 commit；不扩 GitHub CI 剧场为默认环 |
| 上游 | [`TN3_P0_P5_AND_TEST_PYRAMID_PLAN_2026-07-09.md`](./TN3_P0_P5_AND_TEST_PYRAMID_PLAN_2026-07-09.md)、[`TEST_PYRAMID_INVENTORY_2026-07-09.md`](./TEST_PYRAMID_INVENTORY_2026-07-09.md)、[`avrag-rs/docs/e2e-gates.md`](../../avrag-rs/docs/e2e-gates.md)、[`SOLO_DISCIPLINE.md`](./SOLO_DISCIPLINE.md)、[`INGESTION_PDF_STUCK_DIAGNOSIS_2026-07-10.md`](./INGESTION_PDF_STUCK_DIAGNOSIS_2026-07-10.md) |
| 资产 | 入口 `scripts/test-l{1,2,3}-*.sh`；`e2e-test-registry.yaml`（CAP-*）；product_e2e smoke/integration |
| 下游 L3 | [`L3_TEST_INTEGRATION_AND_CORPUS_PLAN_2026-07-10.md`](./L3_TEST_INTEGRATION_AND_CORPUS_PLAN_2026-07-10.md)（thin/journey/quality 拆分 + 标准 doc 灌库复用） |

---

## 0. 问题陈述（从个案到系统）

### 0.1 现场教训（ingestion）

| 真实失败模式 | 默认金字塔为何 miss |
|--------------|---------------------|
| 千级 micro-block × cl100k 重建 → 卡死 | L2 happy 用小文本；无 **Scale** 刺激 |
| lock miss → 假成功 / 假 completed | 无 **终态完整性** 断言与并发/锁路径 |
| 真 LLM 30s 超时 / 大 prompt | mock 绕开；真路径 L3 ignore / 低频 |
| 长任务 processing 空转 | smoke 60s 小样本；无 **阶段 SLA** |
| 病理 PDF IR 形态 | 真 PDF 多 `#[ignore]` / 手工 |

根因不是「没有 E2E」，而是：

1. **目的混装**：L2 同时承载「协议通」与「病理对」却只写了前者。  
2. **刺激面不足**：fixture 只代表 happy 分布。  
3. **失败无定位协议**：红了只知道「链路挂」，不知该下沉哪一层。  
4. **准部署未定义**：不知道哪一层绿才算「可上预发/可上生产」。  
5. **编号两套**：金字塔 L1–L3 与 registry 的 L1–L6（TEAF）并存，心智摩擦。

### 0.2 目标（本计划）

1. **准部署阶梯（DR0–DR3）**：每一档有可重复命令 + 明确能力门槛。  
2. **三轴金字塔**：频率 × 失败信号类型 × 能力域（CAP），不单靠「层号」。  
3. **病理类（Patho classes）泛化**：从 ingestion 抽象出跨域可复用刺激面。  
4. **分层定位协议（triage）**：失败时 5–15 分钟内指到 crate/模块。  
5. **入口不膨胀**：仍 ≤ 正式入口脚本族；病理进 **L2-patho 子套**，不进 L1。

### 0.3 非目标

- 每 commit 跑真 PDF / 真 LLM / 全 Playwright journey  
- 恢复 PR 必跑 smoke 作为 Solo 默认（可保留 workflow_dispatch）  
- 一次把所有 CAP 补到 DR3（按风险排序波次）  
- 推倒现有 `product_e2e` 重写  

---

## 1. 三轴模型（统一语言）

```text
              ┌──────────────── 频率 / 预算 ────────────────┐
              │  L1 每次提交   L2 机制/波次   L3 准部署/发版  │
              └─────────────────────────────────────────────┘
                                    ×
              ┌──────────────── 失败信号类型 ───────────────┐
              │  S0 编译契约  S1 机制  S2 协议/HTTP  S3 旅程  │
              │  S4 病理/SLA  S5 真依赖  S6 质量/性能         │
              └─────────────────────────────────────────────┘
                                    ×
              ┌──────────────── 能力域 CAP ─────────────────┐
              │  INGEST RAG CHAT SEARCH WRITE AUTH …        │
              └─────────────────────────────────────────────┘
```

### 1.1 层（频率）— 保持现入口

| 层 | 命令 | 预算 | 默认触发 |
|----|------|------|----------|
| **L1** | `scripts/test-l1.sh` | ≤ 5–15 min | 每 commit / 改相关 crate |
| **L2-core** | `scripts/test-l2-mechanisms.sh` | ≤ 20 min | 改机制 / 波次末 |
| **L2-int** | `scripts/test-l2-integration.sh` | ≤ 40 min 或 core/edge | 波次末 |
| **L2-patho** | `scripts/test-l2-patho.sh`（**新增**） | ≤ 15 min | 改高风险机制 / 波次末 / 准部署 |
| **L3-thin** | `scripts/test-l3-journey.sh` + `test-l3-llm.sh` | ≤ 40–60 min | **准部署** / 波次关闭 |
| **L3-full** | quality / journey full / 大 PDF staging | 无日常预算 | 发版 / weekly |

### 1.2 信号类型 S（失败时先看「像什么」）

| ID | 名称 | 典型失败 | 首选定位层 |
|----|------|----------|------------|
| **S0** | 编译/契约 | type/API/schema 破 | L1 |
| **S1** | 纯机制 | chunker/lock/loop 分支错 | L1 unit → L2 lib |
| **S2** | 协议/HTTP | 状态码、SSE、Product App 面 | L2 mock smoke |
| **S3** | 旅程/UI | 按钮、路由、可见状态 | L3 Playwright |
| **S4** | 病理/SLA/终态 | 超时、假成功、锁竞态、N 大 | **L2-patho** |
| **S5** | 真依赖 | 真 LLM/真 PDF/Paddle | L3-thin / staging |
| **S6** | 质量/性能 | recall、时延回归 | L3-full / nightly |

**铁律**：S4/S5/S6 **不得**作为 L1 必过；L1 只保 S0/S1 的轻量切片。

### 1.3 能力域 CAP（与 registry 对齐）

沿用 `e2e-test-registry.yaml` 的 `CAP-*`。每个 CAP 定义 **最低门禁矩阵**（见 §3）。

**编号对齐（消除双轨）**

| 金字塔 | TEAF registry 旧号 | 含义 |
|--------|-------------------|------|
| L1 | 契约/lib 测 | 单元·契约 |
| L2 | 旧 L1 mock smoke / L2 integration 混用 | **统一叫 L2**；registry 再生时改注释 |
| L3-thin | 旧 L3/L4/L5 薄切 | UI smoke + llm 抽样 |
| L3-full | 旧 L4 skills / quality | 质量与长旅程 |

实施 Wave 中更新 registry `required_layers` 注释，避免 CAP-WRITE 把 mock smoke 标成 L1。

---

## 2. 准部署阶梯（Deploy Readiness）

> **准部署** ≠ 生产 100% 质量；= 「已知风险可控、主路径可演示、失败可定位」。

| 档位 | 名称 | 必绿门槛 | 允许的已知债 | 用途 |
|------|------|----------|--------------|------|
| **DR0** | 可提交 | L1 相关包绿 | 机制未回归 | 日常 commit |
| **DR1** | 机制可信 | DR0 + L2-core 绿 | 真依赖未验 | 内部 demo / 合并机制波 |
| **DR2** | **准部署** | DR1 + L2-patho 绿 + L3-thin 绿 | 大语料/质量未跑 | **预发 / VPS 预上线** |
| **DR3** | 可发版 | DR2 + L3-full（journey 子集 + quality 或等价） | 仅登记的 flaky quarantine | 生产 |

### 2.1 DR2 准部署检查表（机器可执行）

```bash
# 1) 地基
bash scripts/test-l1.sh

# 2) 机制 + mock 产品面（四模式 + ingest + guard）
bash scripts/test-l2-mechanisms.sh

# 3) 病理子套（新增；失败 = 不可准部署）
bash scripts/test-l2-patho.sh

# 4) 薄旅程 + 真 LLM 抽样（本地栈 + .env）
bash scripts/test-l3-journey.sh          # Playwright smoke 短集
bash scripts/test-l3-llm.sh              # 四模式 1–2 条

# 5) 可选：迷你真 PDF（非全书）
# bash avrag-rs/scripts/run-liteparse-staging-e2e.sh
```

退出码非 0 → **未达 DR2**；输出须带 **层标签**（见 §4）。

### 2.2 一页报告（波次末 / 准部署）

生成 `docs/engineering/_reports/dr-status-YYYYMMDD.md`（脚本可后加）：

| CAP | L1 | L2-core | L2-patho | L3-thin | 结论 |
|-----|----|---------|----------|---------|------|
| CAP-INGEST | ✅/– | ✅ | ✅/❌ | ✅/skip | |
| CAP-RAG | … | | | | |
| … | | | | | |

---

## 3. CAP 最低门禁矩阵（泛化验收）

每个 **P0 CAP** 至少具备：

| 列 | 要求 |
|----|------|
| **L1** | 纯逻辑/契约测；无 Docker 全栈 |
| **L2-happy** | mock 黑盒 1 条：上传或 API → 期望终态 |
| **L2-patho** | ≥ 1 条高风险病理（见 §3.1）；无真外部 Key |
| **L3-thin** | UI 或真 LLM 二选一薄切（按 CAP 风险） |

### 3.1 病理类目录（跨 CAP 复用，不限 ingestion）

| 病理类 | 刺激面 | 断言骨架 | 优先 CAP |
|--------|--------|----------|----------|
| **P-Scale** | N 大（blocks/chunks/messages） | 时限内完成；无 O(n²) 退化 | INGEST, MEMORY |
| **P-Terminal** | 空结果 / 零 chunk / 零 body | **禁止假成功终态** | INGEST, WRITE |
| **P-Lock** | 锁占用 / TTL / 双 worker | fail 明确 or 可 re-claim；无死锁 | INGEST, WORKER |
| **P-Timeout** | 注入慢依赖 | 取消/释放资源；可重试 | INGEST, LLM, SEARCH |
| **P-Cancel** | 中途 drop/kill | 无脏租约；可 requeue | WORKER |
| **P-Idempotent** | 重复 complete/upload | 幂等或冲突码 | INGEST, API |
| **P-Authz** | 错 owner / 跨账户 | 403/隔离 | AUTH, SHARE, TENANT |
| **P-Degrade** | mock down | 可预期降级，不 500 风暴 | RAG, SEARCH, DEGRADE |
| **P-Stream** | 断流 / 乱序 | SSE 契约 | STREAM, CHAT |

**落地形态**：优先 **L1 单元**（快）→ 不够再 **L2-patho 集成**（带 PG/worker mock LLM）。

### 3.2 CAP-INGEST 样板（从本次 bug 固化）

| 信号 | 用例（目标） | 层 |
|------|--------------|-----|
| S1 P-Scale | 2000 micro-block chunk plan &lt; 2s | L1 `ingestion` |
| S1 P-Terminal | 无 body/multimodal → 不得 completed | L1 或 L2-patho |
| S1 P-Lock | DocumentLocked / lock miss → 非 Ok 成功路径 | L1 worker/storage |
| S2 happy | `ingestion_smoke` txt + chunk_count&gt;0 | L2-core（已有） |
| S4 scale-lite | 合成高 block IR 或 mini PDF &lt; 180s completed | L2-patho |
| S5 | liteparse staging / 可选 paddle ignore | L3 / staging |

其它 CAP 按同表扩：WRITE 的 P-Terminal（空文假完成）、CHAT 的 P-Stream、AUTH 的 P-Authz 等。

### 3.3 P0 CAP 优先级（准部署）

| 优先级 | CAP | 理由 |
|--------|-----|------|
| P0 | INGEST, RAG, CHAT, WRITE, AUTH | 产品主路径 |
| P1 | SEARCH, SHARE, GUARD, STREAM | 常用周边 |
| P2 | BILLING, MEMORY, FORMAT, DEGRADE | 可后置 |

---

## 4. 分层快速定位协议（Triage）

### 4.1 原则

> **红在哪一层，先信哪一层；再向下沉，不横向乱开全栈。**

```text
现象（用户/预发）
    │
    ▼
复现最短刺激（同 CAP 的 L3-thin 或手工）
    │
    ├─ 仅 UI 坏、API 正常 ──────────────► S3 → 前端/Playwright
    ├─ HTTP/SSE 契约坏 ─────────────────► S2 → L2 smoke 同 CAP
    ├─ mock 通、真依赖挂 ───────────────► S5 → 配置/配额/外部
    ├─ mock 也挂 ───────────────────────► L2-core 同模块
    │         │
    │         ├─ worker 日志阶段卡 ─────► S4/S1 → L2-patho / L1 机制
    │         └─ API 纯逻辑 ───────────► L1 crate
    └─ 慢/假成功/锁 ───────────────────► 强制 L2-patho 类表 §3.1
```

### 4.2 命令定位卡（贴终端）

| 症状 | 第一条命令 | 第二条 |
|------|------------|--------|
| 提交就红 | `test-l1.sh` 或 `cargo test -p &lt;touched&gt; --lib` | 看失败 crate |
| 上传后一直 processing | `test-l2-patho.sh`（INGEST）+ worker log 阶段 | L1 chunker/lock 单测 |
| completed 但无法 RAG | L2 `ingestion_smoke` + `chunk_count` + Milvus 计数 | P-Terminal 单测 |
| 四模式某一模式挂 | L2 该 `*_smoke` | L1 agent-loop/tools |
| 登录/权限 | L2 `auth_boundary` | L1 contracts/auth |
| 真模型怪答案 | L3-llm 抽样；质量问题再 quality | 勿在 L1 找 |
| UI 点不动 API 200 | L3 journey 单 spec | 前端 vitest |

### 4.3 失败输出约定（实施时改 runner）

所有 `test-l*.sh` 在失败时打印：

```text
[PYRAMID] layer=L2-patho cap=CAP-INGEST signal=S4 class=P-Scale
[PYRAMID] next= cargo test -p ingestion --lib scale_
```

便于 agent/人跳过全栈重跑。

---

## 5. 与现有资产映射（不推倒）

| 现有 | 归入 | 调整 |
|------|------|------|
| `test-l1.sh` | DR0 | 可选：touched crates 探测 |
| `run-product-smoke-e2e.sh` / `test-l2-mechanisms.sh` | L2-core / DR1 | 保持；写清 CAP 列表 |
| `test-l2-integration.sh` | L2-int | 标 core vs edge（文档即可） |
| **新** `test-l2-patho.sh` | L2-patho / DR2 | 聚合 patho 模块 filter |
| `test-l3-journey.sh` / `test-l3-llm.sh` | L3-thin / DR2 | 准部署必跑 |
| `paddle_pdf_smoke` ignore | L3-full / staging | 保持 ignore；DR3 可选 |
| `rag_quality_prod` | L3-full | 仅 DR3 |
| registry CAP-* | 矩阵行 | 对齐层名注释 |

---

## 6. 实施波次

### W0 — 语言与入口（0.5–1d） — **Done 2026-07-10**

- [x] `e2e-gates.md`：**DR0–DR3**、**S0–S6**、triage  
- [x] registry 注释：mock smoke ≠ 金字塔 L1；CAP-INGEST patho_filters  
- [x] `scripts/test-l2-patho.sh` + `scripts/test-dr2.sh`  
- [x] SOLO_DISCIPLINE：准部署 DR2  

**验收**：`bash scripts/test-l2-patho.sh` / `bash scripts/test-dr2.sh` 可发现。

### W1 — L2-patho 首批 — CAP-INGEST 样板 — **Done 2026-07-10**

- [x] L1 `patho_scale_micro_blocks_chunk_plan_under_budget`（1500 blocks &lt; 10s）  
- [x] L1 `patho_terminal_refuses_completed_without_content` + worker 调用 `ensure_ingest_content_for_completed`  
- [x] L1 `patho_lock_*`（class + WorkerRuntime requeue 无 Completed）  
- [x] `test-l2-patho.sh` 执行 `cargo test -p ingestion --lib patho_`（4 passed）  
- [ ] L2-patho 迷你 PDF 高 block 端到端（可选，W2 可补）  

**验收**：`cargo test -p ingestion --lib patho_` 4 passed &lt; 15s。

### W2 — CAP 矩阵补齐 P0 — **Done 2026-07-10**

| CAP | patho 用例 | 包 |
|-----|------------|-----|
| INGEST | scale / lock / terminal（W1） | `ingestion` |
| WRITE | `patho_terminal_empty_write_topic` + unmet bands finish | `write-core`, `app-chat` |
| AUTH | `patho_authz_*` workspace/account scope | `transport-http` |
| STREAM | `patho_stream_*` SSE start/done | `transport-http` |
| CHAT | `patho_chat_budget_exhausted_*` | `agent-loop` |
| RAG | `patho_rag_cross_owner_*` | `avrag-rag-core` |

- [x] `scripts/test-l2-patho.sh` 串联上述 filter  
- [x] registry `patho_filters` 登记  

**验收**：`bash scripts/test-l2-patho.sh` 全绿。

### W3 — 准部署剧本固化 — **Done 2026-07-10**

- [x] `scripts/test-dr2.sh`：L1 → L2-core → L2-patho → L3-thin  
- [x] `REQUIRE_L3` / `SKIP_L3` / `SKIP_L2_CORE`；无 LLM key 自动 skip L3-llm  
- [x] 失败统一 `[PYRAMID] FAIL layer=… signal=… next=…`（`pyramid-lib.sh`）  
- [x] 报告 `docs/engineering/_reports/dr2-latest.md`  
- [x] 各 `test-l*.sh` 接入 pyramid 标签  

**验收**：`SKIP_L2_CORE=1 SKIP_L3=1 bash scripts/test-dr2.sh` → PARTIAL + report；全量 `bash scripts/test-dr2.sh`。

### W4 — 定位与可观测 — **Done 2026-07-10**

- [x] ingestion pipeline / lock / route / terminal：统一 `stage=` 字段  
- [x] `scripts/ingest-doc-dump.sh <document_uuid>`：status / tasks / chunks / FALSE_COMPLETED  
- [x] `scripts/pyramid-triage.sh`：失败文本 → `next=` 命令  
- [x] e2e-gates / SOLO 引用  

**验收**：`echo document_locked | bash scripts/pyramid-triage.sh`；`bash scripts/ingest-doc-dump.sh <uuid>`。

---

## 7. 成功标准（计划级 DoD）

| ID | 标准 |
|----|------|
| D1 | DR0–DR3 定义写入 gates + SOLO，与脚本入口一致 | **Done** |
| D2 | `test-l2-patho.sh` 存在且 CAP-INGEST 至少 3 条病理（L1 或 L2） | **Done** |
| D3 | 准部署（DR2）一条龙脚本；无 key 时明确 skip L3 而非假绿 | **Done** |
| D4 | 任意 P0 CAP 故障可用 §4 表 15 分钟内指到层/CAP | **Done**（`[PYRAMID] next=`） |
| D5 | 再出现「仅大样本/锁/假完成」类 bug 时，**优先红在 L1/L2-patho** | **Done**（patho filter） |

---

## 8. 风险与纪律

| 风险 | 缓解 |
|------|------|
| patho 变慢挤进日常 | **禁止** 进 L1；L2-patho 单独预算 |
| 又变成一锅粥 | DR 脚本串行有序；禁止新第 6 套无关入口 |
| 假绿（skip 当 pass） | skip 必须打印 `DR2_PARTIAL` 且非 0 若 `--require-l3` |
| registry 双编号 | W0 注释统一；再生脚本时改 required_layers 语义 |

---

## 9. 建议拍板（请产品确认）

| # | 选项 | 建议 |
|---|------|------|
| 1 | 准部署默认档 | **DR2**（含 L2-patho + L3-thin） |
| 2 | 日常 commit | **仅 DR0（L1）** — 不变 |
| 3 | patho 首批 | **CAP-INGEST 全套 + 其它 P0 各 1** |
| 4 | 真大 PDF | 仍 staging/ignore；**不进 DR2 必过** |
| 5 | 执行 | 先 W0+W1（本周可完成样板），再 W2–W3 |

---

## 10. 下一跳（若批准）

1. 确认 §9 拍板。  
2. 开 W0（文档+脚本骨架）+ W1（INGEST patho 落地）。  
3. 波次末用 `test-dr2.sh` 跑一轮，填 DR 报告表。  

**一句话**：金字塔从「有没有测」升级为「**哪一档算准部署、哪一类失败归哪一层、病理有稳定刺激面**」——用三轴 + DR 阶梯 + patho 类目录，而不是无限加 E2E。
