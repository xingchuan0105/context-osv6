# ADR 0006: 产品与架构决策（TN 复审后拍板）

## Status

Accepted — 2026-07-09（产品确认）

## Context

Thermo-Nuclear 结构债主路径（C1–C3 / H1–H3 / 长尾）落地后，剩余阻塞项多为 **产品口径与架构边界**，而非“还能再抽一层函数”。本 ADR 记录已确认决策，并给出工程含义与对第 3 / 9 / 11 项的释义。

---

## Decision summary

| # | 主题 | 决策 |
|---|------|------|
| 1 | 用量真相 | **Rolling 为唯一真相**；配额策略为 **软限 + 事后扣减** |
| 2 | Write 模式 | **对所有租户开放**；视为 Chat / RAG / WebSearch 之后的 **第四标准模式**；计费文案只提示“用量大”，**不单独展示 write 成本** |
| 3 | Memory 运行时 | 见下文详释；方向：**生产仅 PG**，Memory 仅测试/开发 |
| 4 | Crate 边界 | **按域拆分**；**Write + heavytail 先拆** |
| 5 | RAG 执行面 | **Execute-plan 已弃用**；运行时 **只认 AgentLoop + ToolCall** |
| 6 | 索引完成 / 长尾检索 | **Text 完成即可搜**；graph/mm/triplet **不单独限流、统一计量**；不做成付费权限门 |
| 7 | 计费主体 | **User 为唯一计费主体**；**后台任务不计客户配额** |
| 8 | Admin | **同一套设计系统**；指标 **全部可见**；**仅 super-admin 可增删改 org-admin** |
| 9 | Desktop LLM | **本地 LLM 用量不上报**；“独立版本线/SLA”释义见下 |
| 10 | 数据保留 | 数据源可服务分析；**保留 1 年**；**可导出给客户** |
| 11 | CI 门禁 | “合并门禁最小集”释义见下；**Real LLM 成本由产品侧承担** |
| 12 | 仓库/文档/生成物 | **单仓**；文档 **全部归集 docs/**；**生成物 gitignore、不落库** |

---

## Detailed decisions

### 1. 用量：Rolling 唯一真相 + 软限事后扣

- **真相源**：`llm_usage_events`（exit metering / rolling 窗口）为 token 与限额判定的权威数据。
- **预检**：不再以“估算 token 硬拦”为产品主语义；采用 **软限**（可提示、可 shadow、可最终以实量纠偏）。
- **扣减**：LLM 调用成功后的 **实量事后记账**（UsageObserver）驱动 rolling；月度/`usage_events` 若仍存在，必须 **派生自或对齐** rolling 实量，不得成为第二套互相打架的数字。
- **工程后果（待做）**：
  - preflight 硬 `ensure_metric_quota` 与估算路径收敛或降级为 soft；
  - 产品 UI 只读 rolling；
  - 文档/API 错误码区分 soft warn vs hard block（若仍保留 hard 仅作滥用防护）。

### 2. Write 为第四标准模式

- 模式集合（用户可选）：**Chat | RAG | WebSearch | Write**。
- **全租户可用**（无单独付费门）。
- 计费：**统一进 user rolling**；feature/stage 可内部打标（如 `write:refine`），**产品侧不单独拆 write 账单行**。
- 文案：只提示该模式 **可能消耗较多额度**，不展示“Write 专用费用”。

### 3. Memory 模式 vs 生产 PG（详细解释）

#### 现在系统里有两套“存数据”的方式

| | **PostgreSQL（生产）** | **Memory（进程内）** |
|--|------------------------|----------------------|
| 数据在哪 | 数据库 | API 进程内存里的 `MemoryState` |
| 重启后 | 还在 | **全丢** |
| 多实例 | 共享同一 DB | **每进程一份，不共享** |
| 谁用 | 真实部署、e2e 带 PG | 历史上用于无 DB 启动、部分单测 |

#### 技术上我们刚做完的事（C1）

- 业务代码（sessions/citations 等）**不再写两套 if PG / else memory**。
- 两边都实现同一个 **port**（如 `ChatPersistencePort`）：Memory 适配器 + PG 适配器。
- 这解决的是 **代码双分支**，不是产品要不要支持“无数据库部署”。

#### 仍然要拍板的产品问题

“用户/客户能不能在 **没有 PostgreSQL** 的情况下，把产品当正式环境用？”

- 若 **能** → 必须持续保证 Memory 与 PG 行为一致（权限、搜索、会话、配额边界），成本高，且 **无法横向扩容、无法持久化**。
- 若 **不能**（推荐且与本次决策一致）→ Memory **只服务单元测试 / 本地 smoke**，生产 bootstrap **强制 PG**；后续可继续删生产路径上的 memory 特化。

#### 本次确认方向

- **生产：只认 PostgreSQL。**
- **Memory：仅开发与测试适配器**，不是对外产品形态。
- 不承诺 “zero-infra 单二进制正式服”。

### 4. 按域拆 crate；Write + heavytail 优先

- 目标：`app-chat` 不再吞掉所有会话/agent/write 逻辑。
- **第一刀**：抽出 **Write 编排 + heavytail**（产品上是第四模式，代码上变化最独立、依赖最重）。
- 后续可再拆 sessions / agents 等；原则是 **统一接口（模式枚举、计量、事件）便于扩展第五模式**。

### 5. 只认 AgentLoop + ToolCall

- **Execute-plan 请求/兼容路径视为已弃用**，不再作为运行时主契约。
- 检索与工具调用统一走 **Agent Loop + `ToolCall`**。
- 工程后果：contracts 上 execute-plan DTO 可保留过渡期，但 **运行时入口、prompt、前端、文档** 不得再引导 execute-plan；兼容代码设删除期限（建议跟下一 major / 固定版本）。

### 6. 索引与长尾检索

- **可搜标准**：正文 text 索引完成即可检索。
- Graph / multimodal / triplet 等为 **质量增强长尾**，失败可降级，**不单独限流、不单独卖权限**。
- 计量：**统一进 user rolling**（不单独 bucket 售卖）。

### 7. 计费主体与后台任务

- **计费主体 = User**（B2C user-level，与 ADR 0001 一致方向）。
- **后台任务**（worker 入库 embedding/summary/triplet、reindex、清理等）**不计入客户配额**。
- 工程后果：worker 的 UsageObserver 可用于 **内部可观测/成本核算**，但 **不得驱动对用户的 rolling 阻断**；需明确 `usage_kind` / subject / 过滤规则，避免误伤。

### 8. Admin

- UI：**与主站同一套设计系统**。
- 指标：**全部对具备 admin 入口的角色可见**（读）。
- 写权限：**仅 super-admin 可增删改 org-admin**（及对应高危管理写操作）；org-admin 与 super-admin 的差集以“能否改管理员集合”为核心，而非藏指标。

### 9. Desktop：本地 LLM 不上报；“独立版本线与 SLA”释义

#### 已拍板

- **桌面端本地 / BYOK LLM 用量不上报到云端计费系统**（与“管道不收费、许可/软件本身收费”的混合模式一致，参见 ADR 0004）。

#### “独立版本线与 SLA”是什么意思（解释，不是再逼你二选一）

这是在问：**桌面客户端和云端 SaaS，是否当成两个产品运营。**

| 概念 | 含义 | 若“独立”则要准备什么 |
|------|------|----------------------|
| **版本线** | 发版节奏、版本号、兼容矩阵 | 例如桌面 1.x 可落后云 API；changelog 分列；强制升级策略 |
| **SLA** | 可用性/支持承诺 | 云端可承诺 99.x% API；桌面通常是 **best-effort** 或按许可证支持窗口，而不是“桌面进程 99.9% 在线” |
| **支持边界** | 谁修什么 | BYOK 密钥错误、用户本地模型挂掉 → 是否算我们的 P1 |

**与你已选“本地用量不上报”的关系：**  
桌面更像 **卖客户端 + 可选连云**，不是 SaaS 配额产品的一部分。因此通常：

- 云端：订阅 + rolling 配额 + SLA（若对外承诺）；  
- 桌面：许可证 + 本地算力用户自担 + **无 token 上报**；连云功能若登录 SaaS，则 **仅云端路径** 走 rolling。

若以后桌面强制绑云账号且所有调用走你们的代理，才需要重新讨论上报与 SLA；**当前决策不走这条。**

### 10. 分析数据

- 可用 `llm_usage_events` / analytics 等作为分析源（以 rolling 实量为准对齐口径）。
- **保留 1 年**。
- **支持导出给客户**（范围/格式后续规格化：按 user、时间窗、CSV/JSON 等）。

### 11. “合并门禁最小集”释义

#### 是什么

**合并门禁最小集** = 往 `master` 合 PR 时，CI **必须全绿** 的那一组检查；不在集合里的可以 nightly / 人工。

#### 为什么要定

全量 product_e2e + real LLM 又慢又贵又 flaky。不定最小集会导致：

- 要么门禁过重（合并一天合不进去），  
- 要么门禁过虚（红了也合，质量失控）。

#### 建议的最小集（实现时可再固化到 CI）

| 必跑（merge gate） | 非必跑（nightly / 手工） |
|--------------------|---------------------------|
| `cargo check` / 关键 crate test | 全量 `product_e2e` integration |
| 契约/关键单元测试 | Real LLM e2e（成本你承担，建议 nightly） |
| frontend `tsc` + 关键 vitest | rag_quality 全量 |
| 格式/lint（若已有） | 多租户长稳压测 |

**你已确认：Real LLM 成本由你承担** → 适合放 **nightly 必看、不阻塞每一笔小 PR**（除非改 LLM 协议/计费核心路径可可选加跑）。

### 12. 单仓、文档、生成物

- **保持 monorepo**。
- **文档单一入口：`docs/`**（ADR、评审、运维说明迁入或只从 docs 链接；根目录不新增长期文档）。
- **生成物不落库**，gitignore 至少覆盖：
  - `graphify-out/`（含各 crate 子路径）
  - `avrag-rs/heavytail-out/`
  - `avrag-rs/prompts/_backups/`
  - 其他本地实验/AST cache 输出

---

## Consequences

### Positive

- 计费口径单一，减少“两个数字对不上”。
- 模式模型清晰：四标准模式，Write 不另起商业套件。
- 运行时契约收敛到 AgentLoop/ToolCall，利于删 execute-plan 死海。
- 后台任务不计配额，避免“用户上传文档把自己刷爆”。
- 文档与生成物卫生提升，降低新人噪音。

### Risks / follow-ups

1. **软限实现**：需定义何时仍 hard-block（滥用/绝对上限）。  
2. **Worker 计量**：内部可观测 vs 用户配额过滤规则要写进代码与测试。  
3. **Write 拆 crate**：行为冻结测试先于搬文件。  
4. **Execute-plan 删除窗口**：前端/脚本/文档检索残留。  
5. **导出与 1 年保留**：存储与合规（删除账号是否级联）。  
6. **.gitignore 扩展**：避免本地生成物再污染 `git status`。

---

## Non-goals (this ADR)

- 不在此 ADR 内完成全部实现；仅锁定产品/架构方向。  
- 不重新打开 Desktop 是否卖订阅 token（本地不上报已定）。  
- 不把 graph/triplet 做成独立 SKU。

## References

- ADR 0001 user-level billing B2C  
- ADR 0004 desktop hybrid business model  
- `avrag-rs/docs/reviews/THERMO_NUCLEAR_REVIEW_2026-07-09_POST_WIP.md`  
