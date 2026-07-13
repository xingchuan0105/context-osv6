# L3 测试整合与标准灌库复用计划

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-10 |
| 状态 | **Done** — W0–W4 complete (2026-07-13) |
| 下游 | Journey 实跑失败修复：[`JOURNEY_INGEST_RLS_AND_E2E_FIX_PLAN_2026-07-13.md`](./JOURNEY_INGEST_RLS_AND_E2E_FIX_PLAN_2026-07-13.md) |
| 触发 | 全套金字塔 DR2 跑批：L3 journey/LLM 假红 + 多入口重复测同一路径；用户要求**修复 + 按业务场景整合**，且 **ingestion 冷灌库尽量复用** |
| 约束 | Solo local trunk；DR2 只含 L3-thin；不把 quality/skills 塞进日常准部署；UI 可 REUSE dev 栈，Rust quality 禁外部 worker |
| 上游 | [`ACCEPTANCE_PYRAMID_STABILIZATION_PLAN_2026-07-10.md`](./ACCEPTANCE_PYRAMID_STABILIZATION_PLAN_2026-07-10.md)、[`TEST_PYRAMID_DEDUP_MAP.md`](./TEST_PYRAMID_DEDUP_MAP.md)、[`avrag-rs/docs/e2e-gates.md`](../../avrag-rs/docs/e2e-gates.md)、[`SOLO_DISCIPLINE.md`](./SOLO_DISCIPLINE.md) |
| 关联诊断 | L3 journey：旧 `avrag-api` vs 已无 `org_id` 的 DB → login 500 伪装 register 409；L3-llm：`scan_data` 假阳性、quality preflight 与 dev worker 冲突、`test-l3-llm.sh` 过宽 |

---

## 0. 已拍板决策（2026-07-10）

| # | 议题 | 决定 |
|---|------|------|
| 1 | 目标优先级 | **故障修复 + 业务场景去重** 同一波次 |
| 2 | 主路径权威 | **L2 mock 协议 1 条 + L3-llm 真模型 1 条**；Playwright 不抢协议硬门 |
| 3 | DR2 默认 L3 | **仅 L3-thin**：UI smoke + 四模式真 LLM 各 1 |
| 4 | Worker 隔离 | **分层**：PW 可 `PLAYWRIGHT_REUSE_SERVER`；Rust quality / smoke_v5 **禁**外部 `avrag-worker` |
| 5 | 入口命名 | **A**：拆 `test-l3-ui-smoke.sh` / 真 `test-l3-journey.sh` / `test-l3-llm.sh` / `test-l3-quality.sh` |
| 6 | 四模式范围 | thin = chat / rag / search / write 各 1；multi_turn + format = `L3_LLM_EXT=1` |
| 7 | **标准灌库（用户重申）** | real 路径核心重叠在 **ingestion**；PW upload 与四 agent Q&A **共用一份标准文档**；冷灌库尽量一次、查询复用 |

---

## 1. 问题陈述

### 1.1 入口语义崩坏

| 入口（重构前） | 文档声称 | 实际 |
|----------------|----------|------|
| `test-l3-journey.sh` | L3 UI 旅程 | 默认跑 Playwright **smoke** |
| `test-l3-llm.sh` | 四模式 thin | filter `llm_real` → **整包**含 quality / smoke_v5 |
| `test-dr2.sh` L3 段 | 准部署 thin | 被过重 L3 + 环境假红拖垮 |

### 1.2 业务路径重复

同一故事被 L2 mock smoke、`llm_real`、PW smoke、PW journey、PW skills **多处硬门**（尤以「上传→RAG→引用」「search web cite」为甚）。  
原则违反：[`TEST_PYRAMID_DEDUP_MAP`](./TEST_PYRAMID_DEDUP_MAP.md)「同一断言只在最低足够层」。

### 1.3 灌库成本重复（用户焦点）

| 现象 | 后果 |
|------|------|
| `rag_real` / `multi_turn` / `format_real` 各自 `upload + wait_for_ingestion` | 重复真 embedding/ingest 分钟级成本 |
| PW journey 用 `sample-document.txt`，Rust 用 `antifragile.txt` | 查询/golden 无法对齐 |
| quality 语料与 thin 混跑 | DR2 假红、预检与 dev worker 互斥 |

### 1.4 现场假红（已诊断，部分已修）

1. **Journey/smoke globalSetup**：API 二进制早于 org 删除，login SQL 仍查 `org_id` → 500 → register 409。  
2. **format_real**：`doc_scan`/`scan_data` 被当 blocking degrade。  
3. **rag_quality_prod / smoke_v5**：`assert_no_external_workers` 与 product-dev-up 冲突。

---

## 2. 目标

1. **L3 分层清晰**：thin-ui / thin-llm / journey / quality 四入口，DR2 只串 thin。  
2. **业务 sole owner**：每条主路径 L2 mock + L3-llm 硬门各 ≤1；UI 最多 1 条旅程硬门。  
3. **标准文档策略**：一份 `antifragile.txt`，跨 Rust thin 与 PW journey 对齐；**单进程冷灌库复用**。  
4. **可定位失败**：5xx login 不伪装 409；quality 不进 thin。  
5. **文档三方一致**：gates / dedup map / 脚本。

### 2.1 非目标

- 删除 quality 语料或 golden set。  
- Playwright 测 SSE 序 / ToolCatalog。  
- 每 commit 跑 L3。  
- 放宽 quality 的 external-worker preflight。  
- 跨 **cargo 进程** 复用灌库（OnceCell 仅同 binary；thin 脚本用 **单次** `cargo test` + `--skip` 保证同进程）。

---

## 3. 标准文档与灌库复用（核心设计）

```text
                    antifragile.txt  (标准产品 fixture)
                    字节一致：
                    · product_e2e/fixtures/antifragile.txt
                    · frontend_next/e2e/fixtures/antifragile.txt
                              │
          ┌───────────────────┼───────────────────┐
          ▼                   ▼                   ▼
   L2 mock smoke        L3-thin-llm           L3-journey (PW)
   (可另起 mock 灌库)    standard_doc.rs        upload 同文件
                        OnceCell 冷一次          问题对齐 antifragility
                        rag/format/multi_turn
                        复用 document_id
```

| 规则 | 说明 |
|------|------|
| **文件名** | `STANDARD_DOC_FIXTURE = "antifragile.txt"` |
| **Rust API** | `fixtures/standard_doc.rs` → `shared_standard_doc_real_llm()` |
| **谁必须复用** | 依赖 doc_scope 的 thin/ext：`rag_real`（citation 条）、`multi_turn`、`format_real` |
| **谁不灌库** | `chat_real`、`search_real`（open web）、`write_real` |
| **谁独立语料** | `rag_quality_prod` / smoke_v5 / realistic / PDF corpus（L3-full） |
| **PW** | journey `workspace-upload-rag` 使用同一 `antifragile.txt` |
| **单进程** | `test-l3-llm.sh` 一次 `cargo test llm_real` + `--skip`，保证 OnceCell 生效 |

---

## 4. 层与 sole owner 矩阵

| 业务场景 | L2 mock 硬门 | L3-thin-llm 硬门 | L3-thin-ui | L3-journey | L3-full |
|----------|--------------|------------------|------------|------------|---------|
| Auth 形状 | `auth_boundary` | — | smoke 登录 UI | — | — |
| Upload→completed | `ingestion_smoke` | （标准 doc 冷路径内含） | — | wait UI 状态 | — |
| Chat 非空 | `chat_smoke` | `chat_real`×1 | — | 可选点聊 | — |
| RAG + 引用 | `rag_smoke` | `rag_real` citation×1 | — | upload-rag×1 | quality |
| Search web | `search_smoke` | `search_real`×1 | — | soft | — |
| Write | `write_smoke` | `write_real`×1 | — | write UI | — |
| Format | integration/mock | `format_real`（EXT） | — | skills soft | — |
| Multi-turn | memory smoke | `multi_turn`（EXT） | — | session UI | — |
| Recall@15 | — | — | — | — | `rag_quality_prod` |

---

## 5. 入口契约

| 脚本 | 层 | DR2 默认 | 命令要点 |
|------|-----|----------|----------|
| `scripts/test-l3-ui-smoke.sh` | L3-thin-ui | **是** | PW `e2e/specs/smoke` |
| `scripts/test-l3-llm.sh` | L3-thin-llm | **是** | 四模式；`--skip` quality/pdf/complex/multi_turn/format |
| `L3_LLM_EXT=1 test-l3-llm.sh` | extended | 否 | + multi_turn + format（仍同标准 doc） |
| `L3_LLM_FULL=1 test-l3-llm.sh` | 整包 | 否 | 全 `llm_real`（须停 external worker 若跑 quality） |
| `scripts/test-l3-journey.sh` | L3-journey | 否 | PW `--project=journey`；`JOURNEY=0` 可委派 smoke |
| `scripts/test-l3-quality.sh` | L3-full | 否 | `rag_quality_prod` only |
| `scripts/test-dr2.sh` | DR2 | — | L1→L2-core→L2-patho→**ui-smoke→thin-llm** |

---

## 6. 实施波次与状态

### W0 — 语言与契约 — **Done**

- [x] 拍板表（§0）  
- [x] 更新 [`TEST_PYRAMID_DEDUP_MAP.md`](./TEST_PYRAMID_DEDUP_MAP.md) 标准 doc + 入口表  
- [x] 更新 [`avrag-rs/docs/e2e-gates.md`](../../avrag-rs/docs/e2e-gates.md) L3 / DR2–DR3  
- [x] 本计划文档  

### W1 — 阻塞修复 — **Done**

- [x] `ensureTestUserAccount`：login 5xx fail-fast；409 后 retry login  
- [x] `non_blocking_degrade`（`doc_scan`/`scan_data` + 空 MM embedding）  
- [x] format / rag / multi_turn 使用非阻塞 degrade 过滤  

### W2 — 入口整合 — **Done**

- [x] `test-l3-ui-smoke.sh`  
- [x] `test-l3-llm.sh` thin / EXT / FULL  
- [x] `test-l3-journey.sh` 真 journey  
- [x] `test-l3-quality.sh`  
- [x] `test-dr2.sh` 接 thin-ui + thin-llm  

### W3 — 标准灌库复用 — **Done（核心）**

- [x] `fixtures/standard_doc.rs` + `shared_standard_doc_real_llm()`  
- [x] `rag_real` / `multi_turn` / `format_real` 改用共享 fixture  
- [x] journey `workspace-upload-rag` 改用 `antifragile.txt` + 对齐问法  
- [x] skills `rag-available` / `search-available`：citation **soft**（`SKILLS_HARD_CITATION=1` 可恢复硬门）  

### W4 — 收尾加固 — **Done 2026-07-13**

- [x] `scripts/dev-stack-check.sh`：login 探测；5xx → rebuild API 提示；接入 `test-l3-ui-smoke.sh`  
- [x] skills 与 journey 去重（UI RAG 硬门 sole owner = journey + L2/L3-llm）  
- [x] journey upload 类 spec 清单评审（见 §7.1）  
- [x] 栈修复：rebuild API/worker；drop 不兼容 `avrag_*` Milvus collections（缺 `owner_user_id`）；`dev-stack-check` login **200**  
- [x] 本计划状态 → Done  

---

## 7. 关键文件清单（已落地）

| 区域 | 路径 |
|------|------|
| 标准 doc | `avrag-rs/crates/app/tests/product_e2e/fixtures/standard_doc.rs` |
| 消费方 | `llm_real/{rag_real,multi_turn,format_real}.rs`，`llm_real/mod.rs`（`non_blocking_degrade`） |
| Auth | `frontend_next/e2e/utils/api-helpers.ts` |
| PW journey | `frontend_next/e2e/specs/journey/workspace-upload-rag.spec.ts` |
| PW skills | `skills/rag-available.spec.ts`，`skills/search-available.spec.ts`（soft citation） |
| 脚本 | `scripts/test-l3-{ui-smoke,llm,journey,quality}.sh`，`scripts/test-dr2.sh`，`scripts/dev-stack-check.sh` |
| 文档 | `TEST_PYRAMID_DEDUP_MAP.md`，`avrag-rs/docs/e2e-gates.md`，**本文件** |

### 7.1 Journey upload fixture 清单（评审结论）

| Spec | Fixture | 是否强制标准 doc | 理由 |
|------|---------|------------------|------|
| `workspace-upload-rag.spec.ts` | **antifragile.txt** | **是** | 与 llm_real / golden 对齐；DR3 UI RAG 硬门 |
| `citation-interaction.spec.ts` | sample-document.txt | 否 | 测 citation chip / feedback UI；问题绑定「技术栈」段落 |
| `analyze-workflow.spec.ts` | sample-document.txt | 否 | analyze 工作流，非 antifragility Q&A |
| `workspace-upload-pdf-rag.spec.ts` | phase0-mini.pdf | 否 | PDF/LiteParse 路径，独立语料 |
| skills `rag-available` | antifragile.txt | 同文件 soft | 与标准 doc 对齐；citation soft |

**原则**：标准 doc 用于「产品主路径 RAG 语义」；UI 交互/PDF/analyze 可保留专用 fixture。

---

## 8. 成功标准（DoD）

| ID | 标准 | 状态 |
|----|------|------|
| D1 | DR2 默认不跑 `rag_quality_prod` / smoke_v5 | Done（脚本） |
| D2 | 四模式 L2 + L3-llm 各 1 硬门，表可核对 | Done（文档+thin filter） |
| D3 | RAG UI 硬门 ≤1 条 journey；标准 doc 对齐 | Done（skills citation soft） |
| D4 | format 不因 `scan_data` 假红 | Done |
| D5 | login 5xx 明确失败，不伪装 409 | Done（代码） |
| D6 | thin llm **同进程** 冷灌库一次，多查询复用 | Done（OnceCell + 单 cargo） |
| D7 | gates / dedup / 脚本一致 | Done |
| D8 | 新 API 下 login 通或 preflight 指明 skew | Done（2026-07-13：`dev-stack-check` login 200；`test-l3-ui-smoke` **18 passed**） |

---

## 9. 操作手册（给人类 / agent）

```bash
# --- DR2 L3-thin ---
bash scripts/test-l3-ui-smoke.sh
bash scripts/test-l3-llm.sh

# 扩展（同标准 doc）
L3_LLM_EXT=1 bash scripts/test-l3-llm.sh

# Journey / quality（非 DR2）
bash scripts/test-l3-journey.sh
# quality 前停 dev worker
bash scripts/test-l3-quality.sh

# 全阶梯
bash scripts/test-dr2.sh
REQUIRE_L3=1 bash scripts/test-dr2.sh
```

**环境注意**

1. org 删除后必须 **重编并重启** `avrag-api` / `avrag-worker`。  
2. Milvus 旧 collection 若缺 `owner_user_id`，API 启动失败：drop 前缀 `avrag_` 的 collection 后重启（e2e 隔离前缀可保留）。  
3. `PLAYWRIGHT_REUSE_SERVER=1` 复用的是**进程内二进制**，不是源码树。先跑 `bash scripts/dev-stack-check.sh`。  
4. quality 与 product-dev-up worker **互斥**（设计如此）。

---

## 10. 风险

| 风险 | 缓解 |
|------|------|
| 单 cargo 仍慢（四模式真 LLM） | thin 默认仅 4 条；EXT/FULL 显式 |
| OnceCell 跨进程无效 | 脚本禁止拆成多次 cargo 跑 thin |
| PW 每 spec 仍独立 upload | 可接受（浏览器会话隔离）；文件与问法与 Rust 对齐即可 |
| skills 仍硬门 | W4 降级 |

---

## 11. 一句话

L3 从「脚本名像旅程、实际一锅粥」改为 **thin-ui / thin-llm / journey / quality 四入口**；业务硬门按层 sole owner；**一份 `antifragile.txt` 标准灌库，Rust thin 同进程复用、PW journey 同文件对齐**——既修假红，又降重复成本。
