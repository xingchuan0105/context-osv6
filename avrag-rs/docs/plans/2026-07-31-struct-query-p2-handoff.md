# struct-query P2 开工文档（2026-07-31）

> 上游：`docs/plans/2026-07-31-struct-query-virtual-tables.md` §13 第 6 项。
> 前置：loop 终答契约 F1/F2/F3 已收官（commit 61ab43f0 交接文档）；2b 表级证据 chunk 已提交（commit 8682e7e7）。

## 0. 一句话现状

P2 四项（telemetry / supervision 工具化 / fts / 数值规整）按序开工。telemetry 先行：终答契约 Activity 计数已透出到非流式 harness 可读面（commit 96d2265d），剩余 pipeline 侧指标与真实触发率观测。

## 1. 四项定义与当前状态

| 项 | 内容 | 状态 |
|---|---|---|
| **telemetry** | ① loop 侧：`synthesis_code_answer_repair`/`violation` Activity stage 计数接成可查指标；② pipeline 侧：提取成功率、指令分布、low/quarantine 率 | ① **已落地**（commit 96d2265d）：Activity counts → `debug_metadata` → analytics + `mode_debug.general`；② 待开工 |
| **supervision 工具化** | P1b 的 6 工具薄 loop（`scripts/struct_query_poc/supervise.py` + `prompts/pipeline/table-supervision/`）从 PoC 脚本产品化为 fetch_slice/run_check 自由调用形态 | 待开工 |
| **fts 表内值发现** | struct_query 加 FTS 索引/值发现查询面 | 待开工（依赖 2b 已提交，无阻塞） |
| **数值规整** | finetype 数值规整列；改提取产物 → 需重灌语料 + 复跑 A0–A3 | 待开工（建议放最后，复用前三项 telemetry 与工具化 supervision） |

## 2. telemetry 已落地部分（commit 96d2265d）

链路：

```
agent-loop synthesis.rs
  → Activity { stage: "synthesis_code_answer_repair"|"synthesis_code_answer_violation",
               counts: { stage: 1 } }
  → CollectingSink（非流式）
  → app-chat pipeline_steps::activity_counts_from_events（折叠 stage 计数）
  → attach_activity_counts_from_sink → execution.debug_metadata["activity_counts"]
  → service_postprocess 已消费 debug_metadata → analytics metadata（可查）
  → merge_activity_counts_into_mode_debug → ChatResponse.mode_debug.general["activity_counts"]
  → 非流式 product_e2e harness 读 HTTP 响应 JSON
```

读法（非流式 harness）：

```rust
let chat: ChatResponse = resp.into_business()?;
let counts = chat.mode_debug
    .and_then(|d| d.general)
    .and_then(|g| g.get("activity_counts").cloned());
// counts["synthesis_code_answer_repair"] == 1 → repair 触发
// counts["synthesis_code_answer_violation"] == 1 → 兜底文案
```

流式路径：SSE 已有 Activity 事件（counts 字段透传），harness 走流式时直接读 SSE。

## 3. telemetry 剩余（下一窗口）

- **pipeline 侧指标**：supervise.py 健康报告聚合（提取成功率 / 指令分布 / low/quarantine 率），接成可查面。
- **真实触发率观测**：跑 `QUESTIONS=86,88,106` 切片，从 `mode_debug.general["activity_counts"]` 读 repair/violation 真实触发率，决定是否需要 C5 carryover 增强 / 预算调优（§3.1 决策依据）。
- **预算调优**：28K token / 12 轮是否适配 struct 深调查，等触发率数据，别凭切片单轮感觉调。

## 4. 验收口径（延伸 §11 矩阵）

| ID | 检查 | Phase | 状态 |
|----|------|-------|------|
| T1 | `synthesis_code_answer_repair`/`violation` Activity 计数可从非流式 HTTP 响应读出 | P2 | ✅（commit 96d2265d；单测 2 例 + agent-loop 272 全绿） |
| T2 | pipeline 提取成功率 / 指令分布 / low/quarantine 率可查 | P2 | ⬜ |
| T3 | 切片 86/88/106 的 repair/violation 真实触发率有数据 | P2 | ⬜ |

## 5. 开工顺序与依赖

```text
[x] 1. telemetry loop 侧：Activity 计数透出（commit 96d2265d）
[ ] 2. telemetry pipeline 侧 + 真实触发率观测
[ ] 3. supervision 工具化（改动面与脏树零交集，可与 2 并行）
[ ] 4. fts 表内值发现（动 struct_query.rs，已提交干净）
[ ] 5. 数值规整（改提取产物，需重灌 + 复跑 A0–A3，放最后）
```

## 6. 操作要点

- **脏树仍在**（SaC 线在途）：`agent-loop/src/react_loop/{assembler.rs, policy/*, skill_request.rs}`、`app-chat/src/chat/pipeline_tests.rs` 等。commit 只挑自己 hunk。
- **验证命令**：
  ```bash
  cd /home/chuan/context-osv6/avrag-rs
  CARGO_BUILD_JOBS=2 cargo test -p agent-loop --lib
  CARGO_BUILD_JOBS=2 cargo test -p app-chat --lib activity_counts
  # 切片（真实 LLM，约 2.5~4 分钟）：
  cd /home/chuan/context-osv6
  CARGO_BUILD_JOBS=2 STRUCT_STORE_DIR=$PWD/avrag-rs/storage/struct_store \
    QUESTIONS=86,88,106 bash avrag-rs/scripts/sac-skill-fail6-reg.sh
  ```
- **LLM 抖动**：同题至少两轮再定论；artifact 读取先看 mtime。
- 修结构性代码后跑 `graphify update .`；`graphify-out/` 不入库。
