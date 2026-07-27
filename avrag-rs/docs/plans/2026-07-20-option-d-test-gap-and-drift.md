# Option D 测试集对齐：缺口与漂移清单

| 项目 | 内容 |
|------|------|
| 日期 | 2026-07-20 |
| 状态 | **T1–T4 已落地（2026-07-20）**；原缺口关闭见 §3 |
| 范围 | 将 **已落地 Option D 代码/机制** 与 **单测 / product_e2e / full_eval / mock_llm / golden** 对照 |
| 设计真值 | [`2026-07-20-unified-product-agent-option-d.md`](./2026-07-20-unified-product-agent-option-d.md) |
| 前置 | Option D **hard cutover 已开发完**；无长期 `PRODUCT_AGENT_OPTION_D` 产品双路径 |

---

## 0. 一句话结论

**T1–T4 已补齐：** 单测覆盖 query 证据槽位、direct、dual 积木、短语互斥、V2 真 executor Answer pack、效用 tool 解析、空证据、编排路径 fail-fast；mock_llm 识别 Option D Answer + worker handoff；full_eval 对 5xx/error 信封/空 answer/eval 桥做分类；golden 增 `option_d_*` 子集；文档漂移已消。

验证：

```bash
cargo test -p app-chat --lib
cargo test -p agent-loop --lib -- config::tests
```

---

## 1. 实现面（真值，简表）

| 机制 | 落点 |
|------|------|
| AnswerOnly | pure chat → `assemble_mode` + utility pool |
| Answer pack | `AgentServiceExecutor::run_chat`：product-answer-base + chat-base + 积木 |
| 证据 | **query** `query_for_agent`；system 无 `### Evidence` |
| Exit | `finish_answer` / `delegate_chat`；`mode=direct\|synthesize` |
| 效用 tools | `utility_tool_pool`；retrieve 相位暴露；synthesis `tools:[]` |

---

## 2. 已对齐测试（关闭后）

| 验收点 | 测试 |
|--------|------|
| G-01 query Evidence / system 无 Evidence | `host::answer_pack_evidence_lives_in_query_not_system` |
| G-03 direct 无 Evidence / 无积木 | `host::answer_pack_direct_mode_skips_evidence_and_material_blocks` |
| G-04 短语互斥 | `host::answer_vs_dispatch_phrase_mutex` + brain V2 Dispatch 负向 |
| G-07 dual 积木 run_chat | `host::answer_pack_dual_materials_load_all_answer_blocks` |
| G-08 空证据单源 | `host::answer_pack_empty_evidence_single_source_contract` |
| G-05/G-06 utility resolve + 禁检索 | `host::answer_mode_tools_for_retrieve_exposes_utility_forbids_retrieval` |
| G-02 V2 真 Answer pack | `brain::v2_answer_phase_uses_product_answer_pack_via_real_executor` |
| G-09 编排路径 internal error | `pipeline_tests::orchestrated_agent_internal_error_propagates_fail_fast` |
| G-13 Option D mock prose | `mock_llm_server` `is_option_d_answer_phase` / `mock_option_d_answer_prose` |
| G-14 worker handoff JSON | `mock_worker_handoff_json` / `is_worker_handoff_prompt` |
| G-11/G-12 fail-fast 信封 | `rag_quality_prod` HTTP 5xx / error envelope / empty_answer |
| G-16 eval bridge | `rag_quality_prod` dense_retrieval 桥断言 |
| G-15 golden 场景 | `option_d_pure_chat_smoke` / `option_d_search_only` / `option_d_dual_source` |
| G-17 utility tools | golden `option_d_utility_tools` + full_eval utility `expected_tool` hard gate；mock `calculator`/`weather_query`；smoke `pure_chat_calculator_*` |
| D-01…D-08 文档 | 设计稿 §13 勾选、提示词文 superseded、host/chat_exit 注释、config 测试注释、assembler 注释 |

---

## 3. 原缺口关闭表

| ID | 原级别 | 状态 |
|----|--------|------|
| G-01…G-09 | P0/P1 | **closed** |
| G-11…G-16 | P2 | **closed** |
| G-17 | P2 | **closed**（2026-07-20）：golden `option_d_utility_tools` + `chat_builtin_tools` 硬门；mock calculator/weather；smoke `pure_chat_calculator_*` |
| D-01…D-08 | P3 | **closed** |

---

## 4. 非缺口（保留）

| 项 | 说明 |
|----|------|
| `chat.yaml` 仅 user_context | YAML 基线；有效池 `utility_tool_pool` |
| BrainMockExec 假答案 | 调度测仍合理；契约测改走真 executor（G-02） |
| RETRIEVAL_MISS 不停 fail-fast | 有意：质量分 ≠ 基础设施失败 |
| 真机 full_eval 全量 | 本波次补 harness；全量跑通另开任务 |

---

## 5. 修订记录

| 日期 | 说明 |
|------|------|
| 2026-07-20 | 初稿：P0–P3 缺口清单 |
| 2026-07-20 | **T1–T4 落地**：关闭 G/D 表；指向具体测试符号 |
