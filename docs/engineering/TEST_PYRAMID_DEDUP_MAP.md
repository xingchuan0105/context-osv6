# 测试去重映射（P5-4 + L3 整合 2026-07-10）

原则：**同一断言只保留在最低足够层**。上层旅程/真 LLM 只验证「用户能完成」与「链路活着」，不重复测协议细节。

**L3 整合与标准灌库计划（入口 / 波次 / DoD）：**  
[`L3_TEST_INTEGRATION_AND_CORPUS_PLAN_2026-07-10.md`](./L3_TEST_INTEGRATION_AND_CORPUS_PLAN_2026-07-10.md)



## 标准文档 / 灌库复用（核心）

| 项 | 约定 |
|----|------|
| **标准文件** | `antifragile.txt`（Rust `product_e2e/fixtures` 与 `frontend_next/e2e/fixtures` **同 MD5**） |
| **Rust L3-thin** | `fixtures/standard_doc.rs`：`shared_standard_doc_real_llm()` **每 test binary 冷上传+ingest 一次**，rag / multi_turn / format 复用 |
| **Playwright journey** | upload→RAG 使用同一 `antifragile.txt`，问题对齐 antifragility（与 llm_real 一致） |
| **不需要灌库** | general chat、open search、write 成文（无 doc_scope） |
| **禁止** | 四模式 thin 各自重复 cold ingest 同一 txt；quality 语料仍独立（smoke_v5 / realistic） |

## 关注点 sole owner

| 关注点 | 保留层 | 权威位置 | 上层禁止再测什么 |
|--------|--------|----------|------------------|
| JWT / 鉴权形状 | L1 | `transport-http` auth 单测 | smoke 只测业务 401/403 语义 |
| SSE 事件序（start→done） | L1 | `transport-http` `chat_stream_contract` | product_e2e 不重复完整序断言 |
| Workspace CRUD API 字段 | L1/L2 | contracts + smoke `workspace_crud` | Playwright 只点 UI，不 assert JSON 字段全集 |
| ToolCatalog / dispatch | L1 | `agent-tools` lib | E2E 不测 tool id 枚举 |
| Loop 迭代/退出策略 | L1 | `agent-loop` lib | 真 LLM 只看「有答案/有工具活动」 |
| Mock RAG 可引用 | L2 | product_e2e smoke `rag_smoke` | L3 skills 不重复 mock 路径硬门 |
| 真 LLM 四模式各 1 | **L3-thin-llm** | `chat_real` / `rag_real`（单条）/ `search_real` / `write_real` | 不把 quality 塞进 DR2 |
| 质量 recall | **L3-full** | `test-l3-quality.sh` → `rag_quality_prod` | 不进 DR2 / `test-l3-llm.sh` 默认 |
| UI 登录/导航 | **L3-thin-ui** | Playwright smoke | journey 不测 JWT |
| 上传→RAG UI 旅程 | **L3-journey** | Playwright journey（标准 doc） | 不进 DR2 默认 |

## 执行入口

| 脚本 | 层 | DR2 默认 |
|------|-----|----------|
| `scripts/test-l1.sh` | L1 | — |
| `scripts/test-l2-mechanisms.sh` | L2-core | yes |
| `scripts/test-l2-patho.sh` | L2-patho | yes |
| `scripts/test-l3-ui-smoke.sh` | L3-thin-ui | **yes** |
| `scripts/test-l3-llm.sh` | L3-thin-llm（四模式） | **yes** |
| `scripts/test-l3-journey.sh` | L3-journey | no（DR3 / 显式） |
| `scripts/test-l3-quality.sh` | L3-full quality | no |
| `L3_LLM_EXT=1 test-l3-llm.sh` | + multi_turn + format | no |
| `L3_LLM_FULL=1 test-l3-llm.sh` | 整包 llm_real | no |

## 巨石文件状态

| 文件 | 行数量级 | 处理 |
|------|----------|------|
| storage-pg cleanup 测 | 已拆 | `cleanup_delete_soft` / `cleanup_task` / `cleanup_targets` |
| llm_real/mod.rs | ~1k | 抽出 `stream_reasoning_tests.rs`（无网单测）；完整再拆推迟（风险高） |
| llm_real/rag_quality_prod.rs | ~1.3k | **L3-release only**；保持单文件，入口为 release gate |
| test_context/builder+http | 大 | 基础设施；改时再拆，不阻塞金字塔 |

## 明确不做

- 为去重删除真 LLM 质量语料  
- 把 Playwright 当 API 契约测试  
- 日常合并 L1+L2+L3 为一条命令  
