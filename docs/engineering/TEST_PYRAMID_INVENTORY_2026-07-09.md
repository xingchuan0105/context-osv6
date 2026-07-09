# 测试金字塔 Inventory（P5 完成稿）

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-09 |
| 拍板 | 日常 L1；机制 L2；旅程/真 LLM L3；性能独立 |
| 状态 | **Done** — 入口脚本 + 去重映射 + L1 实测 + 巨石部分拆分 |

## 目的标签

- **A** 真 UI / 可感旅程  
- **B** 真 LLM 主链路  
- **C** 四 Agent loop 健康  
- **D** 性能  
- **E** 机制/契约正确  

## 正式入口

| 脚本 | 层 | 内容 |
|------|-----|------|
| `scripts/test-l1.sh` | L1 | file-size + default crates `--lib` + tsc |
| `scripts/test-l2-mechanisms.sh` | L2 | loop/tools/storage lib + product smoke mock |
| `scripts/test-l2-integration.sh` | L2 | product_e2e integration mock |
| `scripts/test-l3-journey.sh` | L3 | Playwright smoke；`JOURNEY=1` 全旅程 |
| `scripts/test-l3-llm.sh` | L3 | llm_real 抽样（非 quality 语料） |
| `scripts/bench-test-suites.sh` | 测时 | 写 wall-clock 表 |

去重规则：[`TEST_PYRAMID_DEDUP_MAP.md`](./TEST_PYRAMID_DEDUP_MAP.md)。

## L1 实测（本机 `bench-test-suites.sh`，2026-07-09）

| Suite | Wall-clock | Result |
|-------|------------|--------|
| L1 file-size gate | ~0s | ok |
| L1 agent-tools --lib | ~1s | ok |
| L1 agent-loop --lib | ~1s | ok |
| L1 app-chat --lib | ~29s | ok |
| L1 transport-http --lib | ~26s | ok |
| L1 storage-pg --lib | ~1s | ok |
| L1 frontend tsc | ~3s | ok |
| **L1 合计（串行）** | **~1 min** | 预算 ≤5 min ✓ |

> 冷编译首次会更长；上表为已编译后的测试执行时间。

## L2 / L3（不自动跑；手工填）

| Suite | 预算 | 触发 | 实测 |
|-------|------|------|------|
| product smoke mock | ≤20 min | 改机制 / 波次 | _待填_ |
| integration mock | ≤40 min 或拆 core | 波次 | _待填_ |
| Playwright smoke | ≤15 min | 波次末 | _待填_ |
| Playwright journey | ≤45 min | 发版/夜间 | _待填_ |
| llm_real 四模式抽样 | ≤40 min | 波次末 | _待填_ |
| rag_quality_prod | 无日常预算 | release | _待填_ |

## 资产归层

| 资产 | 层 | 目的 |
|------|-----|------|
| crate `--lib` / contracts / vitest / tsc / file-size | L1 | E/C |
| product_e2e smoke mock | L2 | E/C |
| product_e2e integration mock | L2 | E |
| product_e2e llm_real 薄路径 | L3 | B/C |
| rag_quality_prod | L3-release | B/D |
| Playwright smoke | L3-smoke | A |
| Playwright journey | L3-journey | A |
| skills / judge / billing | L3 specialty | A/B |

## 巨石拆分结果

| 项 | 结果 |
|----|------|
| storage-pg cleanup 测 | 拆为 `cleanup_delete_soft` / `cleanup_task` / `cleanup_targets` |
| llm_real stream 单测 | → `stream_reasoning_tests.rs` |
| llm_real cost report | → `cost_report.rs` |
| llm_real/mod.rs | ~913 行（helpers 仍集中；再拆需单独波次） |
| rag_quality_prod.rs | 保留单文件，**仅 release**；禁止进日常 |
