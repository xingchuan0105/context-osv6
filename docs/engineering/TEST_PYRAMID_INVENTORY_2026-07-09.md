# 测试金字塔 Inventory（P5-0）

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-09 |
| 拍板 | 日常 L1；机制 L2；旅程/真 LLM L3；性能独立 |
| 状态 | Inventory 初稿（wall-clock 待 `scripts/bench-test-suites.sh` 实测补齐） |

## 目的标签

- **A** 真 UI / 可感旅程  
- **B** 真 LLM 主链路  
- **C** 四 Agent loop 健康  
- **D** 性能  
- **E** 机制/契约正确  

## 资产归层

| 资产 | 建议层 | 目的 | 触发（拍板后） | 估时（待测） |
|------|--------|------|----------------|--------------|
| `cargo test -p X --lib` | L1 | E/C | 每次提交定向 | 秒～数分钟 |
| `transport-http` lib tests | L1 | E | 改 HTTP 时 | ~1 min |
| `contracts` tests | L1 | E | 改契约时 | <1 min |
| `frontend_next` vitest | L1 | E | 改 FE 时 | ~1–3 min |
| `tsc --noEmit` | L1 | E | 改 FE 时 | ~1 min |
| `scripts/check_file_size_limits.sh` | L1 | E | 每次 | <5s |
| `product_e2e` smoke mock | L2 | E/C | 动机制/波次 | 目标 ≤20 min |
| `product_e2e` integration mock | L2 | E | 波次 / weekly edge | 目标 ≤40 min 或拆 core |
| `product_e2e` llm_real 薄路径 | L3 | B/C | 波次末 | 目标 ≤40 min 抽样 |
| `rag_quality_prod` | L3-release | B/D | release/weekly | 长 |
| Playwright `e2e/specs/smoke` | L3-smoke | A | 波次末 | 目标 ≤15 min |
| Playwright `e2e/specs/journey` | L3-journey | A | 发版/夜间 | 目标 ≤45 min |
| Playwright skills / judge | L3-quality | A/B | nightly | 长 |
| Playwright billing / visual | L3-specialty | A | path / weekly | 中 |

## 正式入口（目标）

| 脚本 | 层 |
|------|-----|
| `scripts/test-l1.sh` | L1 日常 |
| `scripts/test-l2-mechanisms.sh` | L2 机制 |
| `scripts/test-l2-integration.sh` | L2 integration |
| `scripts/test-l3-journey.sh` | L3 UI |
| `scripts/test-l3-llm.sh` | L3 真 LLM 抽样 |

旧 workflow / `run-product-smoke-e2e.sh` 保留为实现细节，文档只推上表。

## 测时记录（填）

| 入口 | 机器 | wall-clock | 日期 | 备注 |
|------|------|------------|------|------|
| test-l1 (default crates) | | | | |
| product smoke mock | | | | |
| playwright smoke | | | | |
| llm_real sample | | | | |
