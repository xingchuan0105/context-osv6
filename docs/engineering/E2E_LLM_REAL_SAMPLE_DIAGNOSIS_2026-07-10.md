# Real LLM 薄采样诊断（四模式）

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-10 |
| 原则 | **只跑只记，不自动修复** |
| 前置 | Mock 门禁收口全绿（`E2E_GATE_CLOSEOUT_DIAGNOSIS_2026-07-10.md`） |
| 日志 | `/tmp/context-osv6-llm-real-2026-07-10/` |
| 墙钟 | ~14:03–14:10 CST（约 6.4 min） |
| 模型 | `AGENT_LLM_MODEL=deepseek-v4-flash`（密钥已配置，不记入文档） |

**本轮范围（薄采样，非 `test-l3-llm.sh` 全量 llm_real）：**

| 模块 | 用例 |
|------|------|
| `llm_real::chat_real` | general chat 实质性回答 |
| `llm_real::rag_real` | 文档 QA + 复杂多工具（2 tests） |
| `llm_real::search_real` | 联网引文 |
| `llm_real::write_real` | Write 多阶段长文 |

**未跑：** `rag_quality_prod`、pdf_corpus、format_real、multi_turn、全 `llm_real` filter、`JOURNEY=1`。

---

## 1. 总览

| 套件 | Exit | 结果 | 墙钟（约） |
|------|------|------|------------|
| **CHAT_REAL** | **0** | **1 passed** | ~15s |
| **RAG_REAL** | **0** | **2 passed** | ~157s |
| **SEARCH_REAL** | **0** | **1 passed** | ~46s |
| **WRITE_REAL** | **101** | **1 failed** | ~192s |

```text
CHAT_REAL=0
RAG_REAL=0
SEARCH_REAL=0
WRITE_REAL=101
```

**结论：Chat / RAG / Search 真 LLM 薄路径绿；Write 真 LLM 在流式读超时失败。**

---

## 2. 失败详情（WRITE_REAL）

| 项 | 内容 |
|----|------|
| 测试 | `llm_real::write_real::real_llm_write_mode_produces_article_with_fingerprint` |
| 位置 | `write_real.rs:43` — `.expect("write stream")` |
| 错误 | `write stream: error decoding response body` |
| 根因链 | `request or response body error` → **`operation timed out`** |
| 环境线索 | 日志：`using real Brave Search at https://api.search.brave.com`（Write research 阶段会打 Search） |
| 时长 | 测试进程 ~192s 后失败 |

### 超时相关常量（只读诊断）

| 常量 | 值 | 文件 |
|------|-----|------|
| `REAL_LLM_STREAM_DEADLINE` | **180s** | `product_e2e/llm_real/mod.rs` |
| Write 管线 | research（真 Search）+ skeleton + multi-section draft + refine + validate | 产品路径 |
| 历史 mock smoke | write_smoke ~30s 级 | 真 LLM 远长于 180s 合理 |

**诊断（不修）：** 180s 流式 deadline / HTTP client 读超时 **不足以覆盖** 真 LLM Write 全链路（research  alone 常需数分钟；与 Playwright write journey 600s 设定一致）。失败形态是 **客户端超时截断 SSE**，不是断言正文不够长或 agent_type 错。

### 建议修复方向（本轮不实施）

| 优先级 | 方向 |
|--------|------|
| P0 | 提高 Write 专用 stream deadline / `HTTP_TIMEOUT_REAL_LLM_SECS`（例如 600s，与 UI journey 对齐） |
| P1 | Write research 在真 LLM 测中可降采样 / mock search 对照（若目标只锁 draft 质量） |
| P2 | 分阶段断言（先 activity research/skeleton，再终端 done）便于区分 research 慢 vs draft 挂 |

---

## 3. 通过项摘要

| 模块 | 含义 |
|------|------|
| chat_real | 真 LLM general chat 非空实质回答 |
| rag_real ×2 | 真 LLM + 检索路径可用（含多工具变体） |
| search_real | 真 LLM + 真 Brave 联网引文路径可用 |

---

## 4. 问题清单

| ID | 严重度 | 说明 |
|----|--------|------|
| **L1** | **中** | `write_real` 流式 **timeout**（~180s 不够）；非内容断言失败 |
| N1 | 信息 | Chat/RAG/Search 真 LLM 薄采样 **绿** |
| N2 | 信息 | 未跑 quality corpus / 全 journey；Write 失败 **不推翻** mock 门禁收口 |

---

## 5. 复现

```bash
cd avrag-rs
export E2E_MODE=nightly CARGO_BUILD_JOBS=2
set -a && source .env && set +a

cargo test -p app --test product_e2e --features product-e2e \
  llm_real::chat_real -- --ignored --test-threads=1 --nocapture
cargo test -p app --test product_e2e --features product-e2e \
  llm_real::rag_real -- --ignored --test-threads=1 --nocapture
cargo test -p app --test product_e2e --features product-e2e \
  llm_real::search_real -- --ignored --test-threads=1 --nocapture
cargo test -p app --test product_e2e --features product-e2e \
  llm_real::write_real -- --ignored --test-threads=1 --nocapture
# 日志
ls /tmp/context-osv6-llm-real-2026-07-10/
```

---

## 6. 变更记录

| 日期 | 说明 |
|------|------|
| 2026-07-10 | 薄采样 chat/rag/search **绿**；write **超时红**；**停下不修** |
| 2026-07-10 | 修复：`WRITE_REAL_STREAM_DEADLINE` + `HTTP_TIMEOUT_REAL_LLM_SECS` → **600s**；重跑 write_real **1 passed**（~273s） |
