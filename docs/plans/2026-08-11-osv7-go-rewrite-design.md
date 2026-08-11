# osv7 总体设计：Go 后端 × pi 核心 × 检索/摄入 MCP

**状态：** 草案（讨论中，未动工）
**日期：** 2026-08-11
**前置：** `2026-08-11-harness-mcp-open-l0-design.md` —— 契约层权威（题卡、三层闸、证据句柄、preflight/错误契约、DocumentIr、平台计费、web 归 agent 侧、担保边界）。本文档是**实现载体权威**：osv7 用 Go 重写后端，pi 为唯一 agent 运行时，前端复用 frontend_next。

## 0. 定位与驱动

osv7 = context-os 的重写版。前置文档定下的所有契约原样继承；变的是载体：

- **Go 后端**：显性驱动是分享等公开只读端点的高并发（goroutine 每连接成本、无状态扇出）；隐性驱动是一语言栈 —— v6 Rust 187k 行中约 1/3（55–60k 行）随自研 loop 删除（前置文档 §10），剩余部分 Go 重写的边际成本低于「Rust 残核 + Go 新层」的长期双栈成本。
- **pi 为唯一 agent 运行时**：智能（检索推理、合成、web）全部在 pi 侧；osv7 不实现任何 loop。
- **检索/摄入双 MCP**：harness 价值本体，Go 实现；主 agent 与外接 agent 同一条路径（前置文档 §2 原则 5）。
- **数据库、队列模块化**：正交、边界清晰（§2 纪律机械执行）。

**与 v6 的关系**：v6（avrag-rs）转 maintenance-only，旧 loop 已 hospice（修复冻结，前置文档 §8）；osv7 各阶段达标前 v6 继续服役；P5 达标后 v6 整体退役删除，永不双栈并存。

## 1. 总体形态

| 组件 | 角色 | 备注 |
|---|---|---|
| `osv7`（Go 单体） | API + 全部业务模块 | 模块化单体，部署一个二进制 |
| `pi` | agent 运行时（主 agent） | 会话/进程模型待 P0 spike |
| Postgres + pgvector | 元数据 + 向量 | **Milvus 退役**（单机原则；v6 已支持 pgvector 后端） |
| Redis | 队列 + 缓存 | 沿用 |
| MinIO | 文档 blobs | 沿用 |
| 外部能力 | embedding / rerank（HTTP）、PaddleOCR、解析器进程 | 平台计费或 BYOK（前置文档 §3） |
| frontend_next | 前端复用 | API 形状对齐（§5） |

**为什么模块化单体而不是微服务**：solo trunk、单机部署，边界用 package 纪律表达即可；唯一有独立扩缩迹象的是 share 端点，而它无状态，直接水平复制整个二进制即可。

## 2. 模块分解（正交纪律）

机械执行的边界规则：

1. SQL 只允许出现在 `store` 与 `index`；Redis 命令只允许出现在 `queue`；MinIO SDK 只允许出现在 `store`。
2. `retrieval-mcp` / `ingest-mcp` 不知道 pi 的存在（服务任意 MCP client）；`agentd` 不知道检索/摄入语义（只管 pi 会话与字节流）。
3. 计费扣款与能力判定（hosted|byok|missing）只经 `billing`。
4. 所有 LLM 面向文案（nudge / 反馈 / 错误 remediation）仍是 md 资产，Go 侧 `embed.FS` 打包；模型上下文里出现的任何 host tag 在 `prompts` 包的 markers 注册表登记 + parity 测试（沿用 v6 host_markers 规则）。
5. 包依赖单向无环：`api → {agentd, share, workspace, billing}`；`{retrieval-mcp, ingest-mcp} → {index, store, billing, queue}`；`agentd → billing`。

| 包 | 职责（拥有） | 明确不做 |
|---|---|---|
| `api` | HTTP/SSE 端点、authn、请求校验、rate limit | 业务逻辑 |
| `share` | 公开只读分享端点（高并发：无状态、ETag / Redis 缓存） | 写路径、触达 pi/LLM |
| `agentd` | pi 会话管理：拉起/附着、provider 注入（平台 key / BYOK）、流中继、**出站薄闸**、pi 用量采集 | 检索语义、消息内容解释 |
| `retrieval-mcp` | MCP server：题卡 schema + 双模式校验、资源闸/契约闸、SaC 原语（dense/lexical/grep/struct/doc_summary）、证据句柄（alias/SELECTED/KEEP）、可选 verify_draft（句柄级） | 摄入、web |
| `ingest-mcp` | MCP server：DocumentIr 契约、ingest_begin/blocks/summary/kg/commit、硬校验、preflight（能力表 + 扫描件嗅探）、解析编排、归一化 | 检索 |
| `index` | pgvector 向量读写、chunk 存取、lexical（tsvector）、rerank 调用、结构目录 | 元数据业务 |
| `store` | PG 元数据（RLS）、MinIO blobs、schema migrations | 向量运算 |
| `queue` | Redis 异步任务（摄入管线、OCR、embed 批），有界终态纪律（done \| failed(reason)，queued 悬挂是 bug） | 业务状态机 |
| `billing` | 能力表、余额扣费、usage 事件、BYOK key 保管（加密）、wallet | 检索/摄入逻辑 |
| `identity` | 用户、workspace、成员、RLS 上下文（T7/T8：user_id / workspace_id） | — |
| `prompts` | md 文案资产 + markers 注册表 | — |

## 3. pi 集成（主 agent 形态）

- 插件栈：**websearch**（deepseek 原生检索）+ **harness MCP client** + **card-keeper**（声明-观测闸、事卡重锚；硬度边界 P0 实测，见 §9.2）。
- web 归 agent 侧：osv7 不提供 web 原语（前置文档 §4）。
- **出站薄闸在 `agentd` 出口**：用户气泡只见 pi 自然语言终答，协议残片 / tool transcript 拦截（与 pi 内 soft 闸互补；硬交付闸以 agentd 为兜底，见 §9.2）。
- 计量：pi 的 LLM 用量由 `agentd` 采集 → `billing` 扣余额（平台模式）。
- 会话持久化（**已定倾向，P0 只验证可实现性**）：**pi transcript 为真源**（append-only 事件流）+ **PG 为 UI / 列表 / 搜索 / 分享投影**（可从 transcript 重建，禁止 UI 气泡反写为 agent 真源）。业界对照见 §9.3。

## 4. 数据决策与迁移

- **向量标准化 pgvector**：Milvus 退役，少一个服务（v6 的 qdrant VPS 已取消，同一单机原则）。
- **embedding 模型不变**：服务端锚定模型与 v6 一致 → 现库向量直接可用，数据零重嵌入；P1 检索腿可直接吃 v6 现库数据验证。
- 迁移策略（倾向）：pgvector / chunk 表结构沿用 v6；元数据（workspace / document）映射迁移脚本；MinIO buckets 原样挂载。
- chat_* 历史会话不迁移（旧 loop 会话与 pi 会话模型不同构）；v6 退役时归档导出，不进 osv7。

## 5. 前端复用与 API 对齐

- frontend_next 业务页面不动；`lib/api` 客户端层重指向 osv7 端点。
- Go API **逐端点对齐 v6 `transport-http` 的现状形状**（前端消费什么就给什么），不让前端适配新 API。
- chat SSE：事件形状保留（消息流 / 活动事件），内容改为 pi 中继 + MCP 事件投影；dashboard 活动面板从 MCP 调用事件重建（粒度不足则面板降级，前置文档 §9）。
- 契约共享（**P1 前定倾向**）：**OpenAPI 为 HTTP 真源 + 生成 TS client**；SSE / 活动事件等生成器吃力处手写；tygo 仅在确需共享非 HTTP 结构时补刀（见 §9.5）。

## 6. 阶段计划（每层有验证闸，不过闸不推进）

- **P0 spike（天级）**：**已完成 2026-08-11** —— 详见 `osv7/docs/p0-spike-findings.md`。摘要：stdio MCP lexical 命中 v6 `rag_text_chunks`；pi 经 **pi-mcp-adapter** 接 MCP；`tool_call` 可 hard-block；transcript 真源 JSONL 且**首条 assistant 前不落盘**；RPC ≈ 一会话一 Node 进程（~110–120MB RSS）。未烧 LLM 全链路 agent 轮（P2 补）。
- **P1 检索腿**：**已落地 + 子集验收 2026-08-11**（`osv7/docs/p1-retrieval-findings.md`）—— MCP 薄切片 + `cmd/retrieval-eval` Layer A。本机 `available` 子集 **hit_rate 0.769（10/13）**（110 题无本地语料 skip）。全量 149 语料与 pi 端到端 → 导入语料 / P2。
- **P2 主 agent 接通**：**收口 2026-08-11**（`osv7/docs/p2-agentd-findings.md`）—— pi RPC + 闸 + harness 检索 + HTTP/SSE + **多轮** + **PG 投影**（`osv7_sessions/messages`）+ card-keeper **软信号**。**未做**：前端灰度、websearch/dual-web 事故复测、计费真扣。
- **P3 摄入腿**：**薄切片 2026-08-11**（`osv7/docs/p3-ingest-findings.md`）—— DocumentIr + preflight/硬校验 + `ingest-mcp`/`ingest-cli`；双生产者（agent 包 + server 文本切分）commit 后 lexical 可见。**未做**：Redis queue、anydoc/markitdown/OCR 适配器、MinIO、KG 检索。
- **P4 分享与计费硬化**：**薄切片 2026-08-11**（`osv7/docs/p4-share-billing-findings.md`）—— `osv7d` 钱包扣费（chat/embed）+ BYOK + 余额地板 402；`GET /public/s/{token}` ETag/304。**未做**：压测基线、对接 v6 wallets、支付充值。
- **P5 切换与退役**：full-149 A/B（pass ≥ 109/149 基线，token 显著低于 47.8k 均值）→ 默认切 osv7 → v6 删除。

## 7. 契约平移表（v6 → osv7）

| 契约 | v6 位置 | osv7 落点 |
|---|---|---|
| 题卡（必填、双模式校验） | loop 内模型自报（提示词讨要） | `retrieval-mcp` MCP schema 必填字段 + 校验器 |
| required_action 闸 | `required_action_missing_continue`（结构闸） | `retrieval-mcp` 契约闸（tool result 反馈） |
| 证据句柄 alias / SELECTED / KEEP | SaC bridge 线协议 | `retrieval-mcp` 线格式，原样沿用 |
| verify_draft（句柄级） | verify skill（LLM） | `retrieval-mcp` 可选工具（只做引用-句柄校验，无 LLM） |
| DocumentIr + 硬校验 | `crates/ingestion/src/ir.rs` | `ingest-mcp`（Go 重定义，字段级对齐） |
| preflight / 能力表 / 错误契约 | （L0 设计，v6 未实现） | `billing` + `ingest-mcp` |
| host tag 注册 + parity 测试 | `host_markers.rs` | `prompts` 包 markers 注册表（Go）+ 同款 parity 测试 |
| 平台计费（余额地板 / BYOK 可选） | （L0 新决，v6 未实现） | `billing` |
| 旧 loop 全部机制 | agent-loop 等约 55–60k 行 | **不迁移** |

## 8. 已排除项

- **Rust 残核 sidecar**（v6 检索层保留为服务）：违反一语言栈与边界清晰；pgvector 上的检索执行是 SQL + HTTP，Go 重写成本可控。
- **Milvus 支持**：pgvector 标准化；规模真到时再议。
- **微服务拆分**：模块化单体 + package 纪律足够；share 高并发由无状态水平复制解决。
- **旧 loop 任何代码/机制迁移**：hospice 已冻结（前置文档 §8）。

## 9. 业界对照与默认收敛（2026-08-11 检索）

对照 MCP 规范、企业 MCP 部署文、Claude Agent SDK 托管模型、Google ADK 会话分层、pi 扩展/会话文档。**结论：原 §1–§5 默认决策与主流一致，无需推翻**；下列为细化默认 + 仍待实测项。

### 9.1 MCP 传输与生命周期

| 做法 | 来源要点 | osv7 默认 |
|---|---|---|
| **stdio** | 规范推荐 client 尽量支持；client 拉起子进程，零网络/隔离白送；适合本机 IDE/CLI | **P0 hello 可用**；不当作多租户 prod 主路径 |
| **Streamable HTTP** | 规范正式传输（取代旧 HTTP+SSE 双端点）；单 endpoint POST+可选 SSE；可多连接、`Mcp-Session-Id`、鉴权头 | **prod 主路径**：`retrieval-mcp` / `ingest-mcp` 以 HTTP 服务任意 MCP client |
| 生命周期 | stdio = client 管生死；HTTP = server 常驻。企业侧共识：stdio 是 N×M 子进程 + 分散审计；共享状态（索引/连接池）应在 HTTP 服务端 | MCP **共享服务进程**，不「每用户一个 MCP 子进程」 |
| 懒连接 | pi-mcp-adapter 等：代理 tool、首调再连、空闲断连、stdio/HTTP 双支持 | pi 侧可经 adapter/extension；harness 本身保持无状态 tool 语义优先 |

**多租户 MCP 安全（补充默认，P1 写进契约）：**

- **身份在 transport，不在 tool 参数。** `workspace_id` / `user_id` **不得**仅靠模型填的 tool arg 裁定；从已验证的 session/token 注入 RLS 上下文，模型声明的 scope 只能在「会话已授权集合」内收窄，不能拓宽（社区多租户 MCP 踩坑：模型被诱导换 workspace）。
- Streamable HTTP：**Origin 校验**、鉴权（Bearer / 产品 session）、审计落在 HTTP 入口（gateway 友好）。
- DB 侧继续 **RLS + workspace 过滤**（与 v6 T7/T8 一致）。

### 9.2 插件 / 闸：软硬分层

| 做法 | 来源要点 | osv7 默认 |
|---|---|---|
| 提示词不可作唯一护栏 | Claude Code hooks 实践：PreToolUse 可 hard-block；*Prompts ask; hooks enforce* | 闸分三层（L0 已定）：资源/契约在 MCP；行为在 agent；交付在 agentd |
| 工具路径硬闸 | PreToolUse / canUseTool 类 API | **card-keeper**：能挂 pi extension 的 tool 事件则做声明-观测；**能否拦 deliver 以 P0 实测为准** |
| 出站边界 | 产品 harness 自有 | **`agentd` 出站薄闸为交付兜底**（协议残片 / tool transcript 永不进用户气泡）——不依赖 pi 是否提供 Stop hook |
| pi 扩展事实 | pi 有 TypeScript extensions + 生命周期事件（session_start / tool 相关事件）；MCP 非核心内置，靠 extension/adapter | 插件栈落点：websearch + MCP client + card-keeper 均为 extension；硬度表 P0 填 |

**若 P0 发现 pi 只能 soft 观察、不能 hard-block deliver：** 不阻塞架构——契约闸仍在 MCP tool result；交付硬闸在 agentd；card-keeper 降级为 observation + telemetry（逃生口仍是改卡）。

### 9.3 会话真源：transcript + 投影

| 做法 | 来源要点 | osv7 默认 |
|---|---|---|
| append-only transcript | Claude Code：JSONL 会话文件；resume/fork/rewind 从 transcript 重建；故意不把 DB 当会话内核 | **pi transcript = 真源** |
| Session = ground truth；working context = computed projection | Google ADK 生产架构表述 | 模型上下文窗口是派生视图，可 compact/裁剪，**不**替代完整事件流 |
| 跨机 resume | Claude Agent SDK：本机 JSONL + SessionStore 镜像到共享存储（S3/PG 等）才能跨 host | agentd：**transcript 落盘或对象存储**；PG 只存会话元数据 + UI 投影行 |
| 投影单向 | event-sourcing：读模型可丢可重建 | PG 气泡/列表/分享 **只读投影**；禁止 UI 编辑反写覆盖 transcript |

与「chat_* 不迁移」一致：旧 loop 消息 ≠ pi 事件流，v6 退役归档导出即可。

### 9.4 进程模型：三层拆开

不要混成一个「池化 vs 每会话」问题——业界按层决策：

| 层 | 业界主流 | osv7 默认 |
|---|---|---|
| **Agent 会话状态** | 一会话一条 transcript；进程可灭，状态在盘 | pi 热会话可附着；冷会话落盘后回收进程 |
| **Agent 运行时进程** | Claude Agent SDK 托管：**一活跃 session ≈ 一子进程**（N 并发 = N 进程树）；不是共享线程池跑多会话 | **不追求跨会话池化 pi 解释器**；用「热附着 + 空闲回收 + 并发上限」控资源。P0 量 RSS 与 resume 延迟 |
| **Harness MCP** | HTTP 多路复用；与 agent 进程解耦 | Go 进程内（或同二进制）HTTP MCP，水平复制整个单体 |
| **不可信代码执行** | ephemeral sandbox（gVisor/Firecracker 等）；不为每用户长期占热沙箱 | 若用 pi 自带执行环境：按执行隔离；**不**与 MCP 进程模型绑死 |
| **无状态只读（share）** | 水平复制 | 已定 |

**对 agentd 的含义：** 角色是「会话投影 + 按需拉起/附着 pi + 流中继 + 出站闸 + 用量」，不是长期占坑的进程池调度器。并发上限与 idle timeout 是配置，不是架构分叉。

### 9.5 Go→TS 契约

| 候选 | 适用 | 默认 |
|---|---|---|
| **OpenAPI → 生成 TS client** | HTTP 端点形状 = 前端真实消费面 | **P1 采用** |
| 手写 | SSE 事件流、活动面板细粒度 | 保留 |
| tygo | 非 HTTP 共享结构（若有） | 按需，不作主路径 |

### 9.6 仓库形态（已拍板）

| 选项 | 利 | 弊 |
|---|---|---|
| 新 repo 第一天 | 产品身份清晰；P5 删 v6 干脆 | P0–P1 吃 v6 现库 / 共用 `.env` / 双后端窗口摩擦大；过早挪 frontend 易乱 |
| monorepo `osv7/` 起骨架，P2 后可 split | P1 共享 PG/MinIO/env；v6 维护与 osv7 开发同仓 | 历史与「删 v6」稍软 |

**已定（2026-08-11）：P0–P1 在 monorepo `context-osv6/osv7/` 起骨架；`frontend_next` 暂留原位；v6（`avrag-rs`）maintenance-only 同仓并行。** P2 灰度前后再议是否 split 为独立 repo 或长期 monorepo；P5 退役 v6 时再处理仓边界，不阻塞主路径。

### 9.7 P0 实测对照表（产出检查清单）

| # | 问题 | 业界期望答案 | 实测要记下的 |
|---|---|---|---|
| 1 | pi 如何连 MCP | 扩展/adapter；stdio 与 HTTP；懒连接 | transport 列表、鉴权头能否注入、首调延迟 |
| 2 | card-keeper 硬度 | tool 前/后事件；deliver 硬拦视 runtime | 有无 PreTool/Stop 等价物；不能硬拦时的降级路径（agentd 兜底已定） |
| 3 | 会话真源 | JSONL/树状 transcript + resume/fork | 文件格式、路径、跨进程 resume、与 PG 投影字段映射草稿 |
| 4 | 进程模型 | 活跃会话≈子进程；冷回收 | 10/50 会话 RSS、kill 后 resume、并发上限建议值 |
| + | hello-retrieval | pi → MCP → 现库一条命中 | stdio 可；注明 prod 改 HTTP 的差距 |

---

## 10. 开放问题（收敛后）

**P0 只验证，不再「二选一空转」的：**

- 会话真源 = pi transcript；PG = 投影（§9.3）。
- prod MCP = Streamable HTTP；stdio 仅 dev/P0（§9.1）。
- 交付硬闸兜底 = agentd；card-keeper 尽量硬、可软（§9.2）。
- agent 进程 = 热附着 + 冷回收，不跨会话池化 pi（§9.4）。
- Go→TS = OpenAPI 主路径（§9.5）。

**已拍板：**

- 仓库形态 = monorepo `osv7/` 起（§9.6，2026-08-11）。

**P0 已填（见 `osv7/docs/p0-spike-findings.md`）：**

1. MCP：pi 用 **pi-mcp-adapter** + `.mcp.json`；P0 stdio 已通；prod 仍 HTTP。
2. 硬度：`tool_call` **可 hard-block tool**；用户气泡 deliver 硬闸仍在 **agentd**。
3. 会话：JSONL 真源；**无 assistant 消息前不落盘**（agentd 须知）。
4. 进程：一活跃 RPC 会话 ≈ 一 Node 进程，~110–120MB RSS；不池化 pi。

**仍待后续阶段定：**

1. **transcript 物理存放**：本机盘 vs MinIO/对象存储镜像（跨机/重启；P1 agentd 前定即可）。
2. **MCP 鉴权具体形态**：产品 session 换短时 MCP token vs 网关透传用户 JWT（P1；原则已定：scope 不信模型 arg）。
3. **heavytail / write_refine**：编辑原语是否 MCP 化——**P3 后议**，不挡主路径。
4. **dashboard 活动面板粒度**：MCP 事件投影不够则降级（前置 L0 §9）——P2 验收时看。
5. **是否 split 独立 repo**：P2 后再议，不阻塞 P1。
6. **带 LLM 的 pi→mcp 一轮冒烟**：P2 接通时补（P0 未烧 token）。

**不开放（已排除，见 §8）：** Rust sidecar、Milvus、微服务拆分、旧 loop 机制迁移。
