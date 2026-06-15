# E2E 测试分析框架（TEAF）

> **TEAF** = Test Evidence Analysis Framework（测试证据分析框架）  
> **受众**：Coding Agent、发布验收人、测试维护者  
> **最后更新**：2026-06-13  
> **关联**：[全功能 E2E 指南](full-functional-e2e-guide.md)（测什么）、[E2E 门禁](e2e-gates.md)（过不过）、[`e2e-test-registry.yaml`](e2e-test-registry.yaml)（机读索引）、[`e2e-analyzer`](../crates/e2e-analyzer/README.md)（离线工具）

本文档回答：**跑完测试之后，如何系统性地分析结果、定位根因、判断覆盖与发布就绪度**。  
与 `full-functional-e2e-guide.md` 的分工：

| 文档 | 问题 |
|------|------|
| `full-functional-e2e-guide.md` | 该测什么、怎么跑、并行规则 |
| `e2e-gates.md` | 单层 pass/fail 语义 |
| **本文档 TEAF** | 如何用测试证据做覆盖/回归/归因/稳定性/质量分析 |

---

## 1. 设计原则

1. **证据优先**：结论必须能追溯到具体测试用例 + 断言层 + 产物文件，禁止「感觉覆盖了」。
2. **分层解耦**：Mock 层验证协议与产品契约；真实层验证外部依赖与端到端质量；分析时不得混用门禁。
3. **能力导向**：以产品能力（Chat / RAG / Search / Ingestion / Auth…）为横轴，不以文件目录为横轴。  
   **入库能力再拆两条线**：`CAP-RAG-TXT`（`antifragile.txt` 本地解析 + mock/真实 LLM）与 `CAP-RAG-PDF`（`liteparse_hybrid` + bundled/staging PDF）；不得用 txt RAG 绿线推断 PDF 入库可用。
4. **最小归因链**：失败先判「基础设施 flake → 断言 → 工具 → LLM → 渲染」，与 `e2e-analyzer` 归因优先级一致。
5. **机读 + 人读**：`e2e-test-registry.yaml` 供脚本/Agent 查询；Markdown 供人工决策。

**Staging 入库（非 PR）**：[`scripts/run-staging-ingest-e2e.sh`](../scripts/run-staging-ingest-e2e.sh) — LiteParse PDF、docx/xlsx office-parser、Black Swan Paddle PDF、可选本地大书 llm_real。

---

## 2. 三维模型

所有分析都在三个正交维度上展开：

```
                    ┌─────────────────────────────────────┐
                    │  能力维 WHAT（产品行为）              │
                    │  chat / rag / search / ingest / …   │
                    └─────────────────┬───────────────────┘
                                      │
         ┌────────────────────────────┼────────────────────────────┐
         │                            │                            │
         ▼                            ▼                            ▼
┌─────────────────┐        ┌─────────────────┐        ┌─────────────────┐
│ 证据维 HOW      │        │ 运行维 WHEN     │        │ 依赖维 DEPS     │
│ P→Prod→Q→Obs   │        │ L1…L6 分层      │        │ M/R/I/P         │
└─────────────────┘        └─────────────────┘        └─────────────────┘
```

### 2.1 能力维（WHAT）

与 `full-functional-e2e-guide.md` §2 对齐，共 **12 个能力域**：

| ID | 能力域 | 核心验收信号 |
|----|--------|--------------|
| `CAP-CHAT` | 通用对话 | HTTP 200、`answer` 非空、SSE 契约 |
| `CAP-RAG` | 文档问答 | `citations` 含 `doc_id`、cite 入 answer |
| `CAP-SEARCH` | 联网搜索 | `source_type==web`、`[[n]]` 或 web citation |
| `CAP-INGEST` | 入库解析 | PG `completed`、chunk/summary 存在 |
| `CAP-STREAM` | 流式可观测 | reasoning delta、trace、prompt_snapshot |
| `CAP-MEMORY` | 多轮记忆 | `resolved_query`、memory tool、PG 历史 |
| `CAP-FORMAT` | 格式输出 | 有效 HTML / slides |
| `CAP-AUTH` | 认证边界 | 401/403/404 语义正确 |
| `CAP-SHARE` | 协作分享 | invite token、跨用户只读 |
| `CAP-BILLING` | 计费同意 | consent_required 错误码 |
| `CAP-TENANT` | 租户隔离 | 跨 org 无泄漏 |
| `CAP-DEGRADE` | 降级韧性 | `degrade_trace`、无 hang、可恢复 |

完整 **测试 → 能力** 映射见 [`e2e-test-registry.yaml`](e2e-test-registry.yaml)。

### 2.2 证据维（HOW）

与 `product_e2e/assertions.rs` 三层断言 + 产物对齐：

| 层级 | ID | 含义 | 适用层 | 典型断言 / 产物 |
|------|-----|------|--------|-----------------|
| **Protocol** | `E-P` | HTTP、JSON schema、字段存在、SSE 顺序 | L1–L6 | `assert_http_ok`、`transport-http` 契约 |
| **Product** | `E-Prod` | 业务规则（引文类型、doc_id、degrade） | L1–L3 | `assert_has_citations`、`assert_degrade_trace` |
| **Quality** | `E-Q` | 回答实质性、judge 分数、成本 | L3–L5 | `llm_real` 非空 answer、Playwright judge |
| **Observability** | `E-Obs` | 推理链、工具 trace、prompt 快照 | L2–L3 | `reasoning_summary.txt`、`trace_reasoning.jsonl` |

**分析规则**：  
- PR 门禁只看 **E-P + E-Prod**（mock 环境）。  
- Nightly 发布增加 **E-Q + E-Obs**（真实 LLM）。  
- 单层失败不得用更高层证据「洗白」——例如 mock RAG 过了不能证明真实 RAG 质量。

### 2.3 运行维 + 依赖维（WHEN + DEPS）

| 层 | ID | 依赖图例 | 分析时注意 |
|----|-----|----------|------------|
| L1 smoke | `L1` | M+I | 并行组分裂：RAG 串行 vs 非 RAG 并行 |
| L2 integration | `L2` | M+I(+P) | 必须 `--test-threads=1`；shared fixture 污染 |
| L3 nightly | `L3` | R+I(+P) | `#[ignore]`；凭证缺失 ≠ 产品 bug |
| L4 skills | `L4` | R+I | citation **硬**门禁 |
| L5 journey | `L5` | R+I | search citation 分层（PR 软 / nightly 硬） |
| L6 unit/contract | `L6` | 轻量 | 与 E2E 互补，不替代 |

图例：**M**=Mock LLM/Search/Embedding，**R**=真实 API，**I**=PG/Milvus/Worker，**P**=真实解析管线。

---

## 3. 五分析平面

TEAF 将离线分析拆为五个平面，可单独或组合使用：

```
  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐
  │ 1. 覆盖平面   │   │ 2. 回归平面   │   │ 3. 归因平面   │
  │ Coverage     │   │ Regression   │   │ Attribution  │
  └──────┬───────┘   └──────┬───────┘   └──────┬───────┘
         │                  │                  │
         └──────────────────┼──────────────────┘
                            ▼
              ┌─────────────────────────┐
              │ 4. 稳定性平面            │
              │ Stability / Trends      │
              └────────────┬────────────┘
                           ▼
              ┌─────────────────────────┐
              │ 5. 质量平面              │
              │ Quality / Cost / Judge    │
              └─────────────────────────┘
```

### 3.1 覆盖平面（Coverage）

**问题**：某产品能力是否在正确的层、用正确的依赖测到了？

**输入**：
- `e2e-test-registry.yaml`（期望映射）
- `cargo test --test product_e2e -- --list`（实际枚举）
- `run-product-smoke-e2e.sh --check-modules`（smoke 模块守卫）

**算法（Agent 可手跑）**：

```
对每个 CAP-*：
  required_layers = registry[cap].layers
  actual_tests    = registry 中带该 cap 的 test_id
  for layer in required_layers:
    if 无 test 落在该 layer → GAP(cap, layer, priority)
  if smoke 模块未进 NON_RAG/RAG_SERIAL 列表 → GAP(module_list)
```

**缺口优先级**（与 backlog 一致）：

| 条件 | 优先级 |
|------|--------|
| CAP 在 L1 无 E-P 测试 | P0 |
| CAP 仅有 mock、发布路径需 R | P1 |
| CAP 有测试但无 E-Obs（流式/推理） | P2 |
| CAP 仅 L6 契约、无 E2E | 记录为「契约覆盖」，非缺口 |

**工具**：`e2e-analyzer coverage`（策略/格式维度）；TEAF 能力覆盖以 registry + 指南 §2 为准。

### 3.2 回归平面（Regression）

**问题**：相对基线，行为/性能/成本是否劣化？

**输入**：
- 基线 run：`e2e-analyzer baseline promote --run <dir>`
- 当前 run：同结构 `e2e_output/` 或 `llm_real/<run_id>/`

**指纹维度**（`TestFingerprint`）：

| 维度 | Critical 条件 | 说明 |
|------|---------------|------|
| `status` | passed→failed | 硬回归 |
| `failure_kind` | 类型变化 | 超时常为 infra |
| `duration_ms` | >50% 且 >阈值 | 性能回归 |
| `token_usage` | >30% | 成本回归（llm_real） |
| `retrieval_hits` | 降至 0 | 检索退化 |
| `llm_calls` / `tool_calls` | 计数或 prompt hash 变 | 路由/编排变更 |
| `answer_text` | hash 变 + 测试失败 | 可能 LLM 或渲染 |

**命令**：

```bash
cargo run -p e2e-analyzer -- diff \
  --baseline <baseline_run> --current <current_run> --min-severity minor

cargo run -p e2e-analyzer -- report \
  --run <current_run> --output report.md --format markdown
```

**门禁合成**（`GateStatus`）：

- `critical > 0` → **FAIL**（阻断发布）
- `major > 0` → **WARN**（需人工确认）
- 仅 `minor/info` → **PASS**

### 3.3 归因平面（Attribution）

**问题**：失败根因落在哪一层（工具 / LLM / 检索 / 基础设施 / 断言）？

**决策树**（与 `e2e-analyzer/src/attribution.rs` 一致）：

```
测试失败
├─ tool_calls 含 status=error → ToolFailure（高置信）
├─ diff: LlmCalls + prompt hash 变 → LlmRegression（中）
├─ diff: ToolCalls missing → ToolFailure（高）
├─ diff: duration Critical → LlmRegression 或 InfrastructureFlake
├─ rendering console_errors / answer_html 变 → RenderingIssue
├─ status 回归、其余无 anomaly → TestAssertion
└─ 否则 → Unknown（查 failures/ 产物）
```

**Product E2E 特化线索**：

| 症状 | 优先怀疑 | 证据文件 |
|------|----------|----------|
| `Connection refused` 在 integration 第 N 个用例 | shared fixture / runtime 死锁 | worker_logs.txt |
| RAG smoke 串行模块偶发 fail | Milvus 冷启动 / 并行污染 | `./scripts/e2e-precheck.sh` |
| `reasoning_empty_warning` | SSE trace 丢失 | `metadata.json` |
| `stream_error_with_done` | 流式竞态 | `metadata.json` + response |
| ingest `failed` | office-parser / Paddle mock | failures/response_body.json |
| `SEARCH_REQUIRE_REAL` 失败 | Brave 配额/网络 | llm_real search 产物 |

**命令**：

```bash
cargo run -p e2e-analyzer -- diagnose --run <run_dir> [--test <name>]
```

### 3.4 稳定性平面（Stability）

**问题**：哪些用例 flaky？耗时是否漂移？

**指标**：

| 指标 | 阈值建议 | 动作 |
|------|----------|------|
| `pass_rate` | <80%（近 10 run） | 标为 flake，查 infra 或断言过严 |
| `stddev_duration_ms` | >均值 40% | 查 Milvus/ingest 或加 timeout |
| 交替 pass/fail | 无 diff 关联 | InfrastructureFlake |

**命令**：

```bash
cargo run -p e2e-analyzer -- trends --history crates/app/tests/e2e_output --limit 20
```

**并行组注意**：RAG 串行模块的稳定性应**单独**统计，不与 `G-parallel-smoke` 混合。

### 3.5 质量平面（Quality）

**问题**：真实 LLM 下回答是否可用、成本是否可控、推理链是否完整？

**仅适用于 L3 `llm_real` + L4/L5 Playwright judge**。

**检查清单**：

| 检查项 | 数据源 | 失败含义 |
|--------|--------|----------|
| citation 非空 | `response.json` | 检索或 synthesis 断裂 |
| `usage` 存在 | `metadata.json` | 成本不可审计 |
| reasoning 非空 | `reasoning_summary.txt` 或 `trace_reasoning.jsonl` | 可观测性回归 |
| `prompt_snapshots` ≥1（debug 流） | `prompt_snapshots.json` | debug 路由或未走 agent loop |
| judge score | Playwright `RUN_QUALITY_JUDGE=1` | <6 warn（非阻断） |

**命令**：

```bash
cargo run -p e2e-analyzer -- llm-real list
cargo run -p e2e-analyzer -- llm-real summary --run crates/app/tests/e2e_output/llm_real/<run_id>
```

---

## 4. 产物目录与 Schema

```
crates/app/tests/e2e_output/
├── failures/<run_id>/<test_name>/     # 失败快照
│   ├── response_body.json
│   └── worker_logs.txt
├── observability/<run_id>/<test_name>/ # 轻量可观测（含 mock 路径）
│   ├── response.json
│   └── metadata.json
└── llm_real/<run_id>/<test_name>/    # 实跑 LLM 完整审计
    ├── response.json
    ├── metadata.json
    ├── reasoning_summary.txt
    ├── trace_reasoning.jsonl
    └── prompt_snapshots.json
```

**分析时选取 bucket**：

| 场景 | 优先 bucket |
|------|-------------|
| PR smoke 失败 | `failures/` |
| integration 流式 | `observability/` |
| nightly 发布验收 | `llm_real/` |
| 跨 run 回归 | 有 `meta.json` 的 legacy 布局 + `llm_real` metadata |

---

## 5. 场景工作流

### 5.1 PR 失败（L1）

```
1. 从 CI 日志定位失败 test_id（product_e2e::smoke::...）
2. registry.yaml → 查 CAP-* 与 parallel_group
3. 若在 RAG_SERIAL：是否 Milvus 未就绪？→ e2e-precheck
4. 读 failures/ 或 --nocapture 断言消息
5. 归因树 → 若 ToolFailure：查 mock_servers 路由；若 Assertion：查 ADR-0008 cite
6. 修复后只跑相关模块（指南 §4.1），勿全量 nightly
```

### 5.2 master 集成失败（L2）

```
1. 确认 --test-threads=1（并行必假阳性）
2. 失败是否在 shared_rag_fixture 之后首个用例？→ runtime 问题
3. streaming_chat 失败 → observability/ + trace_reasoning.jsonl
4. failure:: 或 tenants:: → 产品降级/隔离，非 LLM
5. diff 与上一绿 build 对比（若有 baseline）
```

### 5.3 发布前验收（L1→L5）

```
1. 覆盖平面：registry 中各 CAP 的 L1+L2 均有绿测
2. L3 llm_real 全绿 + llm-real summary 无 reasoning_empty_warning 集群
3. L4 skills citation 硬门禁
4. L5 journey nightly tier search 硬 citation
5. 回归平面：diff vs baseline，gate_status != Fail
6. 稳定性：trends 无 P0 flake
→ 合成发布决策（见 §6）
```

### 5.4 Agent 改代码后「该分析什么」

```
改动文件类型 → 最小分析集
├─ agents/loop, policy → L2 streaming + L3 llm_real rag_real + observability
├─ ingestion, router → L2 liteparse/paddle/office + L1 ingestion_smoke
├─ search executor → L2 failure::search_degrade + L3 search_real
├─ transport-http routes → L6 contract + L1 相关 smoke
├─ frontend chat UI → L5 journey + L4 skills
└─ auth/billing → L1 auth_boundary/billing_boundary + Playwright auth
```

---

## 6. 发布就绪度合成（Release Readiness Score）

人工一票否决项优先于分数。

| 关卡 | 条件 | 阻断？ |
|------|------|--------|
| G0 预检 | `e2e-precheck.sh` 绿 | 是 |
| G1 L1 | smoke 全绿 + module guard 绿 | 是 |
| G2 L2 | integration 0 fail | 是 |
| G3 L3 | llm_real ignored 全绿 | 是（发布） |
| G4 L4 | skills citation 硬门禁 | 是（发布） |
| G5 回归 | e2e-analyzer critical = 0 | 是 |
| G6 质量 | llm-real summary 无集群性 reasoning_empty | 否（warn） |
| G7 Judge | Playwright judge <6 | 否（warn） |

**合成规则**：G0–G5 全部满足 → **可发布**；任一失败 → **不可发布**；G6/G7 仅产生 warn 备注。

---

## 7. 工具映射速查

| 分析平面 | 首选工具 | 备选 |
|----------|----------|------|
| 覆盖 | registry.yaml + `--check-modules` | `e2e-analyzer coverage` |
| 回归 | `e2e-analyzer diff` | `report --format markdown` |
| 归因 | `e2e-analyzer diagnose` | 手动读 failures/ |
| 稳定性 | `e2e-analyzer trends`（无 meta.json 时自动回退 llm_real） | `llm-real trends` |
| 质量 | `llm-real summary` | Playwright judge 报告 |
| 基线 | `./scripts/promote-llm-real-baseline.sh` | `e2e-analyzer baseline --run <dir>` |
| Registry | `./scripts/generate-e2e-test-registry.py` | 手工编辑 yaml |

---

## 8. 与 Brooks 测试审查的关系

Brooks `brooks-test` 评的是**测试代码结构**（brittleness、mock 滥用）；TEAF 评的是**测试运行结果与产品证据**。二者互补：

| Brooks 发现 | TEAF 跟进 |
|-------------|-----------|
| mock 过严导致假绿 | 质量平面：补 L3 对照 |
| 共享 fixture 脆 | 稳定性平面：串行 pass_rate |
| 断言重复 / 协议泄漏 | 覆盖平面：E-P 应下沉 L6 |
| 无失败产物 | 归因平面：强制 save_failure_artifacts |

---

## 9. 维护规则

1. 新增 `product_e2e` 用例 → 运行 `./scripts/generate-e2e-test-registry.py` 更新 `e2e-test-registry.yaml`。
2. 新增 `product_e2e::smoke::*` 模块 → 更新 `run-product-smoke-e2e.sh`（`NON_RAG` / `RAG_SERIAL` / `SMOKE_MANUAL_ONLY`）。
3. 新增能力域 → 先改指南 §2，再改 registry `capability_domains`。
4. 新增产物字段 → 更新本文档 §4 与 `e2e-gates.md` artifact 节。
5. 扩展 `e2e-analyzer` 子命令 → 更新 §7 工具表。

---

## 附录 A：分析维度速查卡

```
TEAF 速记
─────────
WHAT  → registry CAP-* × layer
HOW   → E-P / E-Prod / E-Q / E-Obs
WHEN  → L1–L6，M/R/I/P
COVER → 能力缺层？模块未入 smoke 列表？
REG   → diff severity → gate_status
ATTR  → tool → prompt → perf → render → assert
STAB  → pass_rate, duration stddev
QUAL  → llm_real metadata + judge
```

## 附录 B：86 项测试分层统计（2026-06-13）

| 模块前缀 | 数量 | 默认 CI 层 |
|----------|------|------------|
| `smoke::` | 25（3 ignored） | L1 |
| `integration::` | 20（1 ignored） | L2 |
| `failure::` | 5 | L2 |
| `tenants::` | 2 | L2 |
| `llm_real::` | 16（9 ignored E2E） | L3 |
| 基础设施单测 | 18 | L1/L6 |

完整枚举：`cargo test --test product_e2e -p app --features product-e2e -- --list`
