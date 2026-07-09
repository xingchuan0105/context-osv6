# ADR 0006: 产品与架构决策（TN 复审后拍板）

## Status

**Accepted** — 2026-07-09（产品确认；同日补全 #3 / #9 / #11 终裁）

**Review acceptance** — 2026-07-09：产品方 **同意** 下文「Product & architecture review」的总评、强项、张力补规则、一致性评分与「不建议轻易推翻」清单；下列 **补充规则** 升级为与正文决策同等效力的实现约束。

## Context

Thermo-Nuclear 结构债主路径（C1–C3 / H1–H3 / 长尾）落地后，剩余阻塞项多为 **产品口径与架构边界**。本 ADR 记录全部 12 项决策的最终口径，并附产品/架构评审纪要（已获产品接受）。

---

## Decision summary（终裁）

| # | 主题 | 终裁 |
|---|------|------|
| 1 | 用量真相 | **Rolling 为唯一真相**；配额策略 **软限 + 事后扣减** |
| 2 | Write 模式 | **全租户开放**；**第四标准模式**（Chat / RAG / WebSearch / Write）；计费文案只提示“用量大”，**不单独展示 write 成本** |
| 3 | Memory 运行时 | **不要** Memory 作为正式产品形态 → **生产仅 PostgreSQL**；Memory **仅测试/开发适配器** |
| 4 | Crate 边界 | **按域拆分**；**Write + heavytail 先拆** |
| 5 | RAG 执行面 | **Execute-plan 已弃用**；运行时 **只认 AgentLoop + ToolCall** |
| 6 | 索引 / 长尾检索 | **Text 完成即可搜**；graph/mm/triplet **不单独限流、统一计量**；不做成付费权限门 |
| 7 | 计费主体 | **User 为唯一计费主体**；**后台任务不计客户配额** |
| 8 | Admin | **同一套设计系统**；指标 **全部可见**；**仅 super-admin 可增删改 org-admin** |
| 9 | Desktop | **本地 LLM 用量不上报**；桌面与云端 **独立版本线 + 独立 SLA** |
| 10 | 数据保留 | 分析数据源可用；**保留 1 年**；**可导出给客户** |
| 11 | CI 门禁 | **按建议的 merge gate 最小集**；全量 e2e / Real LLM 走 nightly；**Real LLM 成本由产品侧承担** |
| 12 | 仓库 / 文档 | **单仓**；文档 **全部归集 `docs/`**；生成物 **gitignore、不落库** |

---

## Detailed decisions

### 1. 用量：Rolling 唯一真相 + 软限事后扣

- **真相源**：`llm_usage_events`（exit metering / rolling）为 token 与限额判定的权威数据。
- **预检**：产品主语义为 **软限**（提示 / shadow / 可纠偏），不以估算 token 硬拦为主路径。
- **扣减**：LLM 成功后的 **实量事后记账**（UsageObserver）驱动 rolling。
- **月度 / `usage_events`**：若保留，必须 **派生或对齐** rolling 实量，禁止第二套对账数字。
- **工程后果**：收敛 `ensure_metric_quota` 硬拦；UI 只读 rolling；错误码区分 soft warn vs 滥用 hard block。

### 2. Write 为第四标准模式

- 用户可选模式：**Chat | RAG | WebSearch | Write**（全租户）。
- 计费：统一进 **user rolling**；内部可打 `write:<phase>`，**产品账单不拆 write 行**。
- 文案：仅提示可能 **用量较大**，不展示 Write 专用费用。

### 3. Memory：**不要**正式产品形态（终裁）

**产品问题（已答「不要」）**  
客户 **不能** 在无 PostgreSQL 的情况下把本产品当正式环境使用。

| | PostgreSQL（生产） | Memory（进程内） |
|--|-------------------|------------------|
| 持久化 | 有 | 无（重启即空） |
| 多实例 | 共享 | 不共享 |
| 产品承诺 | **唯一正式存储** | **无产品承诺** |
| 允许用途 | 部署 / e2e 带 PG | 单测、本地无 DB 开发夹具 |

**工程后果：**

- 生产 bootstrap **强制 DATABASE_URL / PG**；无库则拒绝以“正式服”启动（开发 flag 可另开）。
- Memory 适配器可保留服务测试，但 **不出现在产品文档 / 部署手册 / 销售口径**。
- 持续删除生产路径上对 `uses_memory_adapters` 的业务特化。

### 4. 按域拆 crate；Write + heavytail 优先

- 第一刀：`Write` 编排 + `heavytail` 抽出独立 crate（或 workspace member）。
- 模式枚举、计量、事件接口统一，便于扩展第五模式。

### 5. 只认 AgentLoop + ToolCall

- Execute-plan **运行时弃用**；主契约 = Agent Loop + `ToolCall`。
- DTO 可过渡保留；入口 / 前端 / 文档设 **删除期限**。

### 6. 索引与长尾检索

- **可搜标准**：text 索引完成。
- Graph / multimodal / triplet：**质量增强、可降级**；不单独限流、不单独卖权限；统一进 user rolling。

### 7. 计费主体与后台任务

- **计费主体 = User**。
- Worker 入库 / reindex / 清理等 **不计客户配额**（可内部可观测，不驱动对用户 rolling 阻断）。

### 8. Admin

- 与主站 **同一设计系统**。
- 指标：有 admin 入口则 **全可见（读）**。
- 写： **仅 super-admin** 可增删改 **org-admin**（及对等高危写操作）。

### 9. Desktop：不上报 + **独立版本线与独立 SLA**（终裁）

#### 已拍板

1. **本地 / BYOK LLM 用量不上报**到云端计费（与 ADR 0004 混合商业模式一致）。  
2. **桌面与云端按两个产品运营**：

| 维度 | 云端 SaaS | 桌面客户端 |
|------|-----------|------------|
| 版本线 | 独立版本号 / changelog / 发版节奏 | 独立版本号 / changelog / 发版节奏 |
| 兼容 | 服务端 API 兼容策略 | 客户端最低云 API 版本矩阵（可选连云时） |
| SLA | 可对 API/可用性做对外承诺（具体数字另定） | **独立 SLA 文档**（许可支持窗口、缺陷响应、不含“本机进程可用性 99.9%”） |
| 用量 | User rolling 配额 | 本地调用 **不计云配额**；登录云后 **仅云端路径** 进 rolling |
| 支持边界 | 服务端故障、配额、数据 | 安装/许可/UI；**BYOK 密钥与本地模型故障默认用户侧**（SLA 正文需写清） |

#### 必须补齐的运营产物（工程+产品）

- `docs/` 下分列：**Cloud release notes** vs **Desktop release notes**  
- **Desktop SLA / Support policy**（响应时效、不含项、升级政策）  
- 可选：Desktop 最低兼容云 API 版本表  

### 10. 分析数据

- 数据源：以 rolling 实量对齐的 `llm_usage_events` / analytics 等。  
- **保留 1 年**；**支持导出给客户**（格式/范围另开规格）。

### 11. CI 合并门禁最小集（终裁 = 采纳建议方案）

**定义**：合入 `master` 时 **必须绿** 的检查集合；其余 nightly / 人工。

| Merge gate（阻塞合并） | Nightly / 非阻塞 |
|------------------------|------------------|
| `cargo check` + 关键 crate / 契约单测 | 全量 `product_e2e` integration |
| frontend `tsc` + 关键 vitest | **Real LLM e2e**（成本产品侧承担） |
| 既有 lint/format（若 CI 已配） | rag_quality 全量、长稳压测 |

**例外（可选加跑 merge gate）**：改动触及 LLM 协议、计费/配额核心、auth 时，可要求相关 integration / real-LLM 子集。

### 12. 单仓、文档、生成物

- Monorepo 保持。  
- **文档唯一入口 `docs/`**（ADR 已归入 `docs/adr/`）。  
- 生成物不落库：`**/graphify-out/`、`avrag-rs/heavytail-out/`、`avrag-rs/prompts/_backups/` 等（见根 `.gitignore`）。

---

## Product & architecture review（对选择的评审）

> **产品接受记录（2026-07-09）**  
> 产品方明确同意本节总评、强项、张力处理方式、一致性评分与「不建议轻易推翻」清单。  
> 原「建议补规则」已升格为下节 **Accepted addenda（实现必须遵守）**。

### 总体判断（已接受）

**方向正确、内部一致性较好**，适合当前「SaaS 配额 + 可选桌面许可」的混合模型。最大风险不在单条决策，而在 **软限/双表计费收敛** 与 **双产品（云/桌面）运营纪律** 是否跟得上代码速度。

### 强项（已接受：建议保持）

| 决策 | 为何合理 |
|------|----------|
| Rolling 唯一真相 + 事后实量 | 与 exit metering 实现对齐，避免估算硬拦与实量对账分裂 |
| Write 第四模式、不拆账单行 | 降低商业复杂度；用文案管预期比拆 SKU 更轻 |
| 生产不要 Memory | 砍掉永久双运行时税；C1 port 化之后的自然收束 |
| User 计费 + 后台不计配额 | 符合 B2C；避免“上传文档把自己刷爆” |
| ToolCall-only | 与 agent loop 主线一致，利于删 execute-plan 死海 |
| Text 完成即可搜 | 正确的产品可用性门槛；长尾增强不绑售卖 |
| 文档单入口 + 生成物不入库 | 仓库卫生与 onboarding 成本下降 |

### Accepted addenda（评审张力 → 已接受的补充约束）

实现本 ADR 时 **必须** 满足下列补充规则（与 §1–§12 同级）：

1. **软限 + 滥用 hard-cap**  
   - 体验层：软限（提示 / shadow / 可纠偏）。  
   - 成本防护：**绝对上限仍 hard-block**（具体倍数或日封顶另开计费规格；在规格落地前不得删除一切硬拦）。  
   - 软限不得解释为“可无限调用”。

2. **Write 全员开放 × 不单列成本**  
   - 内部计量保留 `write:*`（或等价 feature/stage）供成本分析。  
   - 产品侧 **不在账单拆 Write 行**。  
   - **写模式入口必须可见**“可能消耗较多额度”类提示（模式选择器旁或等价位置）。

3. **后台不计配额 × Worker 可打点**  
   - 代码层区分 **billable user** vs **internal**（标志 / `usage_kind` / subject）。  
   - 用户配额查询 **默认过滤** 后台/内部行，禁止污染客户 rolling。  
   - 内部成本看板可单独看 worker 用量。

4. **桌面独立版本线 + 独立 SLA × 本地不上报**  
   - 运营接受双产品成本：分列 release notes、支持话术。  
   - SLA 必须写清：不含本机可用性；BYOK/本地模型默认用户侧；连云故障走 **云 SLA**。  
   - 口径见 `docs/desktop/SUPPORT-AND-SLA.md`（响应数字可后续填）。

5. **Admin 指标全可见**  
   - 指具备 admin 入口角色可见的 **产品/运营指标**。  
   - **PII 与跨 org 数据仍须租户隔离**；不得解读为任意 org 裸读。

6. **1 年保留 + 可导出**  
   - 必须另开规格：账号删除是否级联用量、导出异步任务与审计、是否含 prompt 片段。  
   - 存储与法务单独立项，不阻塞本 ADR 其余实现。

7. **Merge gate 从简 × Real LLM nightly**  
   - Nightly 失败 **必须有人认领**（责任人/轮值）。  
   - 触及 **LLM 协议、计费/配额核心、auth** 的 PR，可 **升级** 相关 integration / real-LLM 子集进 merge gate。

8. **按域拆 Write 优先**  
   - **先契约/行为测试锁 Write 行为，再搬 crate**。  
   - 禁止“先搬文件、后补测试”的拆分顺序。

### 一致性评分（主观，已接受为决策背景）

| 维度 | 评分 | 说明 |
|------|------|------|
| 商业模型自洽 | 高 | 云配额 vs 桌面许可 + 不上报，不互相拆台 |
| 计费可实现性 | 中高 | 依赖软限细节与 worker 过滤落地 |
| 架构可演进 | 高 | 四模式 + ToolCall + 域拆分 |
| 运营复杂度 | 中 | 双产品版本/SLA 需要纪律 |
| 用户可理解性 | 高 | 不拆 write 账单、text 即可搜 |

### 不建议轻易推翻的决策（已接受）

- 生产不要 Memory  
- Rolling 唯一真相  
- 后台任务不计用户配额  
- 本地 LLM 不上报  

### 实现 backlog（由本 ADR + 已接受评审派生）

1. ~~软限 + 滥用 hard-cap 规格与代码~~ ✅ `feat/adr-0006-metering-and-write-copy`  
2. ~~Worker 非 billable 计量过滤 + 测试~~ ✅ migration `0052` + observer tests  
3. ~~Write 入口用量提示文案~~ ✅ frontend Write mode + usage hint  
4. ~~Desktop 版本线文档完善 + SLA 响应数字~~ ✅ `docs/desktop/SUPPORT-AND-SLA.md` + `VERSIONING.md`  
5. ~~CI merge gate / nightly 分流~~ ✅ `avrag-rs/docs/e2e-gates.md`（认领轮值可运营侧再填人）  
6. Write+heavytail crate 拆分（**测试先行**）— **partial**：`write-core` 已抽出 material/refine 契约；编排仍在 `app-chat`  
7. ~~Execute-plan 运行时删除~~ ✅ 路由物理删除 + App 方法删除；DTO/内部 harness 过渡保留至 2026-09-30  
8. ~~用量导出 / 1 年保留~~ ✅ API + worker purge + 删号级联（`0053`/`0054`）

---

## Consequences

### Positive

- 生产存储与计费口径单一。  
- 模式与契约与主实现（AgentLoop）对齐。  
- 云/桌面边界清晰，避免 BYOK 掏空 SaaS。  
- 文档与仓库卫生可执行。

### Risks

- 软限实现不清 → 成本失控或体验过硬（已用 hard-cap addendum 约束）。  
- 双产品 SLA 写不清 → 支持扯皮（已要求独立 SLA 文档）。  
- Write 全员 + 不透明成本 → 入口文案 + 内部 `write:*` 监控兜底。

### Change log

| 日期 | 变更 |
|------|------|
| 2026-07-09 | 初稿：12 项决策 |
| 2026-07-09 | 终裁 #3 / #9 / #11；写入评审初稿 |
| 2026-07-09 | **产品同意评审结论**；张力项升格为 Accepted addenda |
| 2026-07-09 | 实现进度：#1–#5/#7/#8 文档与代码落地；#6 Write crate 拆分仍待 |
| 2026-07-09 | #6 partial `write-core`；#7 路由删除；#8 export API + 删号级联 |

---

## Non-goals

- 本 ADR 不完成全部实现，只锁定方向与已接受的实现约束。  
- 不重新打开“桌面按 token 向云上报”。  
- 不把 graph/triplet 做成独立付费 SKU。

## References

- ADR 0001 user-level billing B2C  
- ADR 0004 desktop hybrid business model  
- `docs/desktop/SUPPORT-AND-SLA.md`  
- `avrag-rs/docs/reviews/THERMO_NUCLEAR_REVIEW_2026-07-09_POST_WIP.md`  
