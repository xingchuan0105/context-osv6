# Ingestion 提速改造：Qwen Responses 合一会话 + struct_tables 纯代码化 + Pandoc docx 源头控制

| 项目 | 内容 |
|---|---|
| 类型 | 设计决策（ingestion LLM 阶段提速） |
| 日期 | 2026-08-03 |
| 范围 | ingestion LLM 换 Qwen Responses API 合一会话；struct_tables 去 LLM 化；docx 表格源头控制 |
| 分支 | 本地 `master`（solo trunk） |
| 前序 | `docs/plans/2026-08-02-parser-pipeline-direct-readers.md`（解析管线路由）；`docs/engineering/2026-07-30-full149-process-budget-handover.md` |
| 状态 | 方案已定；**struct-supervision 的 `supervise_code_only` 已落地（工作区，未提交，runner.rs:107）**，worker 接线待做 |
| 触发 | 全量 149 并发 8 首跑暴露：**新解析管线（liteparse/office-direct）解析本身 ~1s，但 ingestion LLM 阶段把单文档管道拖到 ~9min**，10 个原件全灌约 1h+，生产不可接受 |

---

## 0. 一句话结论

**ingestion 的 LLM 重活是唯一瓶颈，且多数不该由 LLM 干：**

1. **struct_tables 彻底去 LLM**：表格已由解析器产出规整 markdown，用**自研纯代码 `extract.rs`**（不是 DuckDB markdown 扩展）提取 → `checks.rs` 纯代码判定 → 直接建 DuckDB。砍掉 40 轮 LLM supervision 循环（~2.4min → 秒级）。
2. **docx 表格源头控制**：office-direct 的 docx 路径从 `mammoth + markdownify` 换 **Pandoc `-t gfm`**——修复 markdownify 输出"空表头 → 分隔行 → 真表头"的非标准形态，使所有解析器源头统一为标准 GFM。
3. **三件套合一会话**：ingestion LLM 换 **Qwen3.7-flash（DashScope Responses API，nonthinking）**，每文档一个**有状态多轮会话**（`previous_response_id` 链 + `x-dashscope-session-cache` 会话缓存），首轮塞 chunks 进缓存，同会话续接产出 **summary → profile → triplet**（三件套全进会话，实测缓存命中 ~100%，输入成本降 ~90%）。

---

## 1. 实测证据

### 1.1 ingestion 单文档耗时分布（thesis.docx，office-direct 直读）

| 阶段 | 耗时 | 性质 |
|---|---|---|
| route / parse_validate / ir_project | ~1.0s | 代码（office-direct 直读极快） |
| **struct_tables** | **141.6s (2.4min)** | **LLM 40 轮 supervision 循环** |
| **materialize（含 profile）** | **225.0s (3.8min)** | **LLM 文档画像 `generate_document_profile_with_llm`** |
| struct_line_map | 1.4s | 代码 |
| **summary** | **72.8s (1.2min)** | **LLM 摘要** |
| **index（embedding）** | **89.5s (1.5min)** | embedding API |
| 总管道 | 531s (~8.9min) | 其中 LLM+embedding ≈ 7.7min |

**结论：解析层（我们的新管线）毫秒级，慢的是 LLM 阶段——且 struct_tables 的 2.4min 是"不该有的重"。**

### 1.2 会话 API 实测（2026-08-03）

**Qwen Responses API（最终选型）**：
- `POST https://dashscope.aliyuncs.com/compatible-mode/v1/responses` → **200**，返回 response id
- `previous_response_id` 续接 → **正确**（模型回忆上文）
- **`x-dashscope-session-cache: enable` 会话缓存实测命中**：thesis 全量首轮创建缓存 34426 tokens，续接轮 cached_tokens=34425/35062（**~100% 命中**）
- `reasoning: {effort: "none"}` → reasoning_tokens=0（nonthinking 生效）

**Gemini Interactions API（曾测，否决）**：
- `POST /v1beta/interactions` → 200/2.2s，`previous_interaction_id` 续接正确；模型 `gemini-3.5-flash-lite` 可用
- 否决原因：**成本考量（用户拍板）**——Gemini 付费层会话数据驻留 + 单价高于 Qwen Flash；且实测开 thinking 时 gemini 输出被截断（thought 吞掉 token 预算）
- `GEMINI_API_KEY`（.env）与用户提供 key 一致；Gemini 直连 base_url 已被 `route/client.rs` `detect_protocol` 识别

### 1.3 Pandoc docx 转换实测

| 项 | 结果 |
|---|---|
| `pandoc thesis.docx -t gfm` | ✅ 0.42s，162KB 输出 |
| 表格形态 | ✅ **标准 GFM**（表头 → `|---|` → 数据） |
| 对比 markdownify | ❌ 输出"空表头行 → 分隔行 → 真表头"（非标准，DuckDB 扩展/标准解析器误判） |
| DuckDB 扩展解析 Pandoc 输出 | ✅ 23 个 table block，headers/rows 全对 |
| 宽表（7 列）是否踩 GFM 限制 | ✅ 未踩 |

> **口径澄清（13 vs 23）**：§2.1 的"extract.rs 13 表"与这里的"23 个 block"**不是矛盾**——DuckDB 扩展把跨页续表拆成独立 block（同表头 2-3 个 block），而 `extract.rs` 用 `merge_continuations` 按表头签名合并为 13 个 grid（行数一致：如"措施/内容/影响" DuckDB [4,4] 行 = extract.rs 8 行）。**两次测试同源同份 Pandoc 输出，口径不同。**

### 1.4 DuckDB markdown 扩展实测（否决）

| 函数 | 结果 |
|---|---|
| `md_extract_tables_json` | ❌ **表头/数据全空**（bug，标准 GFM 也空） |
| `md_extract_table_rows` | ❌ 表头 cell 全空 |
| `read_markdown_blocks` | ⚠️ 表头正确但**无源码行号**（只有 element_order） |
| `parse_markdown_to_duck_blocks` | ⚠️ 对"空表头行 → 分隔行"形态误判（把空表头当表头、真表头当首数据行） |

> **形态澄清**：office-direct 的 xlsx/pptx 已输出标准 GFM（`|---|` 分隔行，main.py:128,159）；docx（markdownify）**也有分隔行**，但表头行是空串（`|  |  |  |`）——即"空表头行 → 分隔行 → 真表头 → 数据"的非标准形态。`parse_markdown_to_duck_blocks` 对空表头行不识别为表头、把真表头当数据（§3.1 同源）。

---

## 2. struct_tables 去 LLM：自研纯代码（用户拍板，否决 DuckDB 扩展）

### 2.1 为什么不用 DuckDB markdown 扩展（实测 + 源码对比）

**实测三个致命点：**
1. `md_extract_tables_json` 对标准 GFM 也表头/数据全空（扩展 bug）
2. `read_markdown_blocks` 解析正确但**不保留源码行号**——struct_query 行级 cite 依赖 `__src_line`
3. docx（markdownify）输出"空表头行 → 分隔行"形态 → `parse_markdown_to_duck_blocks` 误判（空表头当表头、真表头当首数据行）

**源码对比（extract.rs vs cmark-gfm）：**

| 能力 | `extract.rs`（自研） | DuckDB 扩展(cmark-gfm) |
|---|---|---|
| GFM 表格检测 | ✅ 状态机，对齐 markdown-it | ✅ |
| 表体吸收无 `\|` 行 | ✅ | ✅ |
| 跨页续表合并 | ✅ `merge_continuations` | ❌ 拆独立块 |
| 假表头提升 | ✅ `auto_rotate`（空列头/Unnamed） | ❌ 无 |
| 源码行号保真 | ✅ `Row.line` | ❌ 无 |
| 解析器成熟度 | 自研状态机（618 行 + 14 测试） | 社区 cmark-gfm |

**`auto_rotate` 澄清**：不是 pandas 特化硬编码——触发条件是表头含空串或 `Unnamed`，覆盖**docx 跨行合并表头**通用场景（实测 thesis 触发：第一行 `生产规模|各型号设备所需台数|(4空列)`，第二行才给全列名 `4T/H|3T/H|...`）。这是 DuckDB 扩展缺失的能力，不是多余硬编码。

**结论：extract.rs 是功能超集 + 保留行号 + 已实测稳定（Pandoc 标准输出 13 表全覆盖）——用它做入库主力，不引入 DuckDB 扩展。**

### 2.2 改造

**struct-supervision 纯代码入口（已落地工作区，未提交；worker 接线待做）**（`crates/struct-supervision/src/runner.rs`）：
```rust
/// 纯代码监督（零 LLM）：Session::new（checks + rebuild_db）→ finish 兜底终态
/// → build_metas（语义字段 None/"low"）→ write_duckdb → evidence
pub async fn supervise_code_only(
    input: &crate::SuperviseInput,
    cfg: &SuperviseConfig,
) -> anyhow::Result<SuperviseReport> {
    let session = Session::new(input)?;   // 纯代码，无 LLM
    finish(input, session, cfg, 0, None, Vec::new())
}
```
- 语义字段（caption/unit/table_kind/confidence）为 None/"low"，status 由 `checks::table_report` 判定（high_candidate/needs_diagnosis），不 quarantine
- `struct_query.rs` 读 `_meta` 语义字段为 `Option<String>`，缺省 None 不崩（已确认）；caption 语义信号缺失对选表的影响见 §7
- 行号经 `Row.line` → `__src_line` 保留（struct_query 行级 cite / `_line_map` 不受影响）
- 原 `supervise()`（LLM 循环）保留但 worker 不再调用；`finish` 维持私有

**worker `struct_stage.rs`**：`stage_struct_tables` 改调 `supervise_code_only`，去掉 `processor.llm.ingestion_llm` gate；`StructTablesOutcome` 分流照旧（纯代码路径恒 `Rebuilt`，除非 IO 错 → OldKept）

---

## 3. docx 表格源头控制：Pandoc 替换 markdownify

### 3.1 为什么

markdownify（HTML→markdown 轻量库）对 docx 表格输出**非标准 GFM**：`| 空表头 | → |---| 分隔行 → | 真表头 | → 数据`。导致：
- DuckDB 扩展/标准解析器把空表头当表头、真表头当第一数据行（实测 headers 全空）
- 每篇 docx 都依赖 extract.rs 的 `auto_rotate` 兜底（技术债务）

**Pandoc 是专业文档转换器**，`-t gfm` 输出标准 pipe table，且完整解析 docx（合并单元格/嵌套更可靠）。

### 3.2 三层优先级对齐

| 层级 | 落地 |
|---|---|
| **源头控制**（首选） | office-direct docx → Pandoc `-t gfm`（5 个解析器源头全标准 GFM） |
| **后处理修复**（次选） | 对未来不可避免的非标准输入，做统一表格规范化（预留） |
| **灵活解析**（兜底） | `extract.rs` 保留 `auto_rotate` 等，仅兜底异常形态 |

### 3.3 改造

**`scripts/office-direct/src/office_direct/main.py` `_extract_docx`**：
```python
def _extract_docx(src: str) -> str:
    # mammoth + markdownify 换 Pandoc -t gfm（标准 GFM 表格输出）
    out = subprocess.run(["pandoc", src, "-t", "gfm"], ...)
    return out.stdout
```
- xlsx/pptx 路径不动（`_extract_xlsx`/`_extract_pptx` 已输出标准 GFM）
- **新依赖**：系统安装 `pandoc`（apt，~100MB+）；worker-dev.md / 部署脚本需记录
- 保留 markdownify 依赖仅在 Pandoc 缺失时回退（或直接移除，倾向移除）

---

## 4. 三件套合一会话（DashScope Responses API · Qwen nonthinking · 会话缓存）

> **2026-08-03 模型选型终定（用户拍板，成本优先）**：从 Gemini 全链改为 **Qwen3.7-flash（DashScope 百炼）**。
> 三件套（summary / profile / triplet）全部进**同一 Responses 会话**（`previous_response_id` 链 + `x-dashscope-session-cache` 缓存）。
> 这**推翻 review 的"triplet 独立并发"决定**——用户明确要求三件套同会话以最大化缓存复用。

### 4.1 现状：4 个独立 stateless LLM client

`LlmDeps`（`processor.rs:118`）有 4 个独立 client——`summary_generator`/`section_index_generator`/`triplet_llm`/`ingestion_llm`，都从 `config.ingestion_llm` 构建，**每次调用 stateless 全量消息，互不共享上下文**。chunk 切分（`chunker.rs`）已纯代码，不动。

### 4.2 目标：每文档一会话（summary + profile + triplet），chunks 进缓存

**技术基础（实测验证 2026-08-03）**：阿里云百炼提供 **OpenAI 兼容 Responses API**（`POST /compatible-mode/v1/responses`），等价 Gemini Interactions：
- **`previous_response_id`** = Gemini 的 `previous_interaction_id`（有状态多轮，服务端管理上下文，响应 id 有效期 7 天）
- **`x-dashscope-session-cache: enable`** 请求头 = 会话缓存（自动缓存对话上下文，命中输入计费 **10%**，TTL 5 分钟，最小 1024 token）
- **`reasoning: {effort: "none"}`** = nonthinking

**实测结果**（thesis 全量 72KB，Qwen3.7-flash）：

| 轮次 | 任务 | cached_tokens | 命中率 |
|---|---|---|---|
| 1（seed chunks） | summary | 0（创建 34426） | — |
| 2（previous_response_id 续接） | profile | **34425** | **~100%** |
| 3（续接） | triplet | **35062** | **~100%** |

> **措辞修正**：会话模式不自动省钱——服务端每轮处理全量上下文并按全量计费，**只有缓存命中的部分才按 10% 计费**。本方案实测命中 ~100%（chunks 前缀不变 + 会话缓存），输入成本降 ~90%。
>
> **缓存键约束（2026-08-03 实测补记，Rust 探针 `crates/llm/tests/dashscope_session_probe.rs`）**：`x-dashscope-session-cache` 的缓存键**包含 `instructions`（system 消息）**——续接轮与 seed 轮 instructions 完全一致才命中（真机 A/B：同 → cached 2217；异/无 → 0）。因此 `DocumentIngestionSession` 的 instructions 恒定为 `INTERACTION_SESSION_SYSTEM`，阶段 system prompt（section-index/summary/triplet）折叠进当轮 user 消息前导块；若按原实现每轮换 system prompt，缓存永远不命中（该 bug 曾被真机探针抓获：cached=0、每轮全价 prompt≈全上下文）。探针终态：turn2 cached=2217/2286（~97%）。

### 4.3 改造分块

**A. llm crate：扩展 `openai_responses` 协议支持 DashScope 会话**
- 现有 `openai_responses` 协议（`crates/llm/src/protocols/openai_responses/`，已支持 DeepSeek `/v1/responses`）扩展：
  - `LlmRequest` 加 `previous_response_id: Option<String>`；`LlmResponse` 加 `response_id: Option<String>`
  - request.rs：DashScope 时加 `previous_response_id` + `reasoning: {effort: "none"}`
  - transport/header：DashScope 时加 `x-dashscope-session-cache: enable`
- `ApiStyle` 加 `DashScopeResponses`（`from_config_str "dashscope_responses"`）
- `route/client.rs`：`api_style == DashScopeResponses` → 复用 openai_responses 路由 + 会话 header
- `LlmClient::complete_response(prev_id, messages, temperature) -> (LlmResponse, Option<String>)`

**B. worker：`DocumentIngestionSession`**（`bins/worker/src/pipeline/ingestion_session.rs` 新增）
- 持 `Arc<LlmClient>` + `previous_response_id: Option<String>` + 文档上下文
- `seed_chunks(chunks)`（首轮塞 chunks + 产 summary）→ `produce(system, user, temperature) -> String`（续接 + 更新 response_id）
- `LlmDeps` 收敛为 ingestion_llm（会话载体）+ completion_cache

> **⚠️ completion_cache × 会话链互斥（必须定策略）**：现有 summary/profile/triplet 都走 `CompletionCache`（result-level，Redis hash）。**缓存命中时跳过 LLM 调用 → 拿不到 response_id → 后续轮 `previous_response_id` 链断裂。**
>
> 二选一，**本期采用「会话路径绕开 result cache」**：`DocumentIngestionSession` 的 `produce` 不查 `CompletionCache`，会话链恒定连续；result cache 继续服务非会话路径（VLM 摘要等）。理由：result cache 收益是"重灌/重试零 token"，会话收益是"同文档多轮复用前缀"——两者都想要时缓存命中那轮会断链，取舍取链。

**C. 阶段接入**（`document_pipeline/mod.rs` + `materialize.rs` + `profile.rs` + `triplet_extraction.rs`）
- **三件套同会话**（用户拍板，推翻 review 的 triplet 独立）：materialize 里 profile 改 session（seed chunks → 产 profile）；summary 阶段改 session 续接；index 阶段 triplet 改 session 续接
- **⚠️ triplet 并入会话的代价**：现 `Semaphore(4)` 并发批次改会话内串行——N batch × 每批延迟相加。但缓存命中省下输入成本；实测单会话三件套 3 轮全部命中，速度可接受（triplet 单轮 2.5s）
- `ParseRunState` 或局部持 session 跨 materialize→summary→index 传递

### 4.4 配置（.env）

```
INGESTION_LLM_BASE_URL=https://dashscope.aliyuncs.com/compatible-mode/v1
INGESTION_LLM_API_KEY=<DASHSCOPE_API_KEY 值>
INGESTION_LLM_MODEL=qwen3.7-flash
INGESTION_LLM_API_STYLE=dashscope_responses
INGESTION_LLM_TIMEOUT_MS=60000
INGESTION_LLM_ENABLE_THINKING=false   # reasoning: {effort: "none"}
```
- `TRIPLET_LLM_*` 并入会话后不再独立（或保留作为 fallback）

---

## 5. 关键文件

| 文件 | 改动 |
|---|---|
| `crates/struct-supervision/src/runner.rs` | `supervise_code_only` 已落地（工作区未提交）；`lib.rs` 导出待补 |
| `bins/worker/src/pipeline/document_pipeline/struct_stage.rs` | 改调 `supervise_code_only`，去 LLM gate |
| `scripts/office-direct/src/office_direct/main.py` | docx 换 Pandoc `-t gfm` |
| `crates/llm/src/protocols/openai_responses/request.rs` | 扩展：DashScope 加 `previous_response_id` + `reasoning:{effort:"none"}` + session header |
| `crates/llm/src/protocols/mod.rs` / `lib.rs` / `route/client.rs` / `client/mod.rs` / `schema/{messages,events}.rs` | ApiStyle `DashScopeResponses` + route + `complete_response` + 字段 |
| `bins/worker/src/pipeline/ingestion_session.rs` | **新增**会话载体 |
| `bins/worker/src/pipeline/{processor,lib}.rs` | LlmDeps 收敛 |
| `bins/worker/src/pipeline/document_pipeline/{mod,materialize,profile}.rs` | 阶段接会话 |
| `bins/worker/src/pipeline/triplet_extraction.rs` | **并入会话**（改串行续接） |
| `prompts/pipeline/interaction-session.system.md` | **新增**（会话引导措辞；summary/profile/triplet 复用现有 include_str! prompts） |
| `.env` | INGESTION_LLM_* 指向 DashScope Responses |
| `docs/runbooks/worker-dev.md` / 部署脚本 | Pandoc 依赖记录 + DashScope 会话数据驻留说明 |

---

## 6. 验证

1. **struct-supervision 单测**：`cargo test -p avrag-struct-supervision --lib`——`supervise_code_only` 无 finals 纯代码建表（duckdb 表 + evidence + FTS + 行号）
2. **llm 单测**：`cargo test -p llm --lib`——DashScope Responses 协议 body/响应解析/response_id 往返/ApiStyle 路由
3. **office-direct 单测**：docx → Pandoc 输出标准 GFM（表格段落断言）
4. **真实 Qwen 探针**（Rust 侧）：`complete_response` 首轮 + 续接，确认 response_id 往返 + 内容正确 + **`x-dashscope-session-cache` 长 chunks 前缀 cached_tokens > 0 与否**（已 Python 实测命中，补 Rust 侧确认）
5. **单文档 E2E**：`E2E_QUESTIONS=1..4`，看 thesis ingestion 速度（目标 <2min）+ struct_tables 纯代码产物 + docx 表格正确 + **正文无 `media/` 死引用**
6. **全量 149 并发 8**：对照 PASS 137/149 基线

---

## 7. 风险与决策点

| 风险 | 说明 | 应对 |
|---|---|---|
| **缓存命中** | 已 Python 实测 `x-dashscope-session-cache` 命中 ~100%（34425/35062） | §6.4 Rust 侧补确认；若不命中，会话模式与 stateless 成本打平（§4.2 措辞） |
| **会话链上限** | `previous_response_id` 响应 id 有效期 7 天；长链上限未公开 | §6.4 探针加"全量 chunks 单链跑完三件套"维度；若触发，改为每文档新建会话（不跨 summary/profile 续接），损失部分前缀复用 |
| **数据驻留** | DashScope 会话缓存存储对话上下文；用户文档全文进会话 | 治理决策：可接受则写明；不可接受则 session 缓存只对 chunks 前缀（摘要/画像产出仍本地落库）——**本期默认接受，worker-dev.md 明示** |
| **prompts-in-md 合规** | seed/续接轮措辞是新增 LLM 文案，须落 `prompts/**/*.md`（仓库硬规则） | §5 文件清单补 prompts 条目：新增 `prompts/pipeline/interaction-session.system.md`（会话引导），summary/profile/triplet 复用现有 include_str! prompts |
| **completion_cache × 会话链** | result cache 命中跳过 LLM → 拿不到 response_id → 链断 | §4.3 B 已定：会话路径绕开 result cache |
| **visual triplet × completion_cache** | page_raster VLM 抽三元组**不在** session 链上（独立 complete 调用） | **接受** result-level `completion_cache`（重灌去重）；与 session 链互斥策略不冲突（2026-08-03 收口拍板） |
| **summary batch** | 大文档 summary 现拆多批 + finalize | 单会话内多轮续接组织方式按实测调 |
| **Pandoc 依赖** | 新增 apt 系统依赖 | 记录到 worker-dev.md / 部署脚本 |
| **struct 语义字段 None** | caption/unit/table_kind 为空 | **查询侧不崩，但 caption 语义信号缺失**：多表文档选表依赖 DESCRIBE + 样例行，选表准确率可能降。如观测到选表退化再单独加轻量标注 |
| **embedding 成新瓶颈** | `<2min` 目标里 parse 1s + line_map 1.4s + embedding 89.5s 已占 ~92s，LLM（summary+profile）只剩 ~28s 预算 | 本轮不动 embedding，但它是下一个优化对象（§7 记录） |
| **`supervise()` 死代码** | LLM 循环保留但不再调用 | 本期保留（可回退）；后续清理 |
| **Pandoc 非表格影响** | Pandoc 换的是整个 docx 转换器：mammoth 主动丢图（`img_element → src:""`），Pandoc 会产 `![](media/imageN.png)` 死引用进 chunk；脚注/标题/列表形态也变 | §6.5 E2E 加"正文无 `media/` 死引用"断言；实现时可能需要 strip 图片语法 |

---

*完。下一棒：按 §2→§3→§4 顺序实施；每块单测绿后进下一步。*
*待办同步：struct_stage 去 gate 后，`StructTablesOutcome::Skipped` 注释（"LLM 缺失"）会过时，实施时同步扫注释。*
