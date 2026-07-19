# 状态：黄金测试集（Orchestrator 新范式 + 能力分组）扩充任务

**日期:** 2026-07-19（当日多次更新，覆盖至 r7 + q1–q65 复盘）  
**状态总览:** **v4 已提交（143 题 + 3 篇语料）；runner 已硬化为「语料只灌一次 + 每题失败即停 + 断点续跑」；r8 排程 2026-07-20 01:12（配额恢复后）从 q66 续跑；q1–q65 复盘完成，核心疑点是 RAG worker 取证系统性失效（见 §5）**

---

## 1. 硬规则（本线铁律，后续会话必须遵守）

### 规则 1：语料只灌一次，默认复用，禁止重新灌库

- 语料缓存在 `avrag-rs/crates/app/tests/e2e_output/realistic_corpus_cache.json`（`workspace_id` + `docs:{filename: doc_id}`，**渐进式**：每灌成一篇立即落盘）。
- runner（`realistic_corpus_full_eval`）**默认复用缓存**：有缓存的文档直接跳过上传；只灌缺失的。**只有 `E2E_FORCE_INGEST=1` 才允许全量重灌**（换语料时才用）。
- 灌库设施是**持久的**：PG = `avrag_rs_e2e_smoke`（`postgres://avrag:avrag@127.0.0.1:5432`）、object store = `tests/e2e_output/realistic_object_store`、Milvus 向量由 `E2E_PRESERVE_MILVUS_ON_DROP=1`（runner 内置设置）保留。**禁止恢复 teardown 删向量行为**。
- 理由：灌库 = 每篇 LLM 画像 + 摘要 + embedding 的真金白银；2026-07-19 一天内 5 轮全量重灌直接烧穿测试身份滚动 5h 配额（429，`retry_after_secs=7756`）。
- 复用前必须验证向量在场（PG 有 chunks ≠ 有向量；查 Milvus `avrag_e2e_00000000_rag_text_chunks` 的实体）。

### 规则 2：每题失败即停，从失败处续跑

- 一律带 `E2E_FAIL_FAST=1`（chat 错误 / 解析错误 / expect_citations 不达标，任一即停，尾部 panic 打出首败详情）。
- 续跑用 `E2E_START_AT=<失败题号>`（1-based），已通过的题不重跑。
- 失败证据在 `avrag-rs/crates/app/tests/e2e_output/realistic_corpus_full_eval/qNNN.json`（答案/引用/scope/caps/mode_debug.dispatches）与 `qNNN.raw.json`（解析失败时的原始响应）。**先读证据再动手**，不允许凭猜修复。
- 修复必须提交 commit 后再重启；日志写到 `output/full_eval_smoke_rN+1.log`。

### 规则 3：配额是信号，不是敌人

- 撞 `quota_exceeded`（429）时按 `retry_after_secs` 排程，**禁止盲目重试或换身份绕配额**——429 正在强制执行规则 1。
- 当前配额窗口：2026-07-19 22:52 撞限，01:04 恢复。

---

## 2. 运行手册（照抄即可）

### 常规续跑（默认路径）

```bash
cd avrag-rs && E2E_MODE=nightly E2E_SKIP_NETWORK_CASES=1 E2E_FAIL_FAST=1 \
  E2E_START_AT=<题号> \
  cargo test -p app --test product_e2e realistic_corpus_full_eval \
  --features product-e2e -- --ignored --test-threads=1 --nocapture \
  2>&1 | tee ../output/full_eval_smoke_rN.log
```

不需要其他 env。联网题（9 道）先跳过；开网复跑时去掉 `E2E_SKIP_NETWORK_CASES=1` 并确认代理（见 §6）。

### 环境变量速查

| 变量 | 语义 | 默认 |
|---|---|---|
| `E2E_FAIL_FAST=1` | 首败即停 | 必带 |
| `E2E_START_AT=N` | 从第 N 题续跑 | 1 |
| `E2E_FORCE_INGEST=1` | 全量重灌（换语料专用） | 关 |
| `E2E_SKIP_NETWORK_CASES=1` | 跳过 `requires_network` 题 | 先带 |
| `RAG_QUALITY_REALISTIC_TRIPLET_ENABLED=1` | 灌库时抽 triplet（graph 题前置） | 关 |

### 当前排程

| 时间 | 任务 |
|---|---|
| 2026-07-20 01:12 | one-shot `2d9e62c8`：启动 r8（复用 3 篇 + 补灌 7 篇 + `E2E_START_AT=66`） |
| 2026-07-20 01:22 | one-shot `20aed5f2`：确认 r8 起来，重建逐题监控 |

---

## 3. 语料现状（每次动手前先核对）

### 可复用（PG chunks + Milvus 向量均已验证，已播种进缓存）

| 文件 | doc_id |
|---|---|
| thesis_y_refrigeration.txt | `bc81ace5-bb3a-44f1-854c-cb99bd85fa63` |
| huawei_ipd_370_activities.txt | `7481adae-a8ea-4e10-87af-adc2c28cd4de` |
| baiyao_it_planning.txt | `04acb106-ba6a-4a8c-9266-32c5b4b607a9` |

### 待补灌（r8 执行，7 篇）

adr-0004 / adr-0009 / consulting_platform / consulting_compensation / **consulting_rbf_drc / consulting_prepared_food / consulting_craftsman_paradox**（后三篇为 v4 新语料，docx 原件 + txt 抽取版在 `product_e2e/fixtures/`，已随 v4 提交）。

注意：持久库里还有 adr4/adr9/platform/compensation 的旧 doc（workspace `7b525929`）——**有 PG chunks 但向量已被旧轮删除，不可复用**，重灌会产生重复 doc（可容忍，retrieval 按 doc_scope 钉死）。

## 4. 集与 runner 版本

- 集：`golden_set_realistic.json` v`4.0.0-orchestrator-groups`，**143 题 / 17 子集**，commit `8e3e76e`
- runner 关键能力（全部已提交）：
  - 逐题 dump（qNNN.json，含 `mode_debug.dispatches` = 编排器派发记录 status/item_count）
  - 解析失败落盘 qNNN.raw.json（字符安全）
  - `expect_citations{min_doc,min_web}` 真门（引用计数来自 response.citations，非 tool_results）
  - 渐进语料缓存 + 默认复用 + `E2E_START_AT` + `E2E_FAIL_FAST`
- commits（时间序）：`8e3e76e` v4 集+语料 → `5011661` assert 修复+FAIL_FAST → `dbe801c` IPD 灌库超时 300s → `f7315a5` dump+codegen 探针 → `281cf20` 字符安全+raw dump → `094d541` 续跑机制 → `e0eb01f` 默认复用 → `fc70f30` 持久设施+渐进缓存 → `6a9e3b2` dump 加 mode_debug

## 5. q1–q65 复盘结论（r4 轮，rag-only 模式）

### 数字

- 65 题全部 `capabilities:["rag"]`、doc_scope=全 10 篇、非流式
- **10/65 有真实文档引用；55/65 零引用**；标签下限 PASS=7
- 成功案例高度集中：**thesis_synthesis 10 题占 5 题**（引用数也最多）；短 factual 题几乎全军覆没

### 三种失败形态（按恶劣程度）

1. **无视语料的参数知识裸答**（q010 问两家领军企业，论文=烟台冰轮/大连冷冻，答 GEA/JBT）——比拒答更糟
2. **蒙对但无据**（q030 数学对、q045 城市对，零引用）
3. **该答不答**（q005/q064，语料里明明有）

### 已达标的机制

引用纪律（65 题零编造）、adversarial 拒答校准（7/8 干净拒答）。

### 核心疑点（r8 验证点）

- **worker 取证疑似退化为单发弱检索**（无 codegen 多跳精化）——synthesis 长锚点能中、短 factual 全空的分布形态与此吻合；codegen 配置链探针已证存活（`assembled_mode_config_roundtrip_keeps_mandatory_codegen`），但未证实 worker 运行时真的拿到/用到。
- **txt Local 路由 vs docx 全管线**：评测语料是 txt（Local），TOC/profile 生成可能残缺，worker 的 `doc_profile` 定向先天受限；生产（docx，148 chunks）同模式表现好得多。
- **q66 "missing field answer"**：degrade 响应无 `answer` 字段——r8 若复现，读 `q066.raw.json` 看 degrade 结构再定夺（产品 or 口径）。
- 评测口径修正：`chunks=0` 是编排器架构假象（worker tool_results 不随响应走），recall/RETRIEVAL_MISS 标签只作下限；真指标看 citations 与 expect_citations。

## 6. 环境速查

- 测试栈：runner 自带（api+worker+PG `avrag_rs_e2e_smoke`+Milvus 共享实例）；**不要动 dev 栈**（tmux `context-os-dev`）
- 代理（search 题必需）：`http://172.27.240.1:20000`（WSL 网关，网络重置后需更新）
- 加载测试：`cargo test -p rag_quality --lib`（43 绿）；编译检查：`cargo test -p app --test product_e2e --features product-e2e --no-run`
- 相关交接：`docs/engineering/ORCHESTRATOR_HANDOFF_2026-07-18.md`（编排器线，已收尾至 R8）

## 7. 待办（按优先级）

1. r8 起逐题诊断：首个失败题 → 读 dump/mode_debug → 区分 worker 无产出 / chat 未引用 / 语料形态 / 评测口径 → 修复续跑。
2. 验证 worker codegen 多跳是否真实运作（§5 疑点 1）——若是配置/装配丢失，修；若是 prompt 引导弱，改 prompt 层。
3. 跑完非联网题后：开网 + 代理复跑 9 道联网题（joint/search/weather）。
4. P1 空洞（来自初版核查）：toc 题、geo/位置题、稳定 `id` 字段、codegen 通道 SSE 真观测（现为内容门）、graph 题需 triplet 重灌验证。

---

**状态栏**

| 项 | |
|---|---|
| 集版本 | `4.0.0-orchestrator-groups` / 143 题（`8e3e76e`） |
| 灌库 | 3/10 可复用（已缓存）；7/10 待 r8 补灌 |
| 当前阻塞 | 配额窗口（01:04 恢复，r8 @01:12） |
| 核心疑点 | RAG worker 取证系统性失效（§5） |
| 下一动作 | r8 启动确认（01:22）→ 逐题 fail-fast 循环 |
