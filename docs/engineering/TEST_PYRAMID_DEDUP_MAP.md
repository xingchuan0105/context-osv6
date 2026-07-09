# 测试去重映射（P5-4）

原则：**同一断言只保留在最低足够层**。上层旅程/真 LLM 只验证「用户能完成」与「链路活着」，不重复测协议细节。

| 关注点 | 保留层 | 权威位置 | 上层禁止再测什么 |
|--------|--------|----------|------------------|
| JWT / 鉴权形状 | L1 | `transport-http` auth 单测 | smoke 只测业务 401/403 语义 |
| SSE 事件序（start→done） | L1 | `transport-http` `chat_stream_contract` | product_e2e 不重复完整序断言 |
| Workspace CRUD API 字段 | L1/L2 | contracts + smoke `workspace_crud` | Playwright 只点 UI，不 assert JSON 字段全集 |
| ToolCatalog / dispatch | L1 | `agent-tools` lib | E2E 不测 tool id 枚举 |
| Loop 迭代/退出策略 | L1 | `agent-loop` lib | 真 LLM 只看「有答案/有工具活动」 |
| Mock RAG 可引用 | L2 | product_e2e smoke rag_* | L3 skills 用硬引文门，不重复 mock 路径 |
| 真 LLM 四模式 | L3 | llm_real 薄路径 | 不把 quality 语料塞进日常 |
| 质量 recall | L3-release | `rag_quality_prod` | 不进 L1/L2/日常 L3 抽样 |
| UI 登录/建工作区 | L3-smoke | Playwright smoke | vitest 不模拟完整浏览器 |
| 上传→RAG 长旅程 | L3-journey | Playwright journey | 不与 product smoke 双跑同一路径作为日常 |

## 执行入口

| 脚本 | 层 |
|------|-----|
| `scripts/test-l1.sh` | L1 |
| `scripts/test-l2-mechanisms.sh` | L2 |
| `scripts/test-l2-integration.sh` | L2 |
| `scripts/test-l3-journey.sh` | L3 UI |
| `scripts/test-l3-llm.sh` | L3 真 LLM |
| `scripts/bench-test-suites.sh` | 测时（填 inventory） |

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
