# 三任务独立验收报告：ProviderPool / skillopt / cache-compression（2026-08-01）

> 验收方式：主线边界预审 + 3 个验收 subagent 按各 ACCEPTANCE 文档逐项执行 + 主线对关键发现二次复核。
> 结论速览：**skillopt 通过、cache-compression 通过、ProviderPool 有条件通过（1 必修）**。

## 1. ProviderPool（`55a6696a`）——有条件通过

硬门禁全过：`avrag-llm` 107（routing 8 新测试逐名核对）、`app-core` 22（配置解析 4）、`app-bootstrap` 11、worker 编译过；提交边界恰 8+1 文件、`search_executor` 重构确未混入；单 route 兼容（不配 pool 时分流为 None，行为不变）；§3.1/3.2/3.4 行为规格逐项与代码/测试对应。

**必修（已二次复核属实）：流式 `delivery_started` 误判，违反 §3.3「交付后不切换」**
- `crates/llm/src/client/mod.rs:635-638`：交付标志仅由**第一个事件**是否为 `TextDelta/ReasoningDelta` 决定。
- 但 OpenAI 兼容协议在正常流式响应中先发 `TextStart`/`ReasoningStart` **标记**再发 delta（`protocols/openai_chat/protocol.rs:76,92,118,134`，gemini/anthropic/openai_responses 同款）。
- 后果链：首事件是 Start 标记 → `delivery_started=false` → consume 真正把内容回调给调用方后中途断流 → 误判「未交付」→ `client/mod.rs:678` 整 provider 冷却 + **退款** + failover 重放 → **调用方收到重复前缀**。
- 这正是 failover 要处理的主场景，属行为规格实质偏离。修复方向：交付标志在 `consume_stream_events` 实际触发内容/推理回调后置位（如传 `&mut bool`），不看首事件；连同补流式边界测试（Start 首事件 + 交付后失败不切换）一并做。

建议：①consume 内部未交付失败一律按 Provider 冷却（anyhow 化丢 LlmError 分类，429 会过度冷却，与 §3.2 分级不一致）；②流式中途错误缺 `record_call_failure` telemetry。
登记：流式 failover 边界零测试覆盖（8 个 routing 测试只覆盖状态机与非流式 try_each）。

## 2. skillopt（`bb147c04`）——通过

§1-§8 全 PASS：14 文件全在 `tools/skillopt/` 零越界；prompts/产品代码零触碰；`check.sh` 全 [OK]（splits 104/30/15、task_types 21）；dataloader 149 题 id=1..149 展平一致；seed 与 agent-base.md 逐字一致（附注：该 md 当前是未跟踪文件属另一线开发批次，非落地缺陷，交叉验证成立）；golden-set 抽样 42 条零泄漏；密钥扫描零命中（yaml 仅环境变量名，runner 不打印值）；逐文件代码审查全【符合】（SwapPromptFile try/finally+备份、评测命令与 handover 文档逐字一致、--check 不触发 LLM、回填流程与红线在 README）。
低级观察 3 项：README 已知限制节未收 graphify 条目；rollouts.json 将含 golden 答案明文（outputs/ 已 gitignore，分享时注意）；`__import__("sys")` 非常规写法。

## 3. cache-compression（`1b2407e7`）——通过

40 文件全在白名单；`cargo check --workspace` 0 error；`avrag-llm` 107 / `agent-loop` 276 与预期逐字一致；功能四条全 ok（completion_cache 4 / trim_json+message_format 5 / synthesis 14 / openai_responses 16 含 reasoning_tokens==5 断言）；①-⑤ 核验点逐项【符合】带文件:行证据（trim 字节预算与保底、synthesis 去重/48k/保底非空、cache key 构成/TTL/kill switch/zeroed 累加、session summary 降级逐字节一致 + prompts-in-md、search 缓存 TTL/命中清 usage、bootstrap 可选注入、reasoning_tokens 19 列对齐 + migrations/0062 up/down 对称）；纪律无 graphify-out/.env 混入；app-chat 豁免失败名与文档点名一致。
低级 1 项：验收文档称 `accumulate` 为 saturating_add 实为普通 `+=`（结论不受影响，建议修正文档措辞）。

## 4. 汇总裁决

| 任务 | verdict | 必修 | 建议/登记 |
|---|---|---|---|
| ProviderPool | **有条件通过** | 1（流式交付边界误判） | 建议 2 / 登记 2 |
| skillopt | **通过** | 0 | 低级观察 3 |
| cache-compression | **通过** | 0 | 文档措辞 1 |

ProviderPool 必修项待用户拍板后修复（修复方向明确：回调置位交付标志 + 流式边界测试）。
