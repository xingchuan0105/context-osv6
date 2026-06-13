# P4 前 MinerU / Shadow / 灰度迁移说明（历史）

> **Status:** 历史归档 — 仅供理解 P4 切换背景，**不是**当前部署或回滚指南。  
> **当前实现：** [LiteParse + Paddle Jobs 统一入库架构（2026-06-13）](../liteparse-paddle-ingestion-architecture-2026-06-13.md)

---

## 1. 已删除的运行时开关（P4 后无效）

以下环境变量在 P4 全量切换后已从代码与 `.env.example` 移除；设置它们**不会**改变现网行为：

| 变量 | 原用途 |
|------|--------|
| `LITEPARSE_ENABLED` | 启用/禁用 LiteParse 主链 |
| `LITEPARSE_SHADOW_MODE` | Shadow 模式：并行跑 LiteParse 与旧链，对比 diff |
| `LITEPARSE_ROLLOUT_PERCENT` | 按文档 hash 灰度切流百分比 |

**现网：** PDF 与 Office→PDF 文档**始终**走 LiteParse 主链，无开关、无 shadow artifact、无一键回退至 lopdf 主解析。

---

## 2. 已删除的 MinerU 配置

MinerU 模块与全部 `MINERU_*` 环境变量在 P4 已物理删除。历史配置示例（**勿再使用**）：

| 变量 | 原用途 |
|------|--------|
| `MINERU_API_KEY` | MinerU 云端 API 密钥 |
| `MINERU_BASE_URL` | MinerU API 基址 |
| `INGEST_MINERU_ENABLED` | 视觉路径旁路开关（Phase 1–3 讨论） |

独立图片 OCR 现由 **Paddle Jobs**（`ParseRoute::PaddleOcrImage`）承接，见主架构文档 §4。

更完整的 MinerU 退场讨论见 [视觉 PDF 入库与检索改造（2026-06-10）](./visual-pdf-ingest-requirements-2026-06-10.md)。

---

## 3. 原计划的上线路径（已跳过）

P0–P2 实现后，原计划为：

1. **P2.5 Shadow** — 并行产出 diff，不切换用户可见结果  
2. **P3 灰度** — `LITEPARSE_ROLLOUT_PERCENT` 按文档 hash 切流  
3. **P4** — 删 MinerU、删 shadow/rollout、全量 LiteParse  

实际执行：**跳过 P2.5/P3**，以 product E2E（`scripts/run-liteparse-staging-e2e.sh`）为门禁后直接 P4 全量。

---

## 4. 历史 wire 名（代码兼容层，M14 收尾）

P4 后部分枚举仍保留旧名以兼容历史 IR / metadata：

| 历史名 | 当前语义 |
|--------|----------|
| `PdfPageBackend::EdgeParse` | LiteParse 数字文本路径（非 lopdf 主解析） |
| `ParseBackend::MineruPdfOcr` / `MineruImage` | 历史 IR only；新 ingest 不得选择 |

命名迁移与 shadow-era API 清理见 [Brooks v6 计划 M14](../brooks-merged-fix-plan-2026-06-13-v6.md#m14--p4-历史命名与-shadow-era-api-收尾-p2)。
