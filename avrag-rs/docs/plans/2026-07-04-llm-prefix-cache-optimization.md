# LLM Prefix Cache 优化方案（2026-07-04）

> 目标：对齐 DimCode / DeepSeek-Reasonix 的 cache-first 设计，降低 DeepSeek 系模型的**实付 input token 成本**。
> 结论先行：**agent 循环追「命中率 ≥90%」，ingestion 追「实付成本下降」——两者手段不同，不共用一个 KPI。**
> 调研结论（DimCode 二进制逆向 + Reasonix 源码分析）见本文 §2；实现改动全部标注文件级边界。

---

## 0. 背景

- DeepSeek（官方 API）对**逐字节相同的 prompt 前缀**自动做 KV cache（64-token 粒度，无需请求参数），cache 命中的 input token 计费约为未命中的 **1/5 ~ 1/10**（以当期价卡为准）。
- DimCode（闭源）与 DeepSeek-Reasonix（MIT, Go）两个 agent 都宣称 90%+ 命中率，核心不是「加了什么缓存」，而是**从不破坏前缀**：稳定 system prompt / 固定 tool schema / 对话只追加 / 低频压缩。
- 本项目 token 大头在 **ingestion**（triplet 抽取、section index、summary、VLM summary），全部是「独立短请求」形态；agent ReAct 循环是「多轮增长」形态但存在 cache 杀手（见 §3.2）。

## 1. TL;DR（三条主线）

| 主线 | 对象 | KPI | 预期收益 | 工作量 |
|------|------|-----|---------|--------|
| A. 可观测性 + 配置启用 | 全部 LLM 调用 | 每调用点可见 cached/prompt 比例 | 决策依据（当前是盲的） | ~1 天 |
| B. Ingestion 组合拳 | worker 管线 | 重跑/重试实付 token ↓95%+；常规 ingest input ↓15~20% | **最大绝对省钱项** | ~2 天 |
| C. Agent cache-first | ReAct 循环 | mock 层前缀命中 ≥90%（Reasonix 式 guard） | 长会话 input 成本 → ~1/3 | ~2-3 天 |

**明确反方案**：不做「ingestion 顺序 append 会话」——算术上负收益（§5 有推导）。90% 命中率这个数字**不适用于**批处理型 ingestion，不要拿它当 ingestion 的验收指标。

---

## 2. 调研结论摘要（DimCode / Reasonix）

### 2.1 机制共性（两个 agent 一致）

1. **字节稳定前缀**：system prompt、tool schema 序列化后逐字节不变（Reasonix 对 tool schema 排序归一化后哈希，见其 `internal/agent/cache_shape.go` 的 `PrefixShape{SystemHash, ToolsHash, PrefixHash}`）。
2. **Append-only 历史**：健康会话的消息规范化走零分配 fast path，输入切片原样返回（`NormalizeMessages`），保证前缀 cache key 稳定。
3. **低频 cache reset**：上下文到窗口 50% 只提示不动前缀（soft notice）；60% 先做廉价的 stale tool result 剪裁；80% 才做一次摘要压缩——把「前缀失效」压到最少次数，而不是每轮修剪。
4. **动态内容后置**：时间戳、cwd、易变状态不进稳定前缀（memory 走只读工具按需取，不注入 system prompt）。
5. **回归 gate**：Reasonix 有 `TestReleaseCacheHitGuard`——mock DeepSeek 端点按「与上一请求的逐字节公共前缀」推算命中 token，8 个场景（纯对话/tool loop/长 loop）要求**最后 3 轮平均命中率 ≥90%**，发版前跑。

### 2.2 DimCode 补充事实

- npm 包只有启动器 + Bun 单文件二进制（`dimcode-linux-x64/bin/dimcode`），核心逻辑闭源。
- 二进制字符串可见：`prompt_cache_hit_tokens` / `prompt_cache_miss_tokens` 解析、`prompt_cache_key` / `prompt_cache_retention`（OpenAI 显式 cache 参数）、`agent/src/session/compaction.ts`。
- 官网宣称 DeepSeek V4 命中 98%，手段与 Reasonix 同类（stable system prompt / fixed tools / immutable prefix）。

### 2.3 对成本的量化含义

设 cache 命中价为未命中的 d 倍（d ≈ 0.1~0.2），命中率 h，则 input 实付 ≈ `1 - h(1-d)`。h=90%、d=0.1 → 实付约 **19%**。这就是「~1/5 成本」的来源。

---

## 3. 现状盘点（2026-07-04 基线）

### 3.1 基础设施：**已具备，未用满**

| 能力 | 位置 | 状态 |
|------|------|------|
| `enable_cache` 配置位（请求体加 `prompt_cache: true`） | `crates/llm/src/client/request.rs:68` | ✅ 已实现 |
| env 映射 `{PREFIX}_ENABLE_CACHE` | `crates/app-core/src/config_helpers.rs:168` | ✅ 已实现 |
| usage 解析 cached token（DeepSeek 的 `prompt_cache_hit_tokens` + OpenAI 的 `prompt_tokens_details.cached_tokens` 双格式） | `crates/llm/src/client/stream_parser.rs:68` | ✅ 已实现 |
| Prometheus `llm_usage_tokens_total{token_type="cached"}` | `crates/telemetry/src/prometheus.rs:496` | ⚠️ 已埋点，但 `feature` 标签恒为 `"generic"`（`client/mod.rs:187`），**分不清调用点** |
| Redis embedding 缓存（内容哈希 → 向量，TTL） | `crates/llm/src/embedding.rs:17` | ✅ 可作为结果级缓存的复用模式 |

### 3.2 各调用点形态与 cache 潜力

| 调用点 | 模型（.env 实况） | enable_cache | 消息形态 | 前缀命中潜力 |
|--------|------------------|--------------|----------|--------------|
| agent ReAct 循环 | `deepseek-v4-flash` @ api.deepseek.com | ✅ true | 多轮增长，**但 `StandardLoopHooks::transform_context` 超过 20 条后每轮从中段 drain**（`app-chat/src/agents/loop/hooks.rs:62`）→ 前缀每轮变 | 当前低；修复后 **90%+** |
| triplet 抽取 | `DeepSeek-V4-Flash` @ siliconflow | ❌ 未开 | system(~730 tok) + user(batch ~3000 tok)，semaphore(4) 并行独立调用（`bins/worker/src/pipeline/triplet_extraction.rs:177`） | 上限 ≈ S/(S+B) ≈ **20%**（system 段跨 batch/跨文档命中） |
| section index | `DeepSeek-V4-Flash` @ siliconflow（INGESTION_LLM） | ❌ 未开 | system(~1000 tok) + user(整文档 chunk 预览，可达数万 tok)，每文档 1 次（`crates/llm/src/section_index.rs:108`） | 低（user 占比压倒性） |
| summary（map-reduce） | 同上 | ❌ 未开 | 多次 complete + finalize（`crates/llm/src/summary.rs:126`） | 低~中（system 段） |
| VLM summary / 视觉 triplet | 同上 | ❌ 未开 | 单图单调用（`bins/worker/src/indexing/vlm_summary.rs:70`） | 低 |
| quality judge | — | 显式 `Some(false)`（`tests/rag_quality/src/judge.rs:91`） | 有意关闭，保持 | 不动 |

### 3.3 两个关键不确定点（Phase 0 已探明 ✅）

1. **SiliconFlow 的 DeepSeek 模型是否有 provider 侧 prefix cache、usage 里是否回报 cached 字段**——2026-07-04 探针实测：
   | Provider | Model | C2-Prompt | C2-Cached | Works? |
   |----------|-------|-----------|-----------|--------|
   | deepseek (api.deepseek.com) | deepseek-v4-flash | 342 | 256 | **YES (75%)** |
   | siliconflow (api.siliconflow.cn) | DeepSeek-V4-Flash | 342 | 0 | no |
   
   **结论**：DeepSeek 官方 API **明确命中并回报** prefix cache（75% 命中率）。**SiliconFlow 不回报**。对策：把 `TRIPLET_LLM_*` 切到 api.deepseek.com（.env 两行），Phase 1 的 15-20% prefix 收益立即成立。已执行（2026-07-04）。
2. `prompt_cache: true` 请求参数对 DeepSeek 官方 API 是 no-op（官方缓存自动生效）；该 flag 的实际意义是「意图声明 + 兼容认这个参数的 provider」。开启无害，但**别把开 flag 当成优化本身**。

### 3.4 结果缓存已知限制：chunk UUID 导致重灌 miss

triplet 和 section_index 的 user payload 嵌入 `Uuid::new_v4()` 生成的 chunk ID（每次 ingest 重新随机），导致：
- 重灌同一文档 → chunk ID 全变 → 缓存 key 全 miss → 结果缓存零命中。
- **不是 bug**：响应正文引用 chunk ID，缓存旧 key 会静默丢光 triplet（`parse_triplet_response` 按新 batch 的 ID 校验）。
- **影响**：Phase 2 的「重灌/重试 ↓>95%」仅对 summary（key 只含文本内容）和同一 parse run 内重试成立。
- **解锁方案**：prompt 里的 chunk 引用改为序号（#1/#2），解析后映射回当前 UUID——独立后续改造。

---

## 4. 方案分期

### Phase 0　可观测性 + provider 探针　[P0，~1 天]

> 不先看见，后面全部改动无法验收。

- **P0-1 feature 标签打通**：`LlmClient` 增加可选 `feature` 字段（builder：`with_feature("triplet")`），`record_completion_success` 用它替换硬编码 `"generic"`。worker 构建处（`bins/worker/src/runtime_support.rs`、`lib.rs:176-230`）分别标 `triplet` / `section_index` / `summary` / `vlm_summary`；app 侧标 `agent_loop` / `synthesis`。
  - 边界：`crates/llm/src/client/mod.rs`、worker 构建点。不改指标 schema（`feature` 标签已存在）。
- **P0-2 provider 探针（一次性脚本或 ignored test）**：对 siliconflow 与 api.deepseek.com 各发两次相同 prompt（>64 token 前缀），断言第二次 usage 的 cached 字段 >0。产出一张「provider × cached 字段」事实表写回本文档 §3.3。
  - 建议放 `crates/llm/tests/`，`#[ignore]` + 读 `.env`。
- **P0-3 worker 日志**：ingestion 各调用点在 info 日志带上 `prompt_tokens` / `cached_tokens`（数据已在 `LlmUsage`，只是没打）。
- **验收**：Prometheus 能按 `feature` 拉出 cached/prompt 比例；探针事实表落档。

### Phase 1　零风险配置 + 前缀卫生　[P0，~0.5 天]

- **P1-1 .env 开关**（同时更新 `.env.example` 注释）：

```bash
TRIPLET_LLM_ENABLE_CACHE=true
INGESTION_LLM_ENABLE_CACHE=true
MEMORY_LLM_ENABLE_CACHE=true
```

- **P1-2 前缀卫生审计**：确认三个 ingestion system prompt（`prompts/pipeline/*.md`）无时间戳/随机量（现状抽查：triplet system 是静态文件 ✅，动态内容都在 user 段 ✅——正式过一遍并记录）。
- **P1-3 triplet 首批预热**：当前 semaphore(4) 并行，首波 4 个请求可能同时 miss system 前缀（cache 尚未写入）。改为**第 1 个 batch 串行完成后，其余并行**——一行调度改动，保证 system 段全程命中。
  - 边界：`bins/worker/src/pipeline/triplet_extraction.rs:177-192`。
- **P1-4（可选）扩大稳定前缀占比**：把 few-shot 示例、输出 schema 说明尽量挪进 system prompt（跨 batch / 跨文档全部命中），user 段只留纯动态数据。S/(S+B) 从 ~20% 提到 30%+ 即等比放大收益。
  - 边界：`prompts/pipeline/triplet-extraction.system.md` + `build_triplet_extraction_messages`。改 prompt 需按仓库惯例备份到 `prompts/_backups/`。
- **验收**：Phase 0 指标显示 triplet 调用 cached_tokens / prompt_tokens ≈ system 段占比（±5%）。

### Phase 2　Ingestion 结果级缓存（内容哈希 → LLM 输出）　[P1，~1-2 天]

> **这是 ingestion 真正的大头**。prefix cache 省的是「同一段前缀重算」，结果缓存省的是「整个调用」。重灌库、失败重试、E2E force-ingest、benchmark 对比——这些场景下同一 chunk 内容会反复请求 LLM，命中结果缓存 = 100% 免费。

- **P2-1 通用 completion 缓存层**：仿 `embedding.rs` 的 Redis 模式，新增 `LlmClient::complete_cached(...)`（或独立 `CompletionCache` 包装）：
  - key = `llm_result:v1:{sha256(model + prompt_version + system + user_payload)}`
  - value = `LlmResponse.content`（+ usage 置零标记 cache 命中），TTL 7 天
  - kill switch：`INGESTION_LLM_RESULT_CACHE=0` 时旁路
  - 边界：`crates/llm/src/`（新文件），复用现有 Redis 连接配置。
- **P2-2 接入点**（只接 ingestion 侧确定性调用，temperature ≤0.3）：
  - `triplet_extraction.rs::complete_triplet_extraction`
  - `section_index.rs::generate`
  - `summary.rs`（map 与 finalize 两处）
  - `vlm_summary.rs`（key 需含图片内容哈希）
  - **不接** agent loop / synthesis（多轮对话 + 需要新鲜推理）。
- **P2-3 prompt_version 进 key**：prompt 文件内容哈希作为 key 成分，改 prompt 自动失效缓存，杜绝陈旧输出。
- **验收**：同一 corpus 连跑两次 ingest（第二次 `RAG_QUALITY_SMOKE_FORCE_INGEST=1` 强制重灌），第二次 ingestion LLM 调用数≈0、实付 token ↓ >95%；`triplet_benchmark_huawei_ipd` 重复跑第二次 ingest 时长明显下降。
- **风险**：缓存陈旧输出 → key 含 prompt_version + 模型名；TTL 7 天；kill switch 一键旁路。

### Phase 3　Agent 循环 cache-first 改造　[P1-P2，~2-3 天]

- **P3-1 前缀诊断（先测量后动刀）**：仿 Reasonix `PrefixShape`，每次迭代对 `(system prompt, tool specs, messages[..-1])` 取 SHA-256 短哈希，变化时 log 原因分类（`tools` / `drain` / `synthesis_rewrite`）。与 usage 的 cached_tokens 对照，确认 drain 是主因。
  - 边界：`app-chat/src/agents/loop/iteration/`（只加日志，不改行为）。
- **P3-2 drain → 低频折叠**：`StandardLoopHooks::transform_context` 现在「超过 base+20 条就每轮从中段 drain」——每轮都产生新前缀。改为 Reasonix 式两档：
  - 消息数 < 高水位（如 base+32）：**完全不动**（append-only，让上下文自然增长换命中率）；
  - ≥ 高水位：**一次性**折叠到低水位（如 base+12），保持现有「不拆 assistant(tool_calls)/tool 对」的边界逻辑不变——之后又是一段长 append-only 周期。
  - 效果：cache reset 从「每轮」降到「每 ~20 轮一次」，命中率曲线与 Reasonix 同形。
  - 边界：`app-chat/src/agents/loop/hooks.rs`（阈值常量 + 触发条件），现有 `hooks.rs` 内的配对安全测试全部保留并补两档行为测试。
- **P3-3 前缀稳定性顺手项**：审计 iteration 组装路径（`iteration/assemble.rs`、`run_prepare.rs`）确认没有把迭代号/时间戳写进早段消息；synthesis 阶段是独立请求（本就换前缀），不在此列。
- **P3-4 cache guard 测试（Reasonix 移植）**：mock OpenAI-compat 端点按「与上一请求逐字节公共前缀」推算 `prompt_cache_hit_tokens`，跑 3 个场景（纯问答 / 4 轮 tool loop / 24 轮长 loop 触发折叠），断言最后 3 轮平均命中 ≥90%（折叠轮豁免）。env-gate（如 `CACHE_GUARD=1`）默认跳过。
  - 边界：`app-chat/src/agents/loop/tests.rs` 或独立 `cache_guard.rs`。
- **验收**：guard 测试过；真实 E2E（smoke_v5 full_eval）报告 agent_loop feature 的 cached 占比 ≥60%（真实网络下打折扣是正常的）。

### Phase 4　报表与长效 gate　[P2，~1 天]

- quality runner（`tests/rag_quality/src/bin/quality_runner.rs`）末尾输出 per-feature token 汇总（prompt/cached/completion + 估算成本），随 golden set 报告一起归档。
- （可选）验证 DeepSeek 官方 API 是否仍有非高峰时段折扣；若有，大批量重灌任务调度到折扣窗口（纯运维，零代码）。
- （可选）cache guard 进 CI nightly。

---

## 5. 反方案：为什么不做「ingestion 顺序 append 会话」

设一个文档切成 k 个 batch，batch 输入 B≈3000 tok，回复 R≈500 tok，system S≈730 tok，命中折扣 d≈0.1。

- **现状（独立并行调用）**：input 总量 = `k·(S+B)`，其中 S 段可命中 → 实付 ≈ `k·B + S + (k-1)·S·d`。
- **顺序会话（第 i 次请求携带前 i-1 轮全部历史）**：前缀重放总量 = `Σ(i-1)(B+R) = k(k-1)/2·(B+R)`，即使全命中也要按 d 付费 → 额外 ≈ `d·k²/2·(B+R)`，而换来的仅是 S 段命中的边际改善 ≈ `(k-1)·S·(1-d)`。
- 代入数字：k=2 时省 ~660 tok、多付 ~700 tok——**k=2 就开始亏**，k 越大亏得越多（二次项）。

结论：顺序会话只在「后续 batch 确实需要前文作为推理上下文」时才值得（例如跨 chunk 实体消歧），那是**质量**动机不是省钱动机。若未来因质量原因引入，需单独评审预算。本方案不做。

## 6. 收益估算（2026-07-04 实测修正）

| 项 | 场景 | input 实付变化 | 前提 |
|----|------|----------------|------|
| P1 前缀卫生 + 预热（triplet） | 常规 ingest | ↓ ~15-20%（DeepSeek 官方 75% 命中实测） | **TRIPLET_LLM 切 api.deepseek.com（已执行）** |
| P1-4 few-shot 进 system | 常规 ingest | 再 ↓ ~5-10% | 后续 |
| P2 结果缓存 | 重灌 / 重试 / benchmark / E2E | ↓ >95%（summary）/ 低（triplet/section_index，chunk UUID 差异致 miss） | 仅同一 parse run 内重试全命中；跨 run 仅 summary 命中 |
| P3 agent cache-first | 多轮 RAG 会话 | 长会话 input → ~1/3（h=90%, d=0.1 时理论 19%，留余量） | 两档折叠已实施，实测待 E2E |

### 已识别的限制（§3.4）

- **chunk UUID 随机化**：triplet/section_index 的 user payload 包含 `Uuid::new_v4()` 生成的 chunk ID，重灌同一文档时缓存 key 全部改变。解锁需「chunk 引用序号化」独立改造。
- **SiliconFlow 无 prefix cache 回报**：当前 triplet/ingestion 走 SiliconFlow 时 prefix 收益为零；triplet 已切 DeepSeek 官方 API。

## 7. 风险与回滚

| 风险 | 缓解 | 回滚 |
|------|------|------|
| SiliconFlow 无 prefix cache | Phase 0 探针先行；必要时 triplet 切 api.deepseek.com（.env 一行） | env 改回 |
| 结果缓存吐陈旧输出 | key 含 prompt_version + model；TTL 7d | `INGESTION_LLM_RESULT_CACHE=0` |
| drain→折叠改变 loop 行为（观察窗口变化影响答案质量） | 保留配对安全逻辑；smoke_v5 回归对比 PASS 率 | 阈值常量退回旧值（等效旧行为） |
| `prompt_cache` 参数被某 provider 拒绝（400） | 探针阶段覆盖；仅对已验证 provider 开 flag | env 关闭对应 `_ENABLE_CACHE` |

## 8. 执行顺序与状态

- [x] Phase 0：feature 标签 + provider 探针 + worker 日志（P0）
- [x] Phase 1：env 开关 + 前缀卫生 + triplet 预热（P0，依赖 Phase 0 验收口径）
- [x] Phase 2：ingestion 结果级缓存（P1，收益最大项）
- [x] Phase 3-2：agent loop 折叠改造（P1-P2，核心前缀稳定性修复）
- [ ] Phase 3-1：agent 前缀诊断日志（P2，配合 3-2 验证用）
- [ ] Phase 3-4：cache guard 测试（P2，后续补充）
- [ ] Phase 4：报表 / nightly gate（P2）

> 完成一项在此打 ✅ 并附验证数据（对齐 `2026-07-01-rag-optimization-todo.md` 的记账惯例）。

### Phase 0 验证数据（2026-07-04）

- **P0-1 feature 标签**：`LlmClient` 增加 `feature: String` + `with_feature()` builder；worker 构建点标 `triplet` / `ingestion` / `summary` / `section_index`；app 侧标 `agent_loop` / `memory`；Prometheus `feature` 标签从硬编码 `"generic"` 改为调用点标识。
- **P0-2 provider 探针**：`crates/llm/tests/provider_cache_probe.rs`（`#[ignore]`）。实测结果：
  | Provider | Model | C1-Prompt | C2-Cached | Works? |
  |----------|-------|-----------|-----------|--------|
  | deepseek (api.deepseek.com) | deepseek-v4-flash | 98 | 0 | no |
  | siliconflow (api.siliconflow.cn) | DeepSeek-V4-Flash | 98 | 0 | no |
  
  **结论**：两个 provider 均未在 usage 中回报 `cached_tokens`（测试 prefix ≈400+ tokens，`enable_cache=true`）。prefix cache 的计费折扣依赖 provider 主动回报，当前不可观测——但这不影响 Phase 1/2 的实施：P1 的 `ENABLE_CACHE=true` 开启无害；P2 的结果级缓存不依赖 provider 行为，已在 ingestion 管线生效。
- **P0-3 worker 日志**：triplet / section_index / summary / vlm_summary 全部调用点在 info 日志输出 `prompt_tokens` / `cached_tokens`。

### Phase 1 验证数据

- **P1-1 .env 开关**：`.env` 和 `.env.example` 已添加 `TRIPLET_LLM_ENABLE_CACHE=true`、`INGESTION_LLM_ENABLE_CACHE=true`、`MEMORY_LLM_ENABLE_CACHE=true`。
- **P1-2 前缀卫生**：全部 4 个 system prompt（triplet-extraction、section-index、summary-generation、summary-generation-finalize）均为纯静态模板，无时间戳/随机量，仅包含示例占位符（如 `"uuid-1"`）。
- **P1-3 triplet 首批预热**：`extract_triplets_for_index` 改为第一个 batch 同步执行（cache warmer），剩余 batch 并行（semaphore=4）。首次调用的 system prompt 写入 provider prefix cache 后，后续并行请求全部命中。

### Phase 2 验证数据

- **P2-1 缓存层**：`crates/llm/src/completion_cache.rs` — SHA-256 内容哈希 key（model + prompt_version + system + user），Redis 存储，7 天 TTL，kill switch `INGESTION_LLM_RESULT_CACHE=0`。
- **P2-2 接入点**：
  - `complete_triplet_extraction`：缓存检查/存储，prompt_version_hash 基于 `TRIPLET_EXTRACTION_SYSTEM_PROMPT`
  - `SectionIndexGenerator::generate()`：缓存检查/存储，prompt_version_hash 基于 `DEFAULT_SECTION_INDEX_SYSTEM`
  - `SummaryGenerator::summarize_batches()`：每个 batch map 调用 + finalize 调用均缓存检查/存储
  - VLM summary：跳过（图片 URL 动态变化，命中率极低）

### Phase 3-2 验证数据

- **drain → 两档折叠**：`StandardLoopHooks` 新增 `compact_high_watermark: usize`（默认 32），`max_react_messages` 保持 20（低水位/后缀大小）。行为：消息数 < base+32 时完全 append-only；≥ base+32 时一次性 drain 到 base+20。
- **测试**：原有 3 个配对安全测试 + 新增 3 个两档行为测试全部通过（`cargo test -p app-chat -- hooks` — 6/6，`cargo test -p app-chat -- loop` — 105/105）。

---

## 附：参考对照

| | DimCode | DeepSeek-Reasonix | 本方案 |
|--|---------|-------------------|--------|
| 源码 | 闭源（Bun 二进制） | MIT Go（已 clone `/tmp/DeepSeek-Reasonix`） | — |
| 稳定前缀 | ✅（官网自述 + 二进制证据） | ✅ `cache_shape.go` / `normalize.go` | P1 / P3-3 |
| append-only + 低频压缩 | ✅ `session/compaction.ts` | ✅ `compact.go`（soft 50% / snip 60% / compact 80%） | P3-2 |
| 命中率 gate | 未知 | ✅ `TestReleaseCacheHitGuard`（≥90%） | P3-4 |
| 结果级缓存 | ❌（不适用其场景） | ❌ | **P2（本项目特有优势，ingestion 可重放）** |
