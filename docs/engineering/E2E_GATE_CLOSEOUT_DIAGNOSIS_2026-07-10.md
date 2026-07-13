# 门禁收口诊断（L2 官方 + full integration + L3 smoke）

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-10 |
| 原则 | **只跑只记，不自动修复** |
| 基线 | F0–F6 修复后 `e9df53c` / `c59b391` 一带 |
| 日志 | `/tmp/context-osv6-gate-closeout-2026-07-10/` |
| 墙钟 | ~13:43–14:02 CST（约 18 分钟） |

**本轮范围（建议顺序，已全部执行）：**

1. `bash scripts/test-l2-mechanisms.sh`（官方 L2：lib + product smoke runner）  
2. `bash scripts/test-l2-integration.sh`（全量 mock `product_e2e`，`E2E_MODE=integration`）  
3. `bash scripts/test-l3-journey.sh`（短 Playwright smoke，本地 `PLAYWRIGHT_REUSE_SERVER=1`）

**本轮未跑：** real LLM（`llm_real` / `--ignored`）、`JOURNEY=1` 全 journey、rag_quality、staging paddle PDF。

---

## 1. 总览

| 套件 | Exit | 结果 | 墙钟（约） |
|------|------|------|------------|
| **L2_MECH** | **0** | **PASS** — `L2 mechanisms OK` | ~6.6 min |
| **L2_INTEGRATION** | **0** | **PASS** — `90 passed; 0 failed; 24 ignored` | ~11.6 min |
| **L3_SMOKE** | **0** | **PASS** — `21 passed`（57s） | ~1 min |

```text
L2_MECH=0
L2_INTEGRATION=0
L3_SMOKE=0
```

**结论：mock 门禁收口三档全部绿。无新失败项。**

---

## 2. 分项说明

### 2.1 L2 mechanisms

- 含 agent-tools / agent-loop / storage-pg lib + `run-product-smoke-e2e.sh`（check-modules + non-RAG 并行 + RAG 串行）。  
- 日志末行：`L2 mechanisms OK`。  
- 与 F0「smoke 只 build product_e2e」兼容：官方入口可完整跑通。

### 2.2 L2 integration 全量

| 指标 | 值 |
|------|-----|
| passed | **90** |
| failed | **0** |
| ignored | **24** |
| 时长 | ~693s |

**Ignored 分类（预期，非失败）：**

| 类 | 示例 |
|----|------|
| real LLM | `llm_real::*`（chat/rag/search/write/format/multi_turn/pdf…） |
| staging 真服务 | office_*_staging、paddle_pdf_smoke、search_real_smoke |
| 工具 | `cost_report`、`backend_launcher` |
| concurrent 真 LLM 变体 | `real_llm_concurrent_rag_queries_…` |

相对 F0–F6 修前诊断（**14 failed**）：本轮 **0 failed**。

### 2.3 L3 short smoke

- 入口：`PLAYWRIGHT_REUSE_SERVER=1`（F5 默认）。  
- **21/21** functional smoke 通过。  
- 未跑 `JOURNEY=1`（含 workspace-write 真 LLM 长路径）。

---

## 3. 与「是否该上 real LLM」的衔接

| 门禁层 | 本轮 | 含义 |
|--------|------|------|
| L2 mock 官方 + 全 integration | **绿** | mock 产品路径可视为收口 |
| L3 UI short | **绿** | 前端 smoke 可视为收口 |
| L3 LLM / llm_real | **未跑** | 下一档验收（成本/墙钟） |
| Journey 全量 / quality | **未跑** | 发版或专项再开 |

**诊断建议（不实施）：** 在 mock 收口已绿的前提下，real LLM 可按 **薄采样** 开下一会话，例如：

```bash
# 示例最小清单（需密钥；勿默认全 corpus）
cd avrag-rs
E2E_MODE=nightly cargo test -p app --test product_e2e --features product-e2e \
  llm_real::chat_real -- --ignored --test-threads=1 --nocapture
E2E_MODE=nightly cargo test -p app --test product_e2e --features product-e2e \
  llm_real::rag_real -- --ignored --test-threads=1 --nocapture
E2E_MODE=nightly cargo test -p app --test product_e2e --features product-e2e \
  llm_real::search_real -- --ignored --test-threads=1 --nocapture
E2E_MODE=nightly cargo test -p app --test product_e2e --features product-e2e \
  llm_real::write_real -- --ignored --test-threads=1 --nocapture
# 可选 UI：
# JOURNEY=1 bash scripts/test-l3-journey.sh   # 含 write，真 LLM 很长
```

---

## 4. 问题清单

| ID | 严重度 | 说明 |
|----|--------|------|
| — | — | **本轮无失败** |
| N1 | 信息 | 24 ignored 均为真服务/真 LLM/工具测，属金字塔设计 |
| N2 | 信息 | 全 journey + real LLM 仍未纳入本轮（有意） |

---

## 5. 复现命令

```bash
export CARGO_BUILD_JOBS=2
bash scripts/test-l2-mechanisms.sh
bash scripts/test-l2-integration.sh
bash scripts/test-l3-journey.sh   # 本地默认 reuse

# 日志
ls /tmp/context-osv6-gate-closeout-2026-07-10/
```

---

## 6. 变更记录

| 日期 | 说明 |
|------|------|
| 2026-07-10 | 收口跑测：L2_MECH / L2_INTEGRATION / L3_SMOKE 全绿；**停下，不修、不跑 real LLM** |
