# struct-query P2 开工文档（2026-07-31）

> 上游：`docs/plans/2026-07-31-struct-query-virtual-tables.md` §13 第 6 项。
> 前置：loop 终答契约 F1/F2/F3 已收官（commit 61ab43f0 交接文档）；2b 表级证据 chunk 已提交（commit 8682e7e7）。

## 0. 一句话现状

P2 四项（telemetry / supervision 工具化 / fts / 数值规整）按序开工。telemetry 先行：终答契约 Activity 计数已透出到非流式 harness 可读面（commit 96d2265d），剩余 pipeline 侧指标与真实触发率观测。

## 1. 四项定义与当前状态

| 项 | 内容 | 状态 |
|---|---|---|
| **telemetry** | ① loop 侧：`synthesis_code_answer_repair`/`violation` Activity stage 计数接成可查指标；② pipeline 侧：提取成功率、指令分布、low/quarantine 率 | ① **已落地**（commit 96d2265d）：Activity counts → `debug_metadata` → analytics + `mode_debug.general`；② **已落地**（commit c35a7fc0 `check_telemetry.py`） |
| **supervision 工具化** | P1b 的 6 工具薄 loop 从 PoC 脚本产品化（Rust 重写） | **已落地**（commit ccf02ea9 S1 确定性核心 + 5c11bb2c S2 loop/S3 CLI）：crate `avrag-struct-supervision`，31 测试全绿，ipd 2 轮 high / 白药 5 轮 live 对拍通过 |
| **fts 表内值发现** | struct_query 加 FTS 索引/值发现查询面 | 待开工 |
| **数值规整** | finetype 数值规整列；改提取产物 → 需重灌语料 + 复跑 A0–A3 | 待开工（放最后） |

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

- **真实触发率已有第一轮数据**：三题 0 触发 repair/violation（`fail6_20260731-171901.log`），repair 回路尚未被真实行使过——继续积累切片数据，触发率 >0 时决定是否需要 C5 carryover 增强。
- **预算调优**：86/106 各 1 次 `budget_exhausted`（token 耗尽），但终答均正常（C5 闸门生效）；28K/12 轮是否需调，等更多轮次数据，别凭单轮感觉调。
- **Q88 SELECTION_MISS**：非终答问题（recall=1.0 但 selection=0），与 struct 取证无关，属既有抖动（同题两轮全 PASS 历史）。

## 4. 验收口径（延伸 §11 矩阵）

| ID | 检查 | Phase | 状态 |
|----|------|-------|------|
| T1 | `synthesis_code_answer_repair`/`violation` Activity 计数可从非流式 HTTP 响应读出 | P2 | ✅（commit 96d2265d；单测 2 例 + agent-loop 272 全绿；切片 artifact `q0NN.json` 的 `mode_debug.general["activity_counts"]` 实证透出） |
| T2 | pipeline 提取成功率 / 指令分布 / low/quarantine 率可查 | P2 | ✅（commit c35a7fc0 `check_telemetry.py`：ipd rate=1.0 / 白药 rate=0.0 全 needs_diagnosis，check 分布 header_suspicious×9 / header_numeric_banner×1 / empty_columns×3） |
| T3 | 切片 86/88/106 的 repair/violation 真实触发率有数据 | P2 | ✅（log `fail6_20260731-171901.log`：三题均 0 触发；Q86 PASS / Q106 PASS / Q88 SELECTION_MISS 非终答问题；budget_exhausted 86/106 各 1 次） |
| S1 | supervision Rust 确定性核心与 Python 语义一致 | P2 | ✅（commit ccf02ea9：28 测试全绿，移植 check_supervise.py 12 项；ipd 370 行 smoke） |
| S2 | 6 工具薄 loop（done 终止/预算降级/未知工具容错） | P2 | ✅（commit 5c11bb2c：+3 mock 测试，31 全绿） |
| S3 | CLI 与 Python 对拍 | P2 | ✅（dry-run 简报对齐；ipd live 2 轮 t0→high+detail+370 行；白药 live 5 轮 6 quarantine/3 low，无 high、无数据改写指令，quarantine 不入库） |
| S4 | struct_query 兼容产物 + ingestion 挂接 | P2 | ◐ Rust 产物与 pipeline 同形状（_meta 12 列/evidence_chunk_id 与 sidecar 一致，有单测断言）；ingestion worker 挂接待其表格提取阶段 Rust 化（worker 现为通用任务队列，无表格阶段） |

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
