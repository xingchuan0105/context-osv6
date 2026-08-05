# ADR 0010: 分享服务商业模式——可分享 Workspace 订阅 + 模型代购储值

## Status

**Accepted**（2026-08-05 决策拍板；**2026-08-06 修订**：补代码现实差距、过渡顺序、BYOK 密钥安全、Publish 导出为新增、LiteLLM 边界、反扒补强、计费单位统一）

**Supersedes（商业叙事与收费对象）：**

- `docs/adr/0004-desktop-hybrid-business-model.md` — 桌面买断许可 + SaaS 卖 token 托管便利
- `docs/superpowers/specs/2026-06-07-pricing-tiers-revamp-design.md` — 以 token 滚动窗口为主商品的三档（文内已部分 SUPERSEDED；本 ADR 完成商品定义翻转）
- `docs/engineering/DEEPSEEK_STYLE_USAGE_BILLING_DESIGN_2026-07-13.md` — 作为**主套餐**的 rolling units / M 系数；事件表与出口计量可复用为**钱包扣费流水**，不再驱动 Free/Plus/Pro 主权益

**Does not supersede：**

- `docs/adr/0001-user-level-billing-b2c.md` — 仍为 user_id 级 B2C 计费
- T7/T8 workspace 产品真相、Creem + Alipay 渠道（扩展 product 即可）
- `avrag-rs/docs/engineering/2026-08-01-llm-providerpool-acceptance.md` — **内部**平台 LLM 路由（纯 Rust `ProviderPool`）；见 §8 边界

## Context

### 旧模型的结构性问题

1. **卖 token**：用户 BYOK 后管道价值归零；与「桌面本地 + 私有化」目标冲突。
2. **卖客户端许可**：与「让尽可能多人建库、再通过分享变现」相反；许可运维（Keygen 等）与核心壁垒无关。
3. **真实壁垒**：可 DIY 的高质量检索基础设施 + **可分享**（桌面 Agent / CLI 难分享；NotebookLM 类与厂商 token 强绑定）。

### 新产品命题

> 客户端免费、私有使用免费；**收费对象是「把 Workspace 经营成可被他人使用的服务」的能力**。  
> 模型算力：用户自配（BYOK）为主；平台代购储值为辅（**标价 = 官方价 × 1.5**，即 markup 50% / 约 33% 毛利，见 §3.1）。  
> 本地 pgvector 数据要分享时，须 **Publish 上云**（云端 PG 元数据 + Milvus 检索面）。

### 代码现实（2026-08-06 四路核对）— 目标 ≠ 起点

下列为**现行实现**，与下文 Decision 目标态对照。修订本 ADR 的动机：避免把目标写成仿佛已存在，从而低估 B1–B4。

| 主题 | 目标（本 ADR） | 现行代码 | 工程含义 |
|------|----------------|----------|----------|
| 分享成本归属 | **Owner-pays** | **Guest-pays**：`check_user_quota(&self.auth)` 用调用者；匿名不能提问（`service.rs` 硬拒绝 unauthenticated share chat） | 需翻转 quota/usage/cost-event 全链路；放开匿名提问（可选模式） |
| Owner 上下文 | 运行时用 Owner 账单与 key | `PublicShareChatContextSnapshot.owner_user_id` 等钩子存在，对应 handler **基本 dead** | 可复用字段，但接线是新工作 |
| 云端 BYOK | Workspace 级可配、可代发 | **不存在**：LLM/embed/rerank 为运营商 **env 级** key；DB 无 per-user/workspace 模型配置表 | 全新配置表 + **密钥托管与加密** |
| 桌面 BYOK | 可与云端统一语义 | 客户端本地明文 JSON（provider preset）；**非** workspace 级、无 rerank BYOK | 桌面可继续本地；**分享路径**必须云端可代发 |
| Token 配额墙 | 私有使用**无**平台 token 墙 | Free/Plus/Pro **5h/7d 滚动配额强制执行**；分享/私有云端问答都过墙 | 拆墙必须以钱包/BYOK 为前提，见 §1.1 / §10 |
| Publish 写入 | `DocumentIndexBatch` → Milvus | **写入契约真实存在**（pgvector/Milvus 均 `replace_document_index`，记录含 `Vec<f32>`） | 导入侧可复用 |
| Publish 导出 | 本地倒出向量+元数据 | **导出路径为零**：`RetrievalReadPort`/`ScoredChunk` **无 vector 字段**；无 export/snapshot API | **B3 大头在导出+打包**，不是写入 |
| 模型指纹 | 包内 model/dim/schema | `DocumentIndexBatch` **无** `embedding_model_id` / `vector_dim` / `schema_version`（全仓零） | 纯新增 schema |
| 身份映射 | 本地→云端 Owner | 未定义 | Publish 须规定 cloud `user_id`/`workspace_id` 映射 |
| 钱包/邀请 | wallet + referral | **全仓零** | B2 纯新建 |
| 支付/计量事件 | 复用 | Creem + Alipay 可用；`llm_usage_events` 完整 | 可接钱包扣费 |
| Member invite vs referral | 分离 | Member invite 已有；referral 无 | §6 区分正确 |
| 桌面激活 | 无激活 | **Keygen 强制** + trial；`desktop-standard`/`desktop-pro` 套餐仍在 | B6 删除；过渡见 §1.2 |
| 注册引导 | 引导页、可跳过配置 | Web：register → dashboard 直达；桌面 setup 可跳过 | 引导页新增 |
| 分享反扒 | §9 | 公开 share 路径：**无限流**、无 noindex/X-Robots-Tag、无 robots.txt、nginx 无限流 | **现状裸奔**；§9 全待实现 |

## Decision

### 1. 免费边界（目标态）

| 永远免费 | 说明 |
|----------|------|
| Web / Desktop 客户端 | **无激活、无买断**；Keygen/许可商业路径退役 |
| 私有 Workspace 使用 | 建库、上传、问答、检索、agent；**不设平台 token 配额墙**（费用来自用户 BYOK 或自有储值） |
| 功能面 | 不因档位阉割 RAG/工具等能力 |

纯离线私有使用可不登录云账号。**开启分享 / 充值 / 订阅名额 / 邀请奖励 / 云端问答（非纯本地）** 必须登录云账号（匿名分享访客除外，见 §4）。

#### 1.1 过渡期与顺序依赖（硬约束）

**禁止**在下列前提未满足时拆掉滚动 token 墙或宣称「云端私有使用无限」：

```text
必须先具备其一（按调用路径）：
  (A) 用户 BYOK 且请求走用户 key，或
  (B) 用户钱包余额可扣且请求走平台代购 key
否则平台 env key = 无限补贴。
```

| 阶段 | 用户可见行为 | 实现 |
|------|----------------|------|
| **T0 现状** | 仍有 Free/Plus/Pro token 墙；分享 guest-pays；匿名不可问；桌面要激活 | 不改叙事对外乱承诺 |
| **T1 钱包+代购可用** | 有余额可走代购；仍可暂留 soft 安全 cap 防刷 | B2 |
| **T2 云端 BYOK 可用** | 配 key 后私有/分享可打用户 key | 与 B2 并行或紧随 |
| **T3 Owner-pays 分享** | 分享问答记 Owner；可选匿名 | 见 §4.1（工作量 ≥ 原 B1 提示文案） |
| **T4 拆主权益 token 墙** | 私有使用不再以 plan token 限额为主商品 | **仅当 T1+T2 覆盖主路径**；原滚动策略可改为「无 BYOK 且无余额时的保护性 hard stop」而非套餐权益 |
| **T5 去激活** | 桌面免费无 Keygen | B6；过渡可「登录云账号即可用完整客户端」 |

对外文案：**目标态**写「使用免费、分享名额收费」；上线前 changelog 标明「迁移期仍可能有用量保护上限，以设置页为准」。

#### 1.2 桌面许可

- 现行代码仍有 Keygen / desktop 套餐痕迹；**无已购用户需兑付**，实现时直接删除激活与卖许可路径即可，不做兑换表。

### 2. 主商品：可分享 Workspace 名额（订阅）

| 档位 | 可分享 Workspace 上限 | 定位 |
|------|----------------------|------|
| **Free** | **3** | 试用经营少量对外知识服务 |
| **Plus** | **10** | 多产品线 / 多客户 |
| **Pro** | **100** | 重度运营商 |

#### 2.1 计费单位（统一用语）

**唯一计费事件：`share_enabled = true` 的 Workspace 数**（「可被外人按分享策略访问」的运营态）。

| 用户类型 | 进入 `share_enabled` 的动作 | 是否占用名额 |
|----------|------------------------------|--------------|
| **纯云端**建库 | 打开分享（链接/邀请策略生效）；数据已在云上，**无** Publish | **占用 1** |
| **本地**建库 | **Publish 成功且**打开分享；仅本地、未上云、未开分享 | **不占用** |
| 本地已 Publish 但关闭分享 | 云副本可保留，`share_enabled=false` | **不占用**（或宽限期后 GC，实现定） |
| 本地 Publish 更新同步 | 不新增名额 | 已占用则仍 1 |

「发布 / Publish」= **数据面动作**（本地→云副本）。  
「开启分享」= **计费与访问策略动作**。  
云端用户可只做后者；本地用户分享对外服务 **隐含** 前者（无 ready 云副本则不能 `share_enabled`）。

- 私有 Workspace：**不按档位硬顶**。
- 超限：禁止 **将新的 Workspace 设为 `share_enabled`**；已有可只读维持至压回上限或升级。
- **月付 + 年付**；年付默认约 **10 个月价/年**（标价实现前再定）。
- 支付：Creem / 支付宝；订阅与储值 **分轨 product**。

降级：到期 → Free；若已 `share_enabled` 数 > 3 → 禁止新增开启，提示升级或关闭多余分享。

#### 2.2 云端私有库的「防撑爆」上限（不是卖点，是护栏）

「使用免费」= 不按 token 套餐收费，**不等于**无限占库。

**存储形态（产品事实）**：上传原文件仅用于解析；**解析成功后删除原文件，长期只保留 Markdown（及索引侧 chunk / 向量 / 图谱等）**。因此同样「文档量」下，持久化体积远小于「整库留 PDF/Office」，**更能装**——护栏应按 **留存数据** 设计，不要按「原文件 GB」当用户心智主指标。

仍有成本的是：md 正文、chunk 行、**向量维度×条数**（往往比 md 更重）、图谱与元数据。护栏：

- **软顶**：提醒「快满了」；
- **硬顶**：暂停**新上传/新解析**（已有可继续读、问，除非另有规则）。

默认量级（可配置；**非**套餐卖点）。主闸用 **chunk 数 + 向量存储估算**；md 体积作辅闸（因不留原件，同等文档数下 md 上限可订得比「留原件」产品宽）：

| 资源（私有、按用户合计） | Free 提醒 | Free 停止新上传 | Plus 约 | Pro 约 |
|--------------------------|-----------|-----------------|---------|--------|
| 留存 md + 元数据体积 | 8 GB | 15 GB | 80 GB | 300 GB |
| 正文 chunk 条数 | 约 30 万 | 约 80 万 | 约 300 万 | 约 1500 万 |
| 向量存储估算（或等价维度×条） | 与 chunk 闸对齐 | 同左 | 同左 | 同左 |
| 同时解析任务数 | 2 | 3 | 更高 | 更高 |

说明：

- 用户体感「很能装」成立：一本 PDF 解析后往往只剩远小于原件的 md + 切块。
- 真正先顶满的经常是 **向量条数**，不是「原文件盘」。
- 人满引导：删文档、大库改**本地桌面**；**本期不做加钱扩容包**。
- 护栏管占库，**不是** token 月套餐墙。

### 3. 辅商品：模型代购储值

#### 3.1 定价语义（确认本意）

| 说法 | 含义 | 本 ADR |
|------|------|--------|
| 「加 50% 利润 / 加价 50%」 | **markup**：`list = official × 1.5` | **采纳** |
| 「50% 毛利率」 | **margin**：`list = official / 0.5 = ×2` | **不采用** |

- 对外/对内统一：**官方价 × 1.5**；毛利率约 **33%**（`1 − 1/1.5`）。
- 勿再写「50% 利润」以免与 margin 混淆。

| 项 | 规则 |
|----|------|
| 目录（白名单） | LLM / 向量以**上线时官方目录核名**为准；意向：DeepSeek flash 档、Qwen flash 档、SiliconFlow 向量；实现前写入配置表，**勿写死易变营销名** |
| BYOK | **目标**：Workspace（或账户默认 + WS 覆盖）级配置；**不扣**平台钱包 |
| 计量 | 代购：平台 key + 产品 `wallet` 先扣后用；`llm_usage_events` 入账 |
| 注册赠送 | 新用户 **一次性 ¥20** 赠送金（不可提现，仅代购白名单） |

上传/索引文案：索引与三元组抽取 **token 显著高于问答**；赠送有限，大库请 BYOK。

#### 3.2 云端 BYOK 与密钥托管（新增安全命题）

**现状**：云端无用户 key；桌面 key 仅本机。

**分享 Owner-pays + BYOK** 要求：云 API 进程能代发请求 → 平台必须在服务端持有 **可解密使用的 Owner 密钥材料**（或短期委托令牌）。这是 §3/§4 落地的隐藏大头。

最低要求（实现规格另文，本 ADR 锁定方向）：

1. **静态加密**：信封加密（KMS/主密钥 + per-secret DEK）；密 不以明文落库/日志。
2. **范围**：密钥绑定 `owner_user_id` + 可选 `workspace_id`；分享请求仅允许使用该 Workspace 授权的 secret。
3. **展示**：UI 只显示末四位 / 指纹；不可回读全文。
4. **轮换与吊销**：用户可轮换；吊销后分享立即停用该 key 路径。
5. **责任文案**：用户协议写明「密钥由平台代为调用第三方模型；泄漏面包括我方托管风险」；鼓励高敏用户仅用储值代购或仅本地。
6. **最小权限**：服务端仅在请求路径解密；不进分析库、不进 support 工具默认视图。
7. **审计**：key 使用记 `workspace_id` / `share_id` / 调用原因，不记 key。

桌面本地 JSON BYOK **可保留**供纯本地；**一旦 Publish + 分享且选 BYOK**，须走云端托管配置（可引导「上传 key 到云端仅用于该分享」），不得假设桌面明文 key 自动可用。

### 4. 分享成本归属与访客模式

#### 4.1 目标

- 访客使用 Shared Workspace 的 LLM/embed/rerank：**一律 Owner-pays**（Owner BYOK 或 Owner 储值）。
- 开启分享 / 复制链接时 **强制确认** 文案。
- 访客模式 Owner 可选：**匿名** | **须注册**。
- 默认：Owner 预算熔断 + 日提问上限；匿名更严。

#### 4.2 现状与翻转范围（非「加一句提示」）

| 现状 | 目标 |
|------|------|
| quota/usage 记 **caller**（访客） | 记 **Owner**（`owner_user_id` from share context） |
| 未登录 share chat **硬拒绝** | 匿名模式允许提问；注册模式保持需登录 |
| 平台 env key + guest 配额 | Owner 钱包或 Owner 托管 key |
| `owner_user_id` 快照未贯通执行 | 全链路：鉴权上下文、quota、usage_events、wallet debit、LLM client 选 key |

**工作量**：billing/chat 执行上下文翻转 + 匿名路径 + 密钥/钱包接线；应单列为 **B2.5 / B3 并行关键路径**，**不得**塞进「B1 文案提示」。

#### 4.3 访客模式与 Denial-of-Wallet

公开问答在 Owner-pays 下 = **以 Owner 为付款人的 LLM 代理面**（LLMjacking / sponge attack 风险）。除熔断外：

- 优先 **按预算/token 单位** 限流，而非仅 QPS（见 §9）。
- **匿名访客：只答库里的（已拍板）**：回答须 grounded 于该分享知识库；闲聊/通用生成默认拒绝或极短降级提示，避免被当免费 ChatGPT 刷 Owner 额度。须注册访客可产品开关放宽，**默认仍建议只答库里**。

### 5. 本地 → 云端 Publish / 更新同步

#### 5.1 为何不能「pgvector 直接移库到：Milvus」

- 存储格式 / 索引 / collection schema 不兼容，无官方物理迁库。
- **导入**：统一契约 `DocumentIndexBatch` + 双方 `replace_document_index`（**已存在，可复用**）。
- **导出**：**不存在**，须新建（见 §5.6）。

#### 5.2 直传向量的条件

同一 `embedding_model_id`、同一 `vector_dim`、同一归一化约定时，`Vec<f32>` 可随包上传并写入，**无需重嵌**。  
校验失败 → 拒绝发布或「云侧重嵌」分支（明示消耗）。

指纹字段（`embedding_model_id` / `vector_dim` / `schema_version`）在 **manifest 与/或扩展 batch** 上均为 **新增**，当前 `DocumentIndexBatch` **没有**这些字段。

#### 5.3 必须同步的数据

| 层 | 内容 | 云端落点 |
|----|------|----------|
| 检索面 | text/multimodal chunks + vectors；entities/relations（三元组）+ vectors；graph passages + vectors | Milvus via `DocumentIndexBatch` |
| 元数据面 | documents、**summary**、**TOC**、库/文档 **profile**、parse_run/doc_version | 云 PG |
| 可选 | 原文/assets | 对象存储 |

**禁止** 本机 `pg_dump` 灌云生产库。

#### 5.4 编辑与更新（已拍板）

- Publish 后 **允许本地继续改库**。
- **更新同步** + 展示：
  - `last_published_at`
  - `last_local_change_at`（或 content revision）
  - `publish_status`：`never` | `publishing` | `ready` | `dirty` | `failed`
  - `last_local_change_at > last_published_at` → **dirty**
- 首版可整库覆盖；增量：`doc_id + content_hash/doc_version`。
- **分阶段进度条**：打包 → 校验 → 分片上传 → 写 PG → 写向量 → 自检。
- 仅云端 `ready` 且 `share_enabled` 时对外服务。

#### 5.5 建议包形态

```text
WorkspacePublishBundle v1
├── manifest.json   # cloud mapping, model fingerprint, hashes, counts, schema_version
├── docs/{doc_id}/meta.json
├── docs/{doc_id}/index.batch.jsonl  # zstd optional
└── docs/{doc_id}/assets/...
```

#### 5.6 导出 API 与身份映射（明确为新增）

**新建**（B3 核心，勿低估）：

1. `RetrievalExportPort`（名可议）：按 `workspace_id`/`doc_id` **读回**含 `vector: Vec<f32>` 的全量索引记录 + 元数据（summary/TOC/profile）。
2. 桌面侧打包器 + 分片上传客户端 + 进度事件。
3. 云端 `POST .../publish` 接收 → 校验指纹 → 映射身份 → `replace_document_index` + PG upsert。

**身份映射：**

- Publish 必须在 **已登录云账号** 下进行。
- 云端创建/绑定 `cloud_workspace_id`（可与本地 UUID 相同若全局唯一策略允许，否则映射表 `local_workspace_id → cloud_workspace_id`）。
- 写入 batch 的 `owner_user_id` = **云端 Owner**，不是未映射的本地临时 id。
- 文档 id 策略：保持 UUID 稳定以便增量；冲突时以 Owner+cloud_workspace 为命名空间。

### 6. 邀请码（Referral）

与 Workspace **协作邀请**（member invite）分离。

| 项 | 规则 |
|----|------|
| 码 | 每用户稳定邀请码 + 分享链接 |
| 填写 | 注册时（或极短窗口内绑定一次） |
| 发放时机 | **注册验证通过即发** |
| 金额 | 邀请人 **+¥5**、被邀请人 **+¥5**（赠送金） |
| 与注册礼 | **叠加**：被邀请人 = **¥20 + ¥5** |
| 次数 | 基础 **5**；`+ floor(lifetime_paid_topup_cny / 50)` |
| 自邀 | 设备/支付工具/图检测；rejected 不计奖 |
| 与订阅 | **只加储值，不加** 可分享名额 |

`referrals(status: pending→rewarded|rejected)`；`wallet_ledger.kind = referral_bonus | signup_grant`。

### 7. 邀请与赠送经济模型（测算）

#### 7.1 单位成本（平台）

```text
COGS_api ≈ gift_spent / 1.5
```

| 场景 | 面值流出 | 估 COGS（花光） |
|------|----------|-----------------|
| 自然注册 | ¥20 | ≈ ¥13.3 |
| 邀请注册（叠加） | ¥30 | ≈ ¥20.0 |
| 增量获客成本 | **+¥10 面值** | ≈ **+¥6.7** |

#### 7.2 邀请次数

```text
referral_quota = 5 + floor(lifetime_paid_topup_cny / 50)
referral_remaining = referral_quota - rewarded_invite_count
```

`lifetime_paid_topup_cny` 仅真实充值。0 充值刷量：最多 5 次 ≈ 增量面值 ¥50 + 5×¥20 注册礼。  
每 +1 次需 ¥50 现金，对单次增量 ¥10 面值约 **5×** 覆盖。

#### 7.3 健康判据

- `gift_redemption_rate`、`share_subscribe_conversion`（第 4 个分享或升 Plus）
- 邀请人 30 日内 topup/subscribe 比例
- CAC 过高：调 `referral_base_quota` 或 `referral_bonus_each_cny`；**不**加回 token 套餐墙

#### 7.4 参数表

| 参数 | 默认 |
|------|------|
| `signup_grant_cny` | 20 |
| `referral_bonus_each_cny` | 5 |
| `referral_base_quota` | 5 |
| `referral_topup_step_cny` | 50 |
| `referral_stack_signup` | true |
| `list_price_multiplier` | 1.5 |
| `annual_price_months` | 10 |

### 8. 网关、配置面与 LiteLLM 边界

#### 8.1 与 2026-08-01 ProviderPool 决定的关系（消除冲突）

| 文档 | 范围 | 决定 |
|------|------|------|
| `2026-08-01-llm-providerpool-acceptance.md` | **平台内部**多 key / 跨 provider 故障切换 | **保持纯 Rust `ProviderPool`**，**不**为内部路由引入 LiteLLM/AI-SDK |
| 本 ADR §8 | **用户代购计费面**（虚拟余额、按用户/WS 预算、白名单标价） | **优先产品内钱包 + 现有 Rust LLM 客户端 + usage 事件扣费**；**不强制**引入 LiteLLM |

**裁定（2026-08-06）：**

- **默认实现：不引入 LiteLLM。** 代购 = 平台 env/pool key + `wallet` 原子扣费 + 白名单模型价目表（官方价 × 1.5）+ 可选 per-user budget 行。
- LiteLLM 仅作为 **可选后续评估**（若自建计量/虚拟 key 成本过高再开 ADR 修订）；在评估前 **不得**与「内部纯 Rust」决定混写为双现行。
- 内部 failover 继续只走 `ProviderPool`。

#### 8.2 配置面

- 账户级：订阅名额、钱包、邀请码、（可选）默认模型偏好。
- Workspace 级：BYOK 密钥引用、默认模型、分享模式、预算、publish 状态与上次同步时间。
- 注册引导页：分享命题、模型来源、索引耗 token；**不强制配完才能进产品**。

### 9. 反扒与成本防护（分享面）

**现状：公开分享 API 基本裸奔**（无限流、无 noindex、无 robots）。以下均为待实现目标。

#### 9.1 索引与链接泄漏

| 措施 | 说明 |
|------|------|
| **优先 `noindex` / `X-Robots-Tag`** | 允许爬虫抓到 HTML 才能看到 noindex |
| **不要**对同一分享 URL 同时 `robots.txt Disallow` + 依赖 noindex | Disallow 后 Google 可能抓不到 noindex，反而靠外链收录裸 URL（[Google: block indexing](https://developers.google.com/search/docs/crawling-indexing/block-indexing)） |
| `Referrer-Policy` | 收紧，降低分享 token 经 Referer 泄漏到第三方 |
| 链接可吊销 / 可过期 | 复用已有 revoke；默认建议可设 TTL |
| unguessable token | 必要但不充分（ChatGPT/Grok 分享被收录先例说明泄漏面在转发与插件） |

#### 9.2 限流维度（应用层为主）

| 层 | 要求 |
|----|------|
| **成本向** | per-share / per-Owner **token 或预算单位** 配额；输入长度上限；输出/agent 步数上限（防 sponge / DoW） |
| **请求向** | per-share + per-IP/session QPS 与日提问次数 |
| **边缘** | Cloudflare 等可选；**免费档限流规则极少且偏 IP** → **per-share 必须在应用层**实现，不依赖边缘 |
| **Challenge** | 匿名首次交互默认 **Turnstile（或同类免费 challenge）**，写入默认项而非「可选 WAF」 |
| **Provider 侧** | 平台 pool 的 spend alert / max budget 作第二道闸 |
| **缓存** | 公开 Q&A 对相同问题做精确/语义缓存，重复抓取近零边际成本 |
| **能力收窄** | 匿名默认「只答库里的」，少当通用免费 ChatGPT（§4.3） |
| **无全库 dump** | API 不提供未授权 list-all-chunks |

### 10. 实现分期（含依赖；修正低估）

| 波次 | 内容 | 依赖 | 验证 |
|------|------|------|------|
| **B0** | 本文（含 08-06 修订）；旧 ADR SUPERSEDED；对外迁移文案 | — | 文档 |
| **B1** | `max_shared_workspaces` / `share_enabled` 计数 3/10/100；分享费用确认 UI；**不**假装 Owner-pays 已完成 | — | 单测+手动 |
| **B2** | 钱包：¥20、充值、代购扣费（Rust 价目表）；邀请码+配额公式 | 支付现网 | 对账 |
| **B2.5** | 云端 BYOK 密钥托管（加密表+API+UI）；Workspace 绑定 | B2 可并行启动设计 | 安全评审+单测 |
| **B3a** | **Owner-pays 翻转** + 匿名可选 + share 预算/token 限流 + 提示 | B2；BYOK 路径需 B2.5 | 分享问答记 Owner |
| **B3b** | Publish：**导出 port**、bundle、上传进度、云导入、指纹字段、身份映射、`last_published_at` | 登录云 | E2E publish |
| **B4** | 更新同步、增量 hash、dirty 态 | B3b | 改库再同步 |
| **B5** | §9 反扒全项（noindex、Referrer-Policy、Turnstile、缓存等）；价目模型核名 | B3a | 压测+抽检 |
| **B6** | 拆 token **主权益**墙（保留无余额无 BYOK 保护停）；去 Keygen/桌面卖许可；清理 desktop-* 套餐 | **T1+T2+T3**（§1.1） | L1 |

原「B1=名额+提示、B3=有写入即可」**低估**：成本归属翻转、密钥托管、导出 API 均为独立大块，已拆入 B2.5 / B3a / B3b。

### 11. Consequences

**正面**

- 收费点与壁垒一致；商品定义翻转干净。
- 目标与代码差距写清后，排期可信。
- 导入契约可复用；导出与指纹诚实标为新建。
- 与 ProviderPool「内部纯 Rust」决定共存：代购默认不引入 LiteLLM。

**负面 / 风险**

- Owner-pays + 托管 key = 安全与合规负担。
- 过渡期若过早拆墙 → 平台无限补贴。
- 注册即发邀请需配额与验证压刷量。
- Publish 导出与大包上传体验工程量大。
- 公开分享 DoW：必须成本向限流，不能只靠 QPS。

**废弃（目标完成后）**

- 桌面软件许可收入与强制激活。
- 以 Free/Plus/Pro **token 滚动限额** 作为主权益与主升级理由。

## References

- 写入契约：`avrag-rs/crates/retrieval-data-plane`（`DocumentIndexBatch`）— **无导出、无模型指纹字段**
- 本地/云检索：`storage-pgvector` / `storage-milvus`
- 分享域：`avrag-rs/crates/share`；chat 配额：`app-chat`（现行 guest auth）
- 内部 LLM 路由（纯 Rust）：`avrag-rs/docs/engineering/2026-08-01-llm-providerpool-acceptance.md`
- 桌面便携数据面：`docs/desktop/2026-08-04-portable-runtime-design.md`
- 反扒参考：Google block-indexing；OWASP LLM10 Unbounded Consumption；DoW cost-aware rate limiting 实践
