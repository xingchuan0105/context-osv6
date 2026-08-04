# VGRAG 正式验收 + 未落地任务合并计划 + SkillOpt 分层训练就绪

| 项目 | 内容 |
|------|------|
| 日期 | 2026-08-04 |
| 状态 | **VGRAG 产品路径验收（D1）**；后续任务合并进本计划；SkillOpt 分层训练待 key 就绪后开工 |
| 评测锚 | graph81 D1 `full149_20260803-180401` / artifact `v2_20260803-180437`：**v2 PASS 78/81** |
| 相关 | `2026-08-03-vector-graph-rag-eval-design.md` · `tools/skillopt/` · `docs/plans/2026-08-02-skillopt-layered-training-impl.md` |

---

## 0. 拍板（本窗口）

| # | 议题 | 决定 |
|---|------|------|
| 1 | **VGRAG 产品路径** | **正式验收**，替代「lexical side-car 图 / 独立 client.graph / 默认 ann」作为产品 dense 路径 |
| 2 | 默认后端 | 代码默认 `DENSE_BACKEND=vgrag`（`vgrag.rs`）；`ann` 仅 ops 回滚 / A/B |
| 3 | graph81 验收口径 | 以 **D1（vgrag）** 为主；`B0–B4` 为 legacy 对照，**不再**当产品门禁 |
| 4 | 架构阶段 | **基本稳定** → 下一主线改为 **SkillOpt 分层优化 skill**，不再扩检索管线 |
| 5 | 训练主 LLM | **不用贵价官方 DeepSeek 直连作主力**；优先 **OpenCode Go** 与 **Ollama Cloud** 订阅通道上的 **DeepSeek V4 Flash** |
| 6 | Key | **2026-08-04 已写入** `avrag-rs/.env`（OpenCode Go → `QWEN_CHAT_*` + Ollama Cloud）；不入库、不打印；chat 烟测均 HTTP 200 |

---

## 1. VGRAG 正式版定义（验收基线）

### 1.1 产品行为

```
client.dense(query)
  → ANN 池（≤24）
  → terms_for_vgrag_seeds(query, pool)
  → search_graph hop=2（owner + doc_scope）
  → cite-safe evidence（support_chunk_id + doc_id + text）
  → RRF fuse → final cut（≤12）
  → 仍叫 dense_retrieval；无 client.graph
```

- 封闭本体 6 谓词：`类型/部分/参与/依赖/位于/标识`（入库 strict 归一化）。
- Skill：entity-first dense subquery + 图扩邻种子策略（`knowledge-base` SKILL 4.6 + strategies）。

### 1.2 验收证据（2026-08-04）

| 项 | 结果 |
|----|------|
| graph81 D1 v2 | **78/81 PASS**（PARTIAL 2 · REFUSAL_WRONG 1） |
| correctness / faithfulness | 0.975 / 0.980 |
| `vgrag_graph_n > 0` | **267/281** dense 调用（~95%） |
| graph_n mean | **3.71**（修 cite-safe 前全 0） |
| entity-first | multi_dense 71/81 · 短串 dense 278/281 |
| 当前语料图 | workspace 719 边 **100% closed6** |

### 1.3 产品默认 / 回滚

| 环境 | 值 |
|------|-----|
| 默认 | 不设或 `DENSE_BACKEND=vgrag` |
| 回滚 | `DENSE_BACKEND=ann`（纯向量，无图 fuse） |
| 侧车 | `RETRIEVAL_GRAPH_AUGMENT=0`（产品默认关；图只在 dense 内） |

`.env.example` 已注释 `DENSE_BACKEND=vgrag|ann`；部署时确认 VPS 未误设 `ann`。

### 1.4 非目标（本验收不阻塞）

- 全量 149 再跑一轮（可排期，非 VGRAG 门禁）。
- Legacy B0–B4 全表（历史对照，可选）。
- e2e_smoke 历史 workspace open 谓词 **残留清理**（当前 doc_scope 已隔离；卫生项见 §2）。

---

## 2. 未落地任务合并（去重后优先级）

来源：本窗口行为分析 · VGRAG 修复 · `2026-08-03-full149-bge-m3-behavior-and-fixes-handover.md` · skill 修改意见 · graph81 三非 PASS。

### P0 — 产品行为 / 正确性（skill 优先，少动代码）

| ID | 任务 | 归属 | 状态 |
|----|------|------|------|
| P0-a | **应拒答纪律**：结构人数 ≠ 访谈人数 — 观察型 skill | `strategies-grounding` FS6 + gotcha | **已做（spoke）** |
| P0-b | **表口径**：行数/`total_hits`/COUNT；去重须声明 | `strategies-tables` + how-to-read-tables B2 | **已做（spoke）** |
| P0-c | **跨文档综合** 共同抽象 | `strategies-grounding` FS7 | **已做（spoke）** |

### P1 — 可观测 / 工程卫生（小 diff）

| ID | 任务 | 状态 |
|----|------|------|
| P1-a | tool_trace：`request` / `vgrag_graph_n` / `relation_n` / `evidence_dropped` | **已做** |
| P1-b | tool_trace **error/stderr 截断**（≤500 字，UTF-8 安全） | **已做**（`compact_tool_trace`） |
| P1-c | v2 `qNNN.artifact.json` **写入同一 tool_trace** | **已做** |
| P1-d | e2e_smoke **非当前 workspace** 的 `rag_kg_*` 清理（open 谓词垃圾） | 未做（可选运维） |

### P2 — 成本 / 策略

| ID | 任务 | 状态 |
|----|------|------|
| P2-a | 表题减 **grep_storm**，压向 struct | **strategies-tables** gotcha 已写；全量行为待 skillopt |
| P2-b | 纯数值/ADR dense → graph0 时 lexical/grep | **strategies-graph** gotcha 已写 |
| P2-c | 全量 149 VGRAG 基线刷新 | **已完成** c12 一次成功；v2 **PASS=132** /149（UNGROUNDED=7 PARTIAL=2 SELECTION_MISS=3 RETRIEVAL_MISS=2 REFUSAL_WRONG=1 JUDGE_ERROR=1 INFRA=1）；log `full149_20260803-185912.log` · artifact `v2_20260803-185913` · ~22.6 min · agent=OpenCode Go Flash / judge=Ollama 0731-cloud |
| P-prog | strategies 渐进披露（薄层 + 场景 spoke） | **已做 2026-08-04** |

### P3 — 明确不做 / 降级

| ID | 说明 |
|----|------|
| 独立 `client.graph` | 已移除；不恢复 |
| 宿主语义「覆盖完整」硬拒 | 停决策归 model+skill（AGENTS.md） |
| B0–B4 当产品门禁 | 废弃为对照 |

### Legacy 评测设计文档状态（更新）

`2026-08-03-vector-graph-rag-eval-design.md` 中 D0/D1 为主验收；B0–B4 跑次改为 **可选考古**，不再阻塞发布。

---

## 3. 部署 / 进程（另一 coding 窗口改动后）

### 3.1 本机已执行

```text
cargo check -p app-chat   # 2026-08-04：通过（约 12s；仅有既有 unused 警告）
```

### 3.2 重部署 API（需显式确认目标）

| 目标 | 命令 | 注意 |
|------|------|------|
| **VPS 产品** | `bash scripts/deploy-backend.sh` | 构建 release bin + 同步 migrations/prompts + 重启容器；**脏树会标 dirty** |
| 仅资产 | `ASSETS_ONLY=1 bash scripts/deploy-backend.sh` | 只同步 prompts/migrations |
| 状态 | `bash scripts/deploy-status.sh` | 对照 local vs VPS |

**本计划不自动 VPS 部署**（共享环境 + 脏工作区含 VGRAG/skill 未提交改动）。你确认「部署 VPS」或「只重启本机 API」后再执行。

本机 API 若用自管进程：在对应目录用当前 `DENSE_BACKEND`（默认 vgrag）重启 `avrag-api` / worker；环境变量以 `avrag-rs/.env` 为准。

---

## 4. SkillOpt 分层优化（下一主线）

### 4.1 权威设计

- 实施方案：`docs/plans/2026-08-02-skillopt-layered-training-impl.md`
- 落地说明：`docs/plans/2026-08-02-skillopt-layered-training-landing.md`
- 工具：`avrag-rs/tools/skillopt/`（README + ACCEPTANCE）

**训练优先序（实证）**：

```text
L2.5 停/答/grounding  >  L3 选择/合成  >  L2 检索  >  L1.5 代码  >  L1 路由
```

graph81 行为报告互证：检索面已健康（entity-first + VGRAG）；**金矿在拒答 / 表口径 / 跨文档综合（L2.5+L3）**。

### 4.2 与 VGRAG 验收的关系

| 层 | VGRAG 后 | SkillOpt 重点 |
|----|----------|----------------|
| L2 检索 | 基本够用 | 小样本回归即可，勿占主预算 |
| L2.5 行为 | 3 道非 PASS 典型 | **主训** |
| L3 合成/引用 | 部分 PARTIAL | 次主训 |
| L1.5 代码 | grep_storm / 方法名 | 夹带在 stop-answer 轨迹 |

### 4.3 当前 skillopt 接线事实

- optimizer：`qwen_chat` backend → 复用 `AGENT_LLM_*` / 映射为 `QWEN_CHAT_*`（OpenAI 兼容）。
- PyPI skillopt 0.2.0 **无**独立 `openai_compatible` 名；用 **兼容端点** 即可。
- 纪律：**golden 不进 skill 正文**；训练期勿并行全量 149。

---

## 5. 训练 LLM：OpenCode Go / Ollama Cloud（官方要点）

> Key 由你稍后提供；下列仅接线规格，**不写真实密钥**。

### 5.1 OpenCode Go（推荐 skillopt optimizer 主通道）

| 项 | 值 |
|----|-----|
| 文档 | https://opencode.ai/docs/go/ · Providers: https://opencode.ai/docs/providers/ |
| 订阅 | 首月 $5，其后 $10/月（官方页） |
| 鉴权 | https://opencode.ai/auth → API key；TUI：`/connect` → OpenCode Go |
| Chat 端点 | `https://opencode.ai/zen/go/v1/chat/completions` |
| 模型列表 | `https://opencode.ai/zen/go/v1/models` |
| **DeepSeek V4 Flash** model id | `deepseek-v4-flash` |
| OpenCode 内引用 | `opencode-go/deepseek-v4-flash` |
| AI SDK | `@ai-sdk/openai-compatible` |
| 额度（Flash） | 约 $60/月模型额度档；5h / 周 / 月总 cap 见官方 Usage limits（$12 / $30 / $60） |

**skillopt 映射建议**（key 就绪后）：

```bash
# avrag-rs/.env（示例名，勿提交）
# SKILLOPT_OPTIMIZER_BASE_URL=https://opencode.ai/zen/go/v1
# SKILLOPT_OPTIMIZER_API_KEY=<OpenCode Go key>
# SKILLOPT_OPTIMIZER_MODEL=deepseek-v4-flash
# 或映射到现有 QWEN_CHAT_* / AGENT_LLM_* 三件套：
# QWEN_CHAT_BASE_URL=https://opencode.ai/zen/go/v1
# QWEN_CHAT_API_KEY=...
# QWEN_CHAT_MODEL=deepseek-v4-flash
```

DeepSeek 官方亦有「OpenCode + DeepSeek 直连」文档（`api-docs.deepseek.com` …/opencode/）；**本计划优先 Go 订阅**，避免直连接计量成本。

### 5.2 Ollama Cloud（备通道 / 本机工具链）

| 项 | 值 |
|----|-----|
| 文档 | https://docs.ollama.com/cloud |
| 登录 | `ollama signin`（ollama.com 账户） |
| 直连 API | `https://ollama.com` + header `Authorization: Bearer $OLLAMA_API_KEY` |
| Key | https://ollama.com/settings/keys |
| 协议 | Ollama 原生 `/api/chat`（**非**标准 OpenAI path 时需适配） |
| DeepSeek | 云模型库见 ollama.com/search；退役表推荐替代含 `deepseek-v4-flash` |

**注意**：skillopt 的 `qwen_chat` 路径期望 **OpenAI 兼容** `/v1/chat/completions`。Ollama Cloud 直连是 Ollama API 形态时，需：

- 用本地 `ollama` 守护进程代理 cloud 模型（`ollama run …-cloud`）再挂 OpenAI 兼容层，或  
- 小适配层把 Ollama chat 转成 OpenAI schema。

**优先接线顺序**：**OpenCode Go（OpenAI 兼容）→ 再 Ollama Cloud**。

### 5.3 双模型角色（分层训练 D 决策对齐）

| 角色 | 建议模型 | 通道 |
|------|----------|------|
| **optimizer / reflect** | DeepSeek V4 Flash | OpenCode Go |
| **rollout 评分** | 产品 agent 现网 LLM（不变） | 现有 `AGENT_LLM_*` / 评测栈 |
| 可选：轻量 reflect 试跑 | 同 Flash 或更小 | Ollama Cloud 试通 |

Rollout 仍跑真实 `realistic_corpus_full_eval`（贵在 **产品侧 LLM+检索**，不在 optimizer）；optimizer 换订阅通道主要省 **reflect 轮次**费用。

### 5.4 就绪检查清单（key 到齐后）

1. [ ] OpenCode Go key → `.env` 三件套可 `curl` chat/completions 通 Flash  
2. [ ] （可选）Ollama Cloud key → `curl https://ollama.com/api/tags`  
3. [ ] `tools/skillopt`：`train_avrag149.py --check`  
4. [ ] 小配置 `stop-answer.yaml` 单 epoch 烟测（小 batch / 少题）  
5. [ ] 确认无 golden 泄漏进 skill 产物  

---

## 6. 建议执行序（合并甘特）

```text
Done
  ├─ VGRAG 正式验收 + D1 78/81
  ├─ P0 skill（grounding/tables/graph spokes）+ 渐进披露
  ├─ P1-a/b/c tool_trace（request/vgrag/error/stderr）+ v2 artifact 合并
  ├─ OpenCode Go / Ollama key 入 .env + 烟测
  └─ cargo check app-chat / disclosure + loop_observability 单测

In progress
  ├─ SkillOpt **底层 L1.5+L2** 全 149 慢训（`retrieval-full149.yaml`）
  │    prompt_target=`capabilities/knowledge-base/SKILL.md`
  │    agent=OpenCode Go Flash · judge=Ollama 0731-cloud · script=`scripts/run-skillopt-retrieval-full149.sh`
  └─ 完成后人工审 best_skill 再回填

Next
  ├─ 上层 L2.5 stop-answer / grounding（确认后）
  ├─ 可选：表题宿主结构触发 strategies-tables
  └─ 等你：VPS deploy？

Later
  ├─ 全量 149 VGRAG 基线（P2-c）
  └─ e2e_smoke 历史 KG 清理（P1-d）
```

---

## 7. 一句话

**VGRAG 按 D1 正式收口**；剩余是 **skill 行为 + SkillOpt 分层**，不是再改检索管线。训练主 LLM 走 **OpenCode Go / Ollama 订阅上的 DeepSeek V4 Flash**；key 就绪即可按 §5.4 开工。部署 API 等你指定目标后再跑 `deploy-backend.sh`。
