# avrag-rs 文档索引

> 最后更新：2026-08-05（解析层 anydoc 广覆盖设计）。
> 本索引是 avrag-rs 文档的入口：哪些是当前权威、哪些已被取代、哪些是历史快照。全仓库总索引见根 [`docs/README.md`](../../docs/README.md)。
> 维护规则：新增现行参考文档时登记到「当前权威」；带日期戳的计划/交接/报告属于时间点快照，无需随架构演进更新内容；文档被取代时在文首加 SUPERSEDED 横幅并登记到「已被取代」。

## 现行架构基线（2026-08-02）

- **单 agent SaC**（Search as Code）：一条 ReAct 循环从指令到答案；检索全部走沙箱 SDK，经 fd 管道桥（ADR-0009）到宿主；无原生检索 function-calling。设计：根 [`docs/plans/2026-07-30-sac-sdk-single-agent-design.md`](../../docs/plans/2026-07-30-sac-sdk-single-agent-design.md)。orchestrator 多 agent 代码已于 2026-08-01 物理删除（commit `7f2d182d`），权威记录：[`engineering/2026-07-31-sac-orchestrator-isolation.md`](engineering/2026-07-31-sac-orchestrator-isolation.md)。
- **Product Apps + AppState 组合根**：T1–T8 法则（根 `docs/adr/0007-product-apps-composition-root.md`、`docs/agent/product-apps.md`）；workspace 唯一产品真相，无 org。
- **计费**：用户级；Creem + Alipay；rolling 窗口 + soft limit。语义参考：[`superpowers/specs/2026-07-05-llm-usage-exit-metering-design.md`](superpowers/specs/2026-07-05-llm-usage-exit-metering-design.md)。
- **Prompts**：全部 LLM 面向文案在 `../prompts/**/*.md`（CDS，第三人称观察文体）；规则见 `../prompts/README.md`、`../prompts/loop/README.md`。
- **解析/入库**：按格式分工（2026-08-05）：PDF→liteparse；**Office/ODF/RTF/EPUB/CSV 等（anydoc 非 PDF 全集）→anydoc**（+ pptx 族 hex strip）；文本/代码/tsv/html…→markitdown；图片→PaddleOCR。设计真相：[`plans/2026-08-05-parser-pipeline-anydoc.md`](plans/2026-08-05-parser-pipeline-anydoc.md)（取代 08-02 office-direct 决策；历史：[`plans/2026-08-02-parser-pipeline-direct-readers.md`](plans/2026-08-02-parser-pipeline-direct-readers.md)）。struct query：[`plans/2026-07-31-struct-query-virtual-tables.md`](plans/2026-07-31-struct-query-virtual-tables.md)；运维：[`runbooks/worker-dev.md`](runbooks/worker-dev.md)（实施 anydoc 后同步）。
- **检索数据面**：云端 Milvus / 本地 pgvector；图增强仅 lexical 1 跳 + 得分落差截断（canonical：[`plans/2026-07-23-lexical-graph-augment-scoring-design.md`](plans/2026-07-23-lexical-graph-augment-scoring-design.md)）。
- **评测**：分轨记分卡（ADR-0011）+ judge-first 生成层（ADR-0012，设计：[`plans/2026-07-24-rag-eval-judge-v2-design.md`](plans/2026-07-24-rag-eval-judge-v2-design.md)）。
- **代码情报**：code-review-graph（graphify 已退役），规则见根 `docs/agent/code-review-graph.md`。

## ADR 决策记录（`adr/`）

| 文件 | 决策 | 真实状态 |
|---|---|---|
| `0001-m1-m2-scope.md` | M1+M2 骨架先行 | 完成快照 |
| `0002-ingestion-routing-and-retrieval.md` | 双路入库 + 多模态召回 | 已被取代（文首有横幅；解析层现为 markitdown） |
| `0003-v5-agent-architecture.md` | v5 能力注册 + Strategy 状态机 | **已被取代**（横幅已加） |
| `0004-rag-agent-loop-native-tools.md` | 原生工具调用 | **已被取代**（横幅已加；检索现走沙箱 SDK） |
| `0005-unified-agent-kernel.md` / `0005-…-revised.md` / `_0005-…md` | 统一 AgentKernel | 已被取代（均有横幅；`_0005` 为标记前副本） |
| `0006-unified-agent-loop.md` / `_0006-…md` | 1+1 工具 ReAct | 已被取代（均有横幅；两文件相同） |
| `0006-unified-agent-loop-revised.md` | 统一 ReActLoop + Tool/Skill 分层 | 部分废止，取代链终点 = SaC（横幅已更新） |
| `0007-react-phased-context-disclosure.md` | 逐迭代上下文披露 | 部分被 SaC 取代；codegen 唯一检索入口等机制仍有效 |
| `0008-query-normalization-and-answer-contract.md` | query 消解 + 答案协议 | 部分现行（§3 废止→ADR-0010；§4/§5 有效） |
| `0009-codegen-sandbox-retrieval-bridge.md` | 沙箱检索桥（fd 管道 RPC） | **现行** |
| `0010-remove-server-side-query-normalization.md` | 移除服务端消解 | **现行** |
| `0011-evaluation-decoupled-scorecard.md` | 分轨记分卡 | 部分现行（生成层门禁已由 0012 取代） |
| `0012-rag-eval-v2-judge-first.md` | judge-first 生成层评测 | **现行** |
| `appstate-decomposition-phase2-5.md` | AppState 拆分（Product Apps 前身） | 完成快照 |

编号说明：0005/0006 各有正本 + revised + 下划线副本并存，属历史沿革，刻意保留；`_` 前缀文件为打标前副本。新增后端 ADR 用 0013 起。

## 当前权威文档（living references）

| 文档 | 内容 |
|---|---|
| [`e2e-gates.md`](e2e-gates.md) | L1/L2/L3 E2E 门径语义 |
| [`e2e-test-registry.yaml`](e2e-test-registry.yaml) | TEAF 机器可读注册表（脚本可再生） |
| [`full-functional-e2e-guide.md`](full-functional-e2e-guide.md) | 全功能 E2E 覆盖手册 |
| [`e2e-analysis-framework.md`](e2e-analysis-framework.md) | TEAF 五平面分析框架 |
| [`code-review-checklist.md`](code-review-checklist.md) | 提交级审查清单（ACL/出处/SSE  parity 等耐久不变量） |
| [`runbooks/local-dev.md`](runbooks/local-dev.md) | 本地开发 runbook |
| [`runbooks/worker-dev.md`](runbooks/worker-dev.md) | 入库/worker 运维（markitdown 时代的入库真相） |
| [`runbooks/milvus-wsl-manual.md`](runbooks/milvus-wsl-manual.md) | Milvus WSL 运维 |
| [`runbooks/visual-regression-testing.md`](runbooks/visual-regression-testing.md) | 视觉回归（frontend_next） |
| [`legal-compliance-pages-design-2026-06-13.md`](legal-compliance-pages-design-2026-06-13.md) | 法务页验收规格（绑定 `scripts/verify-legal-p0.sh`） |
| [`plans/2026-07-24-rag-eval-judge-v2-design.md`](plans/2026-07-24-rag-eval-judge-v2-design.md) | eval v2 设计真相 |
| [`plans/2026-07-23-pgvector-vector-graph-rag-design.md`](plans/2026-07-23-pgvector-vector-graph-rag-design.md) | pgvector 后端设计（桌面/私有路径） |
| [`engineering/2026-07-31-sac-prompt-context-engineering-audit.md`](engineering/2026-07-31-sac-prompt-context-engineering-audit.md) | SaC prompt 包审计 |
| [`engineering/2026-07-31-sac-skill-fail6-reg.md`](engineering/2026-07-31-sac-skill-fail6-reg.md) | fail-6 回归 harness |
| [`superpowers/specs/2026-07-05-llm-usage-exit-metering-design.md`](superpowers/specs/2026-07-05-llm-usage-exit-metering-design.md) | 现行计费计量语义（状态已订正为现行） |

## 进行中的工作流

- [`plans/2026-08-02-skillopt-layered-training-impl.md`](plans/2026-08-02-skillopt-layered-training-impl.md) + `…-landing.md` — SkillOpt 分层训练（进行中）
- [`plans/2026-08-02-final-answer-checkpoint-impl.md`](plans/2026-08-02-final-answer-checkpoint-impl.md) + `…-acceptance.md` — 终答检查点
- [`engineering/2026-08-02-golden149-llm-behavior-report.md`](engineering/2026-08-02-golden149-llm-behavior-report.md) / `…-regression-report.md` — 最新 golden149 行为/回归报告
- 根 [`docs/plans/2026-08-02-architecture-deepening-plan.md`](../../docs/plans/2026-08-02-architecture-deepening-plan.md) — 架构深化 5 波

## 已被取代（文首有 SUPERSEDED 横幅，仅作历史记录）

- **orchestrator 时代**（被 2026-07-30 SaC 单 agent 设计取代，代码已删除）：`plans/2026-07-20-unified-product-agent-option-d.md`、`2026-07-20-option-d-test-gap-and-drift.md`、`2026-07-20-orchestrator-prompt-engineering-optimization.md`、`2026-07-20-prompt-stack-diagnosis-post-full-eval.md`、`2026-07-27-agent-output-compiler-gray-zone.md`、`2026-07-28-channel-persistent-worker-design.md`、`2026-07-28-evidence-plane-retrieval-log-adaptive-k.md`；根 `docs/engineering/ORCHESTRATOR_*` 六篇见根索引。
- **部分过期（orchestrator 叙述 + 仍有效内容）**：`engineering/2026-07-28-handover.md`、`2026-07-29-markitdown-hard-gate-handover.md`、`2026-07-30-full149-process-budget-handover.md`、`plans/2026-07-29-pi-informed-agent-architecture-optimization.md`。
- **早期 agent 架构**：`superpowers/specs/2026-05-12-architecture-baseline.md`（曾自称「定稿」，实为最大误导源）、`adr/0003`、`adr/0004`、`adr/0005` 系列、`adr/0006` 系列、`agents/skill-development-guide.md`、`agents/ARCHIVE-superseded-by-adr-0007.md`（部分过期）。
- **检索/入库旧栈**：`liteparse-paddle-ingestion-architecture-2026-06-13.md`（LiteParse 已移除）、`plans/2026-07-01-rag-optimization-todo.md`、`plans/2026-07-06-rag-answer-reporter-and-multiturn-tests.md`、`superpowers/` 2026-03～05 的 routing/retrieval/main-agent 系列（多自带历史标注）。
- **Write 产品模式**（2026-07-15 移除；write_refine lane 仍在树内）：`plans/2026-07-08-write-mode-launch-plan.md`、`plans/2026-07-14-writing-style-mcp-design.md`（未实施草案）。
- **SaC 前身**：`plans/sa-c-impl-plan-2026-06-02.md`、`plans/perplexity-sac-learnings-2026-06-02.md`（当时结论后被反转）。
- **其他**：`dev-plan-2026-06.md`（上游已部分废止）、`plans/2026-05-12-agent-harness-upgrade-implementation.md`（自标废弃）、`plans/2026-07-04-vector-graph-rag-upgrade.md`（dense-hook 部分自标被 07-23 取代）、`runbooks/figma-parity-gate.md`（STALE：锚定已弃用的 frontend_rust）。

## 历史记录区（dated snapshots，内容不随架构演进更新）

- `plans/`：struct-query 系列 12 篇（2026-07-31，SaC 时代已落地）、loop-terminal-answer 两篇、2026-08-01 各验收/fix-audit、2026-07-04～09 各检索/缓存/存储拆分设计、Jun 各 e2e/ingestion 计划、`writing-style-mcp/` 草案包。
- `engineering/`：2026-07-28 → 08-02 的交接与 full149/golden149 回归报告（注意 07-28～30 三篇为 orchestrator 时代叙述，已加「部分过期」注释）。
- `reviews/`：TN 评审系列（2026-07-08～09，自带 README 标注为历史）。
- `archive/`：brooks 评审 v1–v6 等 30 篇。
- 顶层 `brooks-*-2026-06-13*` 7 篇：brooks 评审最终轮（v7/round7）报告。
- `api/m1-m2-contract.md`、`compose/plans/` 两篇、`spike/`（Paddle 校准数据）、`t13-app-split-inventory.md`、`e2e-bridge-followup-2026-06-09.md`、`e2e-codegen-memory-tests-design-2026-06-10.md`、`prompts-memory-doc-profile-optimization-2026-06-10.md`、`memory-recall-gap-2026-06-13.md`、`query-library-design-2026-06-14.md`、`model-provider-matrix-2026-03.md`、`product-e2e-plan.md`、`architecture-review-2026-06.md`、`HEALTH_OPTIMIZATION_HANDOFF_2026-06-11.md`、`e2e-fix-plan-2026-06.md`、`../handoff-rag-evaluator-fix.md`。
- `agents/`：`cds-v1.1.md`（部分过期，有注释）、`domain.md`、`issue-tracker.md`、`triage-labels.md`（solo 项目下近似 vestigial）、`progressive-disclosure-framework.md`（自带 ⛔ 归档横幅）。

## 旧检索栈提醒

仍提到 Qdrant / Tantivy / PG-BM25 / MinerU / LiteParse 作为目标架构的文档均为历史记录（多数已有横幅或自带标注）。除非文档明确说明描述的是当前兼容实现，否则不要把旧栈描述视为当前目标；现行检索/入库真相以「现行架构基线」一节为准。
