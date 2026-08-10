# docs/ 文档索引

本索引是文档体系的入口：告诉你哪些文档是**当前权威**、哪些是**历史记录**。
维护规则：新增现行参考文档时登记到「当前权威」；带日期戳的计划/审计/复盘属于时间点快照，无需登记，也无需随架构演进更新内容；文档被取代时在文首加 SUPERSEDED 横幅并在「已被取代」一节登记。（索引建立于 2026-08-02 文档体系梳理）

## 现行架构基线（2026-08-02）

- **单 agent SaC**（Search as Code）：一条 ReAct 循环从指令到答案，检索全部走沙箱 SDK；orchestrator 多 agent 架构已物理删除。设计：`plans/2026-07-30-sac-sdk-single-agent-design.md`
- **Product Apps + AppState 组合根**：T1–T8 法则生效。见根 `../AGENTS.md` + `agent/product-apps.md`
- **workspace 唯一产品真相**，无 org（T7/T8）
- **计费**：B2C 用户级；渠道 **Creem + Alipay**（Stripe 已移除）；**现行商业模式见 ADR-0010**（可分享 Workspace 名额 + 代购储值）；旧 token 滚动套餐 / 桌面买断见已取代 ADR-0004
- **检索桥**：沙箱↔宿主 fd 管道 RPC（`adr/0009-retrieval-bridge.md`）
- **代码情报工具**：code-review-graph（graphify 已退役，`agent/code-review-graph.md`）

## 当前权威文档（living references）

| 文档 | 内容 |
|---|---|
| `../AGENTS.md` | 仓库法则（优先级、prompt 规则、T1–T8、验证默认） |
| `agent/product-apps.md` | T1–T8 / workspace / org 完整条文 |
| `agent/code-review-graph.md` | code-review-graph 查询与更新规则 |
| `agent/wsl-services.md` | 服务、端口、VPS 假设 |
| `agent/rust-resources.md` | WSL cargo jobs/target 策略 |
| `agent/coding-behavior.md` | 长文行为准则（人类参考） |
| `engineering/SOLO_DISCIPLINE.md` | solo 本地主干纪律 |
| `engineering/frontend-visual-debt.md` | 前端视觉债登记册 |
| `engineering/TEST_PYRAMID_DEDUP_MAP.md` | 测试去重原则与标准语料约定 |
| `engineering/PROFILE_MEMORY_SCOPE_CHAT_SEARCH.md` | 产品决策：profile memory 仅 Chat+Search |
| `engineering/DEEPSEEK_STYLE_USAGE_BILLING_DESIGN_2026-07-13.md` | 旧 token 套餐用量语义（frozen）；**主商品已由 ADR-0010 翻转**，事件/计量可复用为钱包流水 |
| `adr/0010-share-service-business-model.md` | **现行商业模式**：可分享 Workspace 订阅、代购×1.5、邀请码、本地 Publish 上云 |
| `design/STYLE_BASELINE.md` | 现行视觉基线（Slate × Indigo，Canonical） |
| `design/PRODUCT_IA.md` | **登录后产品信息架构 v1**（Jobs / Sitemap / Canonical / Shell；改导航前必读） |
| `design/PRODUCT_IA_AUDIT.md` | 产品 IA 审计（入口矩阵、P0/P1、本轮关闭项） |
| `desktop/RELEASE-AND-DOWNLOAD.md` | 桌面端构建/签名/发布 runbook（**v0.2+ 免费客户端**） |
| `desktop/VERSIONING.md` | 桌面 SemVer 与云端 API 兼容矩阵 |
| `desktop/SUPPORT-AND-SLA.md` | 桌面 vs 云端支持边界 |
| `desktop/SMOKE_CHECKLIST.md` | 客户端安装与本机栈冒烟清单（无激活墙） |
| `desktop/2026-08-10-v0.2.0-free-client-release.md` | **v0.2.0 免费客户端打包设计**（ADR-0010 对齐） |
| `desktop/2026-08-04-portable-runtime-design.md` | **便携 PG+pgvector+Redis 捆绑设计**（装进 NSIS，无 Docker） |
| `desktop/LOCAL-CLIENT-MCP-CLI-AGENT-ACCESS.md` | **本机客户端 × Coding Agent**：MCP/CLI 能力矩阵、鉴权边界、P0/P1（建库/解析/问答/分享） |
| `release/desktop/2026-08-10-v0.2.0.md` | Desktop v0.2.0 release notes |
| `specs/usage-export-and-retention.md` | 用量导出与保留规格（Draft，与 ADR-0006 一致） |
| `../avrag-rs/docs/e2e-gates.md` | L1/L2/L3 E2E 门径语义（本目录外，生效中） |
| `../avrag-rs/prompts/README.md` | prompt CDS 布局与撰写规则 |
| `../avrag-rs/prompts/loop/README.md` | loop 观察消息加载路径与规则 |

## 近期审计快照（时间点；未吸收进 AGENTS 前非强制）

- `engineering/2026-08-10-harness-llm-user-channel-philosophy-diagnosis.md` — **Harness / LLM / 用户三角信道** vs 现网出站；主审复核通过；**§17 方案补丁**（verify 面 / ceiling 分叉 / 次数阈值）闭合后可开 P0

## 进行中的计划

- `plans/2026-08-02-architecture-deepening-plan.md` — 架构深化 5 波（C1 doc-scope / C2 profile memory / C3 Alipay / C4 llm / C5 citations 等）
- `../avrag-rs/docs/plans/2026-08-06-ingestion-window-session-ps-merge.md` — **ingestion 原文均切窗口会话 + PS 合一 + triplet 同链 + SaC 去 doc_profile**（主干已接线；需 migration 0075 + 探针）
- `../avrag-rs/docs/plans/2026-08-06-rerank-style-sticky-and-querycard-calc.md` — **Rerank style 粘连 + P-calc-ok + 外推/截断/天气/元数据提示**（2026-08-06 **已落地**）

## ADR 决策记录

| 文件 | 标题（文内编号） | 真实状态 |
|---|---|---|
| `adr/0001-user-level-billing-b2c.md` | 用户级计费 B2C | **现行**（Stripe 描述已过期，文首有注释） |
| `adr/0002-agent-decision-model-messenger-first.md` | messenger-first 决策模型 | **现行**（尾部两个 Related 链接已失效，已就地标注） |
| `adr/0003-router-policy-removal-and-auto-mode-subagents.md` | RouterPolicy 移除 | **部分取代**：删除决定有效；orchestrator+subagents 前瞻方向已被 SaC 单 agent 取代（文首有横幅） |
| `adr/0004-desktop-hybrid-business-model.md` | （文内题 "ADR 0003"）桌面混合商业模式 | **已被 ADR-0010 取代**（客户端买断 + SaaS token） |
| `adr/0010-share-service-business-model.md` | 分享服务商业模式 | **现行** Accepted（2026-08-05） |
| `adr/0005-llm-provider-protocol-architecture.md` | （文内题 "ADR 0004"）LLM 四轴协议架构 | **现行**，Accepted（已实施） |
| `adr/0006-product-architecture-decisions-post-tn.md` | TN 后 13 条产品/架构裁决 | **现行**，最常被引用的决策文档 |
| `adr/0006-execute-plan-removal-inventory.md` | execute-plan 删除清单 | 已完成快照 |
| `adr/0006-write-heavytail-crate-split-plan.md` | write-core/heavytail 拆分 | 已完成快照 |
| `adr/0007-product-apps-composition-root.md` | Product Apps 组合根 | **现行**（Phase A/B 已落地） |
| `adr/0009-retrieval-bridge.md` | 检索桥（fd 管道 RPC） | **现行**（已落地） |

### ADR 编号已知问题（刻意保留，不要"修"）

- 存在三个编号 0006 的文件（post-tn / execute-plan / write-heavytail）。
- `0004-desktop-*.md` 文内标题为 "ADR 0003"、`0005-llm-*.md` 文内标题为 "ADR 0004"：历史编号碰撞，e2e 语料与审计文档引用了 "ADR-0004"，改名会破坏引用，故保留（决策见 `plans/2026-08-02-architecture-deepening-plan.md` §5）。
- 无 0008。
- `0010-share-service-business-model.md` 已占用；新增 ADR 请使用 **0011** 起的编号。

## 已被取代（文首有 SUPERSEDED 横幅，仅作历史记录）

- 商业模式（被 ADR-0010 分享服务取代，2026-08-05）：
  - `adr/0004-desktop-hybrid-business-model.md`（桌面买断 + SaaS token 托管）
  - 主商品意义上的 token 三档：`superpowers/specs/2026-06-07-pricing-tiers-revamp-design.md`（及 plan）；`engineering/DEEPSEEK_STYLE_USAGE_BILLING_DESIGN_2026-07-13.md` 仅保留计量公式参考
- orchestrator 时代（被 2026-07-30 SaC 单 agent 设计取代）：
  - `engineering/ORCHESTRATOR_SUBAGENT_CHAT_DESIGN_2026-07-16.md`
  - `engineering/ORCHESTRATOR_SUBAGENT_CHAT_PLAN_2026-07-16.md`
  - `engineering/ORCHESTRATOR_O1_FIX_PLAN_2026-07-17.md`
  - `engineering/ORCHESTRATOR_V2_REACT_EVIDENCE_STORE_DESIGN_2026-07-18.md`
  - `engineering/ORCHESTRATOR_HANDOFF_2026-07-18.md`
  - `engineering/GOLDEN_SET_ORCHESTRATOR_STATUS_2026-07-19.md`
  - `plans/2026-07-30-sac-six-fail-fix-plan.md`（关注点已被后续计划吸收）
- superpowers 旧架构：`superpowers/specs/2026-05-23-e2e-state-machine-prompt-validation-design.md`（v5 状态机已移除）、`superpowers/specs/2026-06-07-e2e-test-rearchitecture-for-v6-design.md`（notebook-first 被 T7 取代）
- superpowers 定价对（配额模型被 ADR-0006 #1 部分取代）：`superpowers/specs/2026-06-07-pricing-tiers-revamp-design.md`、`superpowers/plans/2026-06-07-pricing-tiers-revamp-plan.md`
- 根目录散件：`../DESIGN.md`（被 `design/STYLE_BASELINE.md` 取代）、`../CONTEXT.md`（自称 source-of-truth，已过期）

## 部分过期（主体仍有效，文首有注释）

- `engineering/CAPABILITIES_MULTISELECT_AND_USER_CONTEXT_DESIGN_2026-07-15.md` / `..._PLAN_2026-07-15.md` — 产品面有效；Runtime follow-on 指向已删除的 orchestrator
- `engineering/AGENT_PROGRESS_DISCLOSURE_DESIGN_2026-07-13.md` — WorkFact 机制有效；"四种模式" 中的 Write 已移除
- `design/UI_REVIEW_AND_VIBRANT_COLOR_PROPOSAL_2026-07-21.md` — 提案已被采纳（头部状态已更新）

## 历史记录区（dated snapshots，内容不随架构演进更新）

以下均为已完成的时间点记录，保留原样：测试金字塔/E2E 修复系列（2026-07-09～13）、TN 整改与 Product App 迁移系列（07-09～10）、WORKSPACE_RENAME_DECISIONS、org 移除与 ingestion 系列、计费实施记录（STRIPE_BILLING_REMOVAL / DEEPSEEK_USAGE_BILLING_DEV_PLAN / ALIPAY_F2F_INTEGRATION）、品牌/多站点/桌面 journey/视觉系统系列（07-10～14）、`_reports/`、`agent/ocr-review-2026-07-31*.md`、`plans/2026-07-30-prompt-cache-strategy.md`、`plans/2026-07-31-ocr-review-fix-plan.md`、`plans/2026-07-30-sac-sdk-single-agent-dev-plan.md`（执行记录，状态行已更新为已落地）、`engineering/UNIFIED_SYNTHESIS_CONTRACT_2026-07-15.md`（机制仍在代码中生效）。

## 本目录之外的文档位置

- `../avrag-rs/docs/` — 后端文档，有自己的索引 [`../avrag-rs/docs/README.md`](../avrag-rs/docs/README.md)（2026-08-02 已随本索引同步重写）
- `../frontend_next/docs/` — 前端零星设计/ops 文档（尚未梳理）
- `../docs-recovered-from-grok/` — 历史恢复档案（自称非当前决策依据）

## 文案总索引

- [`docs/copy-catalog/`](./copy-catalog/) — 产品 UI i18n 全文 + prompts 清单 + 内联/后端硬编码线索（集中改文案用）
