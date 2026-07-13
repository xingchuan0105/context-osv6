# DeepSeek 用量计费 — 开发计划

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-13 |
| 状态 | **In progress / mostly landed** — Wave 1–4 代码已合本地 trunk；Wave 5 硬化进行中 |
| 约束 | Solo 本地 trunk；L1 / 针对性 `cargo test -p …` / `pnpm test`；**不**默认 push / PR / 扩 CI |
| 产品铁律 | B2C `user_id`；`workspace` 唯一真相；支付仅 **Creem + Alipay**；**价格不变** |
| 权威设计 | [DEEPSEEK_STYLE_USAGE_BILLING_DESIGN_2026-07-13.md](./DEEPSEEK_STYLE_USAGE_BILLING_DESIGN_2026-07-13.md) **v2 frozen** |
| 产品档位源 | [2026-06-07 pricing tiers](../superpowers/specs/2026-06-07-pricing-tiers-revamp-design.md) §2（约 tokens + 价格） |
| 兄弟计划 | [PRODUCT_UI_CHROME_AND_BILLING_DEV_PLAN](./PRODUCT_UI_CHROME_AND_BILLING_DEV_PLAN_2026-07-13.md)（Chrome Wave 1–5 **可并行**，不阻塞本计划） |
| 工程纪律 | [SOLO_DISCIPLINE.md](./SOLO_DISCIPLINE.md)、`AGENTS.md` §7–§8 |

---

## 0. 一句话目标

把 **三桶折算（miss / cache hit / output）+ 分档乘数 M + 限额倒推 + 用户可见「约 tokens / M」** 接到出口计量主路径，纠正「token 数误写进 units 列」的历史债；**订阅标价不动**。

---

## 1. 已冻结决策（实现不得偏离）

| ID | 决议 |
|----|------|
| D1 | 相对权重 hit:miss:out = **0.02 : 1 : 2**（Flash 默认 / fallback） |
| D2 | 分档 M：**free 2.0 / plus 1.5 / pro 1.3**，**对用户透明** |
| D3 | 部署日起新公式；**不回算**历史 `usage_units` |
| D4 | 约 tokens 档位保持 06-07；`limit_units = ceil(T/1000 × M)`；**价格不变** |
| D5 | Worker `billable=false` 不进 5h/7d |
| D6 | 用户语言主轴：**约 tokens** + 明示 M；进度比仍用 units |
| D7 | Free M=2.0 为设计默认（可改配置，须同步重算限额） |

### 1.1 目标限额表（入库）

| plan | M | 5h 约 tokens | **5h units** | 7d 约 tokens | **7d units** |
|------|---|--------------|--------------|--------------|--------------|
| free | 2.0 | 100,000 | **200** | 400,000 | **800** |
| plus | 1.5 | 600,000 | **900** | 4,000,000 | **6,000** |
| pro | 1.3 | 2,500,000 | **3,250** | 15,000,000 | **19,500** |

### 1.2 扣费公式（入库时）

```text
raw = miss/1k·r_miss + cached/1k·r_cache + out/1k·r_out
usage_units = 0 if no tokens else max(1, ceil(raw * M_plan))
```

### 1.3 展示换算

```text
tokens_approx = units / M * 1000
percentage    = used_units / limit_units   // 与约 tokens 比一致
```

---

## 2. 基线（已完成 — 勿回退）

| ID | 项 | 状态 |
|----|----|------|
| B-U-1 | `llm_usage_events` RLS：`set_current_user` 读写 | Done |
| B-U-2 | 账单页去掉 token/doc「未设置」双轨 | Done |
| B-U-3 | 用量副文案（将被 Wave F 改写） | Done |
| B-PAY-* | Stripe 硬切 | Done |
| Design v2 | 三桶 + M + 倒推 | **Frozen** |

**禁止回归：** Stripe；账单主 UI 双轨；把 token 整数再写回 `rolling_*_limit_units`；隐藏 M。

---

## 3. 波次编排（DAG）

```text
Wave 0  文档互链 + 迁移编号冻结 ──────────────┐
         │                                      │
Wave 1  Schema migration（cached / M / limits）──┤  可先合入库，代码稍后读
         │                                      │
Wave 2  核心公式 + 出口贯通 cached + insert×M ──┤  后端可用
         │                                      │
Wave 3  窗口 API 扩展（M + 约 tokens 字段）──────┤  契约就绪
         │                                      │
Wave 4  前端透明展示（约 tokens + M）────────────┤  用户可见
         │                                      │
Wave 5  测试硬化 + 定价/帮助文案 + design errata ┤
         │                                      │
Wave 6  （可选）usage_events 产品路径收敛 ───────┘  默认 skip
```

| 关系 | 说明 |
|------|------|
| 0 → 1 → 2 → 3 → 4 → 5 | 主链顺序 |
| Wave 1 ∥ Chrome Wave 1–2 | **可并行**；互不阻塞 |
| Wave 2 后本地 API 必须 **restart** + 跑 migration | 验收前置 |
| Wave 6 | 仅明确要清双表时启动 |

**建议本地提交粒度（solo）：**

1. `billing: migration deepseek units + M`（Wave 1）  
2. `billing: three-bucket units × plan margin`（Wave 2）  
3. `billing: usage window approx tokens API`（Wave 3）  
4. `frontend: usage transparency approx tokens + M`（Wave 4）  
5. `docs+tests: billing wave harden`（Wave 5）

---

## 4. Wave 明细

### Wave 0 — 文档与开工门禁（0.25d）

| 步骤 | 动作 | 验收 |
|------|------|------|
| 0.1 | 本计划 + 设计 v2 互链；Chrome 计划 B-U-4 指向本计划 | 三份 doc 状态一致 |
| 0.2 | 确认下一 migration 序号（当前最新 **0058** → 建议 **0059**） | 无冲突 |
| 0.3 | 开工前读设计 §5 / §10 决策表 | 实现者对齐数字 |

**状态：** 文档落地即 **Done**（本提交）。

---

### Wave 1 — Schema migration（0.5d）

**目标：** 库表具备三桶与分档 M 的列与目标限额；历史事件行不回算。

| 步骤 | 文件（预期） | 说明 |
|------|----------------|------|
| 1.1 | `avrag-rs/migrations/0059_deepseek_usage_units.up.sql` | 见下 SQL 契约 |
| 1.2 | 同名 `.down.sql` | 可逆：drop 列 / 恢复旧 limits（旧 limits 仅 rollback 用） |
| 1.3 | 本地 `sqlx migrate` / 现有 migrate 路径跑通 | 无 error |

**SQL 契约（逻辑，非最终字符级）：**

```sql
-- llm_usage_events
ALTER TABLE llm_usage_events
  ADD COLUMN IF NOT EXISTS cached_tokens BIGINT NOT NULL DEFAULT 0;

-- llm_model_weights
ALTER TABLE llm_model_weights
  ADD COLUMN IF NOT EXISTS cache_hit_unit_rate DOUBLE PRECISION NOT NULL DEFAULT 0.02;

-- usage_limit_plan_policies
ALTER TABLE usage_limit_plan_policies
  ADD COLUMN IF NOT EXISTS margin_multiplier DOUBLE PRECISION NOT NULL DEFAULT 2.0;

UPDATE usage_limit_plan_policies SET
  margin_multiplier = 2.0,
  rolling_5h_limit_units = 200,
  rolling_7d_limit_units = 800
WHERE plan_id = 'free';

UPDATE usage_limit_plan_policies SET
  margin_multiplier = 1.5,
  rolling_5h_limit_units = 900,
  rolling_7d_limit_units = 6000
WHERE plan_id = 'plus';

UPDATE usage_limit_plan_policies SET
  margin_multiplier = 1.3,
  rolling_5h_limit_units = 3250,
  rolling_7d_limit_units = 19500
WHERE plan_id = 'pro';

-- 可选：upsert DeepSeek 权重行（provider/model 与 .env 主用模型对齐）
```

**验收：**

- `SELECT plan_id, margin_multiplier, rolling_5h_limit_units, rolling_7d_limit_units FROM usage_limit_plan_policies` 与 §1.1 一致。  
- 旧 `llm_usage_events` 行 `cached_tokens=0`，`usage_units` 原值不变。

**验证：** migrate up/down 各一次（或至少 up）。

---

### Wave 2 — 核心计量（后端，1–1.5d）**【关键路径】**

**目标：** 出口 → 账本完整三桶 + plan M；enforcement 读新 limits。

| 步骤 | 文件（预期） | 说明 |
|------|----------------|------|
| 2.1 | `crates/llm/src/usage_observer.rs` | `ChatUsageRecord.cached_tokens: u32` |
| 2.2 | `crates/llm/src/client/mod.rs` | `record_completion_success` 填 `cached_token_count()` |
| 2.3 | `crates/app-core/src/billing_store.rs` | `UsageLimitUsageRecord` + `cached_tokens` |
| 2.4 | `crates/app-core/src/billing_usage_units.rs` | 三 rate 公式；thin wrapper 旧两参 `cached=0` |
| 2.5 | `UsageLimitPlanPolicyRow` + ports | `margin_multiplier: f64` |
| 2.6 | `pg_usage_limit_store.rs` | `load_model_rates` → (miss, cache, out)；`load_plan_policy` 读 M；insert 时 `get_user_plan` → M → compute |
| 2.7 | `app-billing/usage_observer_impl.rs` | 传 `cached_tokens`；stub `load_model_rates` 三元组 |
| 2.8 | 调用方：`cost_events` / worker `profile` 等 `compute_usage_units` | 编译通过；worker 路径仍 non-billable |
| 2.9 | 单测 | 见下 |

**insert 伪代码：**

```text
(miss_r, cache_r, out_r) = load_model_rates(provider, model)
plan = get_user_plan(user_id)
M = load_plan_policy(plan).margin_multiplier  // fallback free 2.0
units = compute_usage_units_three_bucket(
  prompt, completion, cached, miss_r, cache_r, out_r, M)
INSERT ... cached_tokens, usage_units, ...
```

**单测矩阵（最低）：**

| 用例 | 期望 |
|------|------|
| prompt=1000, cached=0, out=0, M=1 | units=1 |
| 同上 M=1.5 | units=2（ceil 1.5）或按实现写死期望 |
| prompt=1000, cached=1000, out=0, rates 默认, M=1 | units=1（ceil 0.02）→ max(1,…) = 1 |
| prompt=20000, cached=16000, out=2000, M=1.5 | raw=8.32 → ceil(12.48)=13 |
| 同 raw Free M=2 vs Pro M=1.3 | Free > Pro units |
| worker billable=false | 行存在但不计入 sum billable |

**验收：**

- 同请求 Prometheus cached 与行 `cached_tokens` 一致（抽样/单测 mock）。  
- 二次相同上下文 cache hit 时 units **低于** 全 miss。  
- Free 用户新对话后 5h used 可 >0 且 limit=200（migration 后）。

**验证：**

```bash
cd avrag-rs
cargo test -p app-core --lib
cargo test -p app-billing --lib
cargo test -p llm --lib
cargo test -p app-bootstrap --lib
# 若有 usage_limit / billing 集成：
cargo test -p avrag-billing --lib   # crate 名以 Cargo.toml 为准：billing
```

---

### Wave 3 — 窗口 API 契约（后端，0.5d）

**目标：** 前端无需猜公式即可展示约 tokens 与 M。

| 步骤 | 文件 | 说明 |
|------|------|------|
| 3.1 | `app-core/billing_domain.rs` `UsageWindowResponse` | 增字段（serde，**向后兼容**：新字段必填服务端始终发） |
| 3.2 | `billing_sql/core_usage.rs` `load_usage_window` | 填 M；约 tokens |
| 3.3 | `usage_limit` 若另有 window DTO | 对齐或映射 |
| 3.4 | billing 集成测试 seed 新 limits | window 断言 200/800 等 |

**建议响应形状（权威）：**

```json
{
  "plan_id": "plus",
  "margin_multiplier": 1.5,
  "rolling_5h": {
    "used": 13,
    "limit": 900,
    "used_tokens_approx": 8667,
    "limit_tokens_approx": 600000,
    "percentage": 1,
    "reset_at": "..."
  },
  "rolling_7d": { "...": "..." },
  "soft_limit_hit": { "rolling_5h": false, "rolling_7d": false },
  "hard_limit_hit": { "rolling_5h": false, "rolling_7d": false }
}
```

**换算规则（服务端单一真相）：**

```text
used_tokens_approx  = round(used  / M * 1000)
limit_tokens_approx = round(limit / M * 1000)   // free 5h → 100000
```

`UsageWindowBucket` 扩字段时：前端旧代码忽略未知字段亦可，但本 monorepo **同步改 TS 类型**（Wave 4）。

**验收：** `GET /api/billing/usage/window`（或现网路径）含 `margin_multiplier`；free limit_tokens_approx_5h=100000。

**验证：**

```bash
cargo test -p billing --test test_usage_window_endpoint
# 或 crates/billing/tests/*
```

---

### Wave 4 — 前端透明展示（0.75–1d）

**目标：** 用户只看到 **约 tokens** + **M**；进度条仍按 percentage。

| 步骤 | 文件（预期） | 说明 |
|------|----------------|------|
| 4.1 | `frontend_next/lib/billing/api.ts` | 类型对齐 Wave 3 |
| 4.2 | `lib/billing/format.ts` | 若需 `formatApproxTokens`；复用千分位 |
| 4.3 | `components/billing/UsageMeter.tsx` | 主文案 `约 {used} / {limit} tokens`；副文案 M |
| 4.4 | `settings-billing-panel` / `usage-dashboard-client` / `paywall-page-client` | 同源 |
| 4.5 | `workspace-surface` toast / warning | used/limit 用约 tokens（若展示数字） |
| 4.6 | `app/(marketing)/pricing/*` | 三档：**约 T tokens** + **M=…**；价格文案不动 |
| 4.7 | i18n：`usage.ts` / `pricing.ts` / `settings.ts` / `paywall.ts` | zh/en |
| 4.8 | mocks：`workspace-surface.setup.ts` 等 | 新字段 + 新 limit 量级（900 等）避免假 100000 units |
| 4.9 | unit tests | UsageMeter / api parse / format |

**文案契约（zh 示例）：**

- 主：`约 {used_tokens_approx} / {limit_tokens_approx} tokens`  
- 副：`方案乘数 M={margin_multiplier}；缓存命中更省`  
- 定价：`5 小时约 600,000 tokens · 乘数 1.5`

**验收：**

- 账单 / 用量 / 定价 / paywall **无**「未设置」双轨。  
- Free mock：limit 约 100,000 tokens 展示，不是 200 当 tokens。  
- 价格仍 ¥49/$9、¥129/$19。

**验证：**

```bash
cd frontend_next
pnpm exec vitest run tests/billing
# 相关 settings / workspace mock 测试
```

---

### Wave 5 — 硬化与文档 errata（0.5d）

| 步骤 | 动作 | 验收 |
|------|------|------|
| 5.1 | 设计 doc 状态 → **Implemented**（或波次勾选） | 与代码一致 |
| 5.2 | pricing revamp 文增加 errata：units≠token 原样；展示约 tokens | 交叉链接 |
| 5.3 | 导出路径若存在：CSV 增加 `cached_tokens`（有则做，无则记 residual） | 可选 P2 |
| 5.4 | 手工：restart API → migrate → 登录 free → 新对话 → 5h 进度 | used>0；limit 约 100k tokens |
| 5.5 | 同 prompt 连发：观察 cached 路径 units 更省（有 provider cache 时） | 日志/DB 抽查 |

**验证（wave 末建议）：**

```bash
# 后端针对性
cd avrag-rs && cargo test -p app-core -p app-billing -p billing --lib
# 前端
cd frontend_next && pnpm exec vitest run tests/billing
# 不强制 full L1 / Playwright，除非用户要求
```

---

### Wave 6 — 可选：双表收敛（默认 **Out of scope**）

| 步骤 | 说明 |
|------|------|
| 6.1 | 审计产品 handler 是否仍读 `usage_events` 做用户主进度 |
| 6.2 | 仅 Admin/内部保留；禁止新 UI 绑 `quota_limits`「未设置」 |
| 6.3 | **不**删表除非单独批准 |

---

## 5. 触达面清单（防漏）

### 5.1 后端（必碰）

| 区域 | 路径 |
|------|------|
| 公式 | `crates/app-core/src/billing_usage_units.rs` |
| 领域类型 | `crates/app-core/src/billing_domain.rs`、`billing_store.rs` |
| Observer 类型 | `crates/llm/src/usage_observer.rs`、`client/mod.rs` |
| PG 适配 | `crates/app-bootstrap/src/adapters/pg_usage_limit_store.rs` |
| 窗口 SQL | `.../billing_sql/core_usage.rs` |
| Observer 实现 | `crates/app-billing/src/usage_observer_impl.rs` |
| 限额服务 | `crates/billing/src/usage_limit/*` |
| HTTP | `crates/billing/src/handlers.rs`、`transport-http` billing 路由 |
| Migration | `migrations/0059_*.sql`（序号以仓库为准） |

### 5.2 后端（编译跟进）

| 区域 | 注意 |
|------|------|
| `app-billing/cost_events.rs` | `compute_usage_units` 签名 |
| `worker/.../profile.rs` | 同上；保持 non-billable 语义 |
| Stub stores in tests | `load_model_rates` 三返回值；policy 含 M |

### 5.3 前端（必碰）

| 区域 | 路径 |
|------|------|
| API 类型 | `lib/billing/api.ts` |
| Meter | `components/billing/UsageMeter.tsx` |
| Settings / Usage / Paywall | `settings/*`、`usage/*`、`upgrade/paywall/*` |
| Pricing | `app/(marketing)/pricing/*` |
| Workspace 压力提示 | `workspace-surface.tsx` |
| i18n | `lib/i18n/messages/{usage,pricing,settings,paywall}.ts` |
| 测试 mock | `tests/billing/*`、`tests/workspace/helpers/*` |

### 5.4 明确不碰

- Checkout / Creem / Alipay 价格配置（除非文案展示约 tokens）。  
- Stripe（已死）。  
- Product App 架构重构。  
- 把 ingestion worker 改为 billable。

---

## 6. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| 限额从 1e5 units 降到 200 | Free 用户立刻硬顶 | **预期行为**；文案说明约 100k tokens；部署说明写进 release note |
| 历史 used_units 仍按旧公式偏大 | 进度条虚高直至窗口滚出 | 不回算；5h 窗口最多 5h 自愈；7d 最坏 7d |
| insert 时 plan 查询失败 | M 错误 | fallback free M=2.0 + warn log |
| 前端只改文案未改 mock limit | 单测/ e2e 假绿 | Wave 4 强制改 setup 为新量级 |
| `ceil(raw*M)` 与「约 tokens」舍入差 1 | 展示 99999 vs 100000 | 服务端 `limit_tokens_approx` 用产品表常量或 round；单测固定 |
| 与 Chrome Wave 改同一 settings 文件 | 冲突 | 串行改 settings 或先合 Chrome Wave 1 |

---

## 7. 完成定义（整计划 Done）

- [x] Migration 文件已写（0059）；**本机/VPS 需跑 migrate** 后 policy 数字 = §1.1  
- [x] 新 LLM 调用写入 `cached_tokens`；units = ceil(raw×M)（代码路径）  
- [x] 同 raw：free > plus > pro 的 units（unit test）  
- [x] `usage/window` 返回 M 与约 tokens（core_usage + DTO）  
- [x] UI：约 tokens + M；定价价格未改  
- [x] Worker 路径 billable=false / M=1.0 for non-billable unitization  
- [ ] pricing revamp errata 文段（可选文档）  
- [x] 针对性测试绿（app-core/app-billing/llm/billing/bootstrap + frontend billing）  
- [ ] 手工 free 新对话进度可信（需 migrate + restart API）  

---

## 8. 与 Chrome 计划的分工

| 工作 | 归属 |
|------|------|
| AccountMenu / 顶栏创建 / 页脚 / Share 文案 | Chrome 计划 Wave 1–5 |
| 三桶 + M + 限额 + 约 tokens 展示 | **本计划** Wave 1–5 |
| 账单页「管理订阅」已 Done | 两计划均勿回退 |
| Stripe | 已 Done；两边禁止回归 |

可并行：**Chrome Wave 1** ∥ **本计划 Wave 1–2**。  
若只开一人：优先 **本计划 Wave 1–2**（计量正确）再 Chrome，或按产品偏好先 AccountMenu——**互不依赖**。

---

## 9. 开工检查清单（执行者）

```text
[ ] 读设计 v2 §3–§5、§10
[ ] 读本计划 §1 数字表
[ ] git status 干净或明确 WIP 边界
[ ] 确认 migration 序号
[ ] Wave 1 → 2 → 3 → 4 → 5 顺序，每波验证命令跑过再下一波
[ ] 不 push / 不扩 CI 除非用户要求
```

---

*Plan ready. 用户说「按计划开工」或指定 Wave N 后开始写代码。*
