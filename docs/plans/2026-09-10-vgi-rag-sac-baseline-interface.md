# VGI-RAG：新增 F 组产线 SAC 系统对照

日期：2026-09-10。承接 [Hard-60 准备](2026-09-10-vgi-rag-hard-benchmark-preparation.md)，按用户要求增加产线 SAC 检索接口，原有 A–E 和题集保留。

## 接口选择

F 使用 [chat 路由](../../avrag-rs/crates/transport-http/src/routes/chat.rs)及 [chat_post_handler](../../avrag-rs/crates/transport-http/src/handlers/chat.rs)，实际路径为 `POST /api/v1/chat`，handler 经 `state.conversation().execute` 执行。

请求遵循 [ChatRequest](../../contracts/src/chat.rs)：`agent_type="rag"`、`capabilities=["rag"]`、指定评测 `workspace_id` 和本题 `doc_scope`，`session_id=null`，前文通过 `messages` 提供，非流式返回并启用 debug。调用方不注入无效 `model` 字段，不绕过原生执行路径直接拼装检索结果。

F 执行产线 Lead/RAG Worker、SaC 取证和综合回答，属于完整系统基线。A/B/C/E 继续作为图×树的 2×2 消融；D 衡量桌面 Agent＋grep。F 与其他组的解析、切块、embedding、reranker、提示和内循环差异需要另报，不能把所有收益归因于图索引。

## 实现位置与边界

独立原型：`C:\Users\xingc\Documents\Codex\repository-tree`。

- `src/repository_tree/evaluation/hard/sac.py`：配置、原文到产线 UUID 绑定、HTTP 请求、原始响应与原生证据适配。
- `src/repository_tree/evaluation/hard/agent.py::create_agent`：统一 `answer(question, document_ids, prior_turns)`，F 不构造本地 Agent。
- `eval/SAC-INTERFACE.md`：配置、绑定格式、请求/输出和比较口径。
- `tests/test_sac_interface.py`：合成 HTTP 契约及隔离测试。

本轮未改 Rust 产品代码或原生提示，未向产线发出真实问答，未写入/新建生产 workspace，未重启或部署服务。配置模板增加应用 Bearer token 与 API 地址项；LLM API key 不能用于应用鉴权。

## 证据与运行保护

检索抽取对齐 [harness_extract.rs](../../avrag-rs/tests/rag_quality/src/harness_extract.rs) 的成功工具集合、数组/包裹结构和 first-seen 顺序。引用独立保留，不倒填成检索命中。原生答案、工具结果、用量、mode_debug 均保留，引用不自动修复。

空文档范围在本地拒绝，防止产线扩大到整个 workspace；未知 ID、原文哈希不一致、返回证据/引用越界均记录为接口或完整性错误，不计为检索质量失败。对所有成功检索行先审计再去重；缺失文档 ID 标记未核验，不猜测来源。

POST 不自动重试；HTTP 失败、超时、取消保留未知用量。服务端返回的模型与预期分别记录，未报告不当作一致。服务端汇总用量不等于本机独立计量了全部 Worker/embedding/reranker 消耗。绑定里的部署版本与原文哈希仍需真实回读验证。

## 联调前门槛

本轮独立原型的相关测试 **51 项通过，5.63 秒**，均为本地合成 transport；路由、请求字段及检索工具集合与当前源码对齐检查通过。候选准备脚本在禁用网络的进程中复跑成功，问题哈希未变。可提交验证记录位于独立工程 `evidence/sac-interface-validation.json`，不含凭证、真实题目或金标正文。

先完成同一原始语料的隔离 workspace 导入、绑定和出处回读，核验服务部署版本及各角色模型，再做短批原生 SAC 联调与六组统一运行。保持每组并发 2、全局最多 8、不设总 token 预算；服务内部并发另外观察。既有 Hard-60 五组准备回执和问题哈希保留，六组运行使用新的配置快照。
