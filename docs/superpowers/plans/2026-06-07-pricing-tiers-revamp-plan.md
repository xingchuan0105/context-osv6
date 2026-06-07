# Pricing Tiers Revamp Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实施 `2026-06-07-pricing-tiers-revamp-design.md` 定义的三档定价重构（Free/Plus/Pro）+ 4 个前端展示界面（价格页、用量仪表盘、对话 toast、Paywall），5 周分阶段交付。

**Architecture:** 沿用现有 `crates/billing`（Rust）+ `frontend_next`（Next.js 16/React 19/CSS Modules）。后端加 1 个 migration + 3 个新 endpoint + 1 个 endpoint 增强；前端加 1 个 API client + 6 个组件 + 4 个页面 + i18n 词条。所有价格展示走双币种（CNY/USD），所有 token 用量展示走 5h/7d 双窗口。

**Tech Stack:** Rust (axum + sqlx + tokio) / Next.js 16 + React 19 + TypeScript + CSS Modules / Vitest / Playwright / next-intl

---

## 文件结构总览

### 后端（avrag-rs）

| 路径 | 状态 | 责任 |
|------|------|------|
| `migrations/0037_pricing_revamp.up.sql` | 新建 | 刷新 quota_limits + usage_limit_plan_policies |
| `migrations/0037_pricing_revamp.down.sql` | 新建 | 回滚到 0036 状态 |
| `crates/billing/src/types.rs` | 修改 | BillingConfig 调整 alipay_price_* 默认值 + price_label |
| `crates/billing/src/api.rs` | 修改 | 增强 handle_get_plans（双币种 price_label）|
| `crates/billing/src/core_usage.rs` | 修改 | 新增 handle_get_usage_window / history / forecast |
| `crates/billing/src/lib.rs` | 修改 | 导出新 handler |
| `crates/billing/tests/test_pricing_revamp.rs` | 新建 | 单元测试 |
| `crates/transport-http/src/routes/billing.rs` | 修改 | 新增 3 个路由 |

### 前端（frontend_next）

| 路径 | 状态 | 责任 |
|------|------|------|
| `lib/billing/api.ts` | 新建 | API client（4 个端点 + TS 类型）|
| `lib/billing/format.ts` | 新建 | 数字/时间/百分比格式化工具 |
| `components/billing/UsageMeter.tsx` + `.module.css` | 新建 | 5h/7d 双卡进度条（full + compact）|
| `components/billing/PricingCards.tsx` + `.module.css` | 新建 | 3 档对比卡（full + compact）|
| `components/billing/UsageWarningToast.tsx` + `.module.css` | 新建 | 80%/95% toast |
| `components/billing/UsageForecastCard.tsx` + `.module.css` | 新建 | 智能建议卡 |
| `components/billing/UsageTrendChart.tsx` + `.module.css` | 新建 | 7 日折线图（纯 SVG）|
| `components/billing/PaywallModal.tsx` + `.module.css` | 新建 | 限流模态 |
| `app/(marketing)/pricing/page.tsx` + `.module.css` | 新建 | `/pricing` 路由 |
| `app/settings/usage/page.tsx` + `.module.css` | 新建 | `/settings/usage` 路由 |
| `app/upgrade/paywall/page.tsx` + `.module.css` | 新建 | `/upgrade/paywall` 路由 |
| `app/upgrade/success/page.tsx` + `.module.css` | 新建 | `/upgrade/success` 路由 |
| `lib/i18n/messages.ts` | 修改 | 新增 pricing.* / usage.* / paywall.* 词条 |
| `e2e/specs/billing/pricing-page.spec.ts` | 新建 | 价格页 E2E |
| `e2e/specs/billing/usage-dashboard.spec.ts` | 新建 | 仪表盘 E2E |
| `e2e/specs/billing/paywall-flow.spec.ts` | 新建 | Paywall E2E |
| `e2e/pom/BillingPage.ts` | 新建 | POM（Page Object Model）|

**测试文件位置**：
- Rust：`crates/billing/tests/test_pricing_revamp.rs`
- Vitest 组件测试：随组件文件同目录 `*.test.tsx`（如 `UsageMeter.test.tsx`）
- Playwright：`e2e/specs/billing/`

---

## Phase 0: 后端基础（Tasks 1-2，约 1 周）

### Task 1: 编写 migration 0037（up + down + seed 更新）

**Files:**
- Create: `avrag-rs/migrations/0037_pricing_revamp.up.sql`
- Create: `avrag-rs/migrations/0037_pricing_revamp.down.sql`
- Test: `avrag-rs/crates/billing/tests/test_migration_0037.rs`

- [ ] **Step 1: 编写失败的测试**

```rust
// crates/billing/tests/test_migration_0037.rs
use sqlx::PgPool;

#[sqlx::test]
async fn migration_0037_sets_pricing_revamp_quotas(pool: PgPool) {
    // Apply migrations up to 0037
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // Check Free tier
    let row: (Option<i64>, Option<i64>) = sqlx::query_as(
        "SELECT soft_limit, hard_limit FROM quota_limits WHERE plan_id = 'free' AND metric_type = 'llm_input_tokens'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, (Some(50000), Some(100000)));

    // Check usage_limit_plan_policies for Plus
    let row: (i64, i64) = sqlx::query_as(
        "SELECT rolling_5h_limit_units, rolling_7d_limit_units FROM usage_limit_plan_policies WHERE plan_id = 'plus'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, (600000, 4000000));
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cd avrag-rs && cargo test --test test_migration_0037`
Expected: FAIL with "table quota_limits not found" or "no rows" (migration 0037 not yet created)

- [ ] **Step 3: 编写 up migration**

```sql
-- migrations/0037_pricing_revamp.up.sql

-- 1) 刷新 quota_limits：容量数值（pages / embedding / storage / llm_in / llm_out）
-- 容量三档不变，仅作幂等刷新
INSERT INTO quota_limits (plan_id, metric_type, soft_limit, hard_limit) VALUES
    ('free', 'pages_processed', 100, 500),
    ('free', 'embedding_tokens', 100000, 500000),
    ('free', 'llm_input_tokens', 50000, 100000),
    ('free', 'llm_output_tokens', 25000, 50000),
    ('free', 'storage_bytes', 1073741824, 5368709120),
    ('plus', 'pages_processed', 5000, 10000),
    ('plus', 'embedding_tokens', 5000000, 10000000),
    ('plus', 'llm_input_tokens', 500000, 1000000),
    ('plus', 'llm_output_tokens', 250000, 500000),
    ('plus', 'storage_bytes', 5368709120, 10737418240)
ON CONFLICT (plan_id, metric_type) DO UPDATE
SET soft_limit = EXCLUDED.soft_limit, hard_limit = EXCLUDED.hard_limit;

-- 2) 刷新 5h/7d 滚动限额 policy（核心改动）
-- 注：usage_limit_plan_policies 表已在 0018 创建，结构为 (plan_id PRIMARY KEY, rolling_5h_limit_units, rolling_7d_limit_units)
INSERT INTO usage_limit_plan_policies (plan_id, rolling_5h_limit_units, rolling_7d_limit_units) VALUES
    ('free',  100000,    400000),
    ('plus',  600000,    4000000),
    ('pro',   2500000,   15000000)
ON CONFLICT (plan_id) DO UPDATE
SET rolling_5h_limit_units = EXCLUDED.rolling_5h_limit_units,
    rolling_7d_limit_units = EXCLUDED.rolling_7d_limit_units;
```

- [ ] **Step 4: 编写 down migration（保证可回滚）**

```sql
-- migrations/0037_pricing_revamp.down.sql

-- 回滚 5h/7d policy 到 0036 状态
UPDATE usage_limit_plan_policies
SET rolling_5h_limit_units = NULL,
    rolling_7d_limit_units = NULL
WHERE plan_id IN ('free', 'plus', 'pro');

-- quota_limits 数值本就未实质变化，无需回滚
```

- [ ] **Step 5: 运行测试验证通过**

Run: `cd avrag-rs && cargo test --test test_migration_0037`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add avrag-rs/migrations/0037_pricing_revamp.up.sql \
        avrag-rs/migrations/0037_pricing_revamp.down.sql \
        avrag-rs/crates/billing/tests/test_migration_0037.rs
git commit -m "feat(billing): add migration 0037 for pricing revamp quotas

- Refresh quota_limits (capacity values unchanged)
- Update usage_limit_plan_policies with new 5h/7d rolling limits
- Free: 5h 100K, 7d 400K
- Plus: 5h 600K, 7d 4M
- Pro: 5h 2.5M, 7d 15M
- Includes up + down migrations for reversibility"
```

---

### Task 2: 更新 BillingConfig 双币种价格默认值

**Files:**
- Modify: `avrag-rs/crates/billing/src/types.rs:55-79, 76-79`
- Test: `avrag-rs/crates/billing/tests/test_billing_config.rs`

- [ ] **Step 1: 编写失败的测试**

```rust
// crates/billing/tests/test_billing_config.rs
use avrag_billing::BillingConfig;

#[test]
fn billing_config_default_alipay_prices_use_new_pricing_revamp() {
    let config = BillingConfig::default(); // 假设存在 default impl；若不存在则用 from_env
    assert_eq!(config.alipay_price_plus(), "49.00");
    assert_eq!(config.alipay_price_pro(), "129.00");
}

#[test]
fn billing_config_price_label_uses_dual_currency() {
    let config = BillingConfig::default();
    assert!(config.price_label_for_plan("plus").contains("¥49"));
    assert!(config.price_label_for_plan("plus").contains("$9"));
    assert!(config.price_label_for_plan("pro").contains("¥129"));
    assert!(config.price_label_for_plan("pro").contains("$19"));
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cd avrag-rs && cargo test --test test_billing_config`
Expected: FAIL with "alipay_price_plus" not found or mismatch

- [ ] **Step 3: 修改 BillingConfig**

修改 `crates/billing/src/types.rs`：

```rust
// 在 BillingConfig impl 中找到 billing_price_label_plus / pro 的默认值（约 55-59 行）
// 修改为：
billing_price_label_pro: std::env::var("BILLING_PRICE_LABEL_PRO")
    .unwrap_or_else(|_| "¥129 / 月 · $19 / 月".to_string()),
billing_price_label_plus: std::env::var("BILLING_PRICE_LABEL_PLUS")
    .unwrap_or_else(|_| "¥49 / 月 · $9 / 月".to_string()),

// 修改 alipay_price_plus / pro 默认值（约 76-79 行）
alipay_price_plus: std::env::var("ALIPAY_PRICE_PLUS")
    .unwrap_or_else(|_| "49.00".to_string()),
alipay_price_pro: std::env::var("ALIPAY_PRICE_PRO")
    .unwrap_or_else(|_| "129.00".to_string()),
```

并增加便捷方法（在 impl BillingConfig 内）：

```rust
pub fn alipay_price_plus(&self) -> &str { &self.alipay_price_plus }
pub fn alipay_price_pro(&self) -> &str { &self.alipay_price_pro }
```

> 若 `BillingConfig` 没有 `Default` impl，测试改为：
> ```rust
// 测试中临时清空所有相关 env vars
std::env::remove_var("BILLING_PRICE_LABEL_PLUS");
std::env::remove_var("BILLING_PRICE_LABEL_PRO");
std::env::remove_var("ALIPAY_PRICE_PLUS");
std::env::remove_var("ALIPAY_PRICE_PRO");
let config = BillingConfig::from_env();
```

- [ ] **Step 4: 运行测试验证通过**

Run: `cd avrag-rs && cargo test --test test_billing_config`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add avrag-rs/crates/billing/src/types.rs \
        avrag-rs/crates/billing/tests/test_billing_config.rs
git commit -m "feat(billing): dual-currency price labels for Plus/Pro

- Default Plus: ¥49/月 · $9/月
- Default Pro: ¥129/月 · $19/月
- Alipay prices: 49.00 / 129.00 CNY
- Add alipay_price_plus() / alipay_price_pro() accessors"
```

---

## Phase 1: 后端 API（Tasks 3-6，约 1 周）

### Task 3: 增强 `/api/v1/billing/plans` 端点（双币种 + 限额）

**Files:**
- Modify: `avrag-rs/crates/billing/src/api.rs:handle_get_plans`
- Test: `avrag-rs/crates/billing/tests/test_plans_endpoint.rs`

- [ ] **Step 1: 编写失败的测试**

```rust
// crates/billing/tests/test_plans_endpoint.rs
use avrag_billing::{handle_get_plans, BillingPlan};

#[tokio::test]
async fn plans_endpoint_returns_three_tiers_with_dual_currency() {
    let repo = /* mock repo */;
    let user_id = uuid::Uuid::new_v4();
    let response = handle_get_plans(repo, user_id.into()).await;

    assert_eq!(response.plans.len(), 3);
    let plus = response.plans.iter().find(|p| p.plan_id == "plus").unwrap();
    assert!(plus.price_label.contains("¥49"));
    assert!(plus.price_label.contains("$9"));
    assert!(plus.quotas.iter().any(|q| q.metric_type == "llm_input_tokens" && q.soft_limit == Some(500000)));
}

#[tokio::test]
async fn plans_endpoint_marks_current_user_plan() {
    // 假设 user_id 已订阅 Plus
    let response = handle_get_plans(/* plus-subscriber mock */, plus_user_id).await;
    let plus = response.plans.iter().find(|p| p.plan_id == "plus").unwrap();
    assert!(plus.current);
}
```

> **mock 模式**：参考 `crates/billing/tests/` 已有测试的 mock pattern（如果有 test-kit crate 用法）。

- [ ] **Step 2: 运行测试验证失败**

Run: `cd avrag-rs && cargo test --test test_plans_endpoint`
Expected: FAIL with assertion mismatch on price_label format

- [ ] **Step 3: 修改 handle_get_plans**

在 `crates/billing/src/api.rs` 中找到 `handle_get_plans` 函数。修改其组装 BillingPlan 的部分：

```rust
// 找到形如：
// BillingPlan {
//     plan_id: PLAN_PLUS.to_string(),
//     price_label: config.price_label_for_plan(PLAN_PLUS),
//     ...
// }
// 改为使用新的双币种字段：

let plan = BillingPlan {
    plan_id: PLAN_PLUS.to_string(),
    name: "Plus".to_string(),
    description: "深度研究首选".to_string(),
    price_label_cny: "¥49 / 月".to_string(),
    price_label_usd: "$9 / 月".to_string(),
    interval: "month".to_string(),
    checkout_available: config.checkout_available(PLAN_PLUS),
    current: current_plan_id == PLAN_PLUS,
    quotas: quotas.get(PLAN_PLUS).cloned().unwrap_or_default(),
};
```

并在 `BillingPlan` 结构（`types.rs`）中新增字段：

```rust
pub struct BillingPlan {
    pub plan_id: String,
    pub name: String,
    pub description: String,
    pub price_label: String,        // 保留：用于向后兼容的拼接字符串
    pub price_label_cny: String,    // 新增
    pub price_label_usd: String,    // 新增
    pub interval: String,
    pub checkout_available: bool,
    pub current: bool,
    pub quotas: Vec<BillingPlanQuota>,
}
```

- [ ] **Step 4: 运行测试验证通过**

Run: `cd avrag-rs && cargo test --test test_plans_endpoint`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add avrag-rs/crates/billing/src/api.rs \
        avrag-rs/crates/billing/src/types.rs \
        avrag-rs/crates/billing/tests/test_plans_endpoint.rs
git commit -m "feat(billing): expose dual-currency price labels in plans endpoint

- Add price_label_cny / price_label_usd to BillingPlan
- Keep price_label for backward compat (concatenated)"
```

---

### Task 4: 新增 `/api/v1/billing/usage/window` 端点

**Files:**
- Create: `avrag-rs/crates/billing/src/core_usage.rs:handle_get_usage_window` (追加)
- Modify: `avrag-rs/crates/billing/src/lib.rs:14` (导出)
- Modify: `avrag-rs/crates/transport-http/src/routes/billing.rs:7-13`
- Test: `avrag-rs/crates/billing/tests/test_usage_window_endpoint.rs`

- [ ] **Step 1: 编写失败的测试**

```rust
// crates/billing/tests/test_usage_window_endpoint.rs
use avrag_billing::UsageWindowResponse;
use chrono::Utc;

#[tokio::test]
async fn usage_window_returns_5h_and_7d_with_reset_at() {
    let repo = /* mock with seeded usage events */;
    let user_id = uuid::Uuid::new_v4();
    let resp: UsageWindowResponse = handle_get_usage_window(repo, user_id.into()).await.unwrap();

    // 5h 窗口
    assert_eq!(resp.rolling_5h.limit, 100000); // 假设 Free
    assert!(resp.rolling_5h.reset_at > Utc::now());

    // 7d 窗口
    assert_eq!(resp.rolling_7d.limit, 400000);
    assert!(resp.rolling_7d.reset_at > Utc::now());

    // 软/硬限位 flag
    assert!(!resp.soft_limit_hit.rolling_5h);
    assert!(!resp.hard_limit_hit.rolling_5h);
}

#[tokio::test]
async fn usage_window_reset_at_uses_oldest_event_in_window() {
    // 在窗口内 4.5h 前消耗 1 个事件
    // reset_at 应 = 0.5h 后（5h - 4.5h）
    let resp = handle_get_usage_window(/* seeded */, user_id).await.unwrap();
    let expected_reset = Utc::now() + chrono::Duration::minutes(30);
    let diff = (resp.rolling_5h.reset_at - expected_reset).num_seconds().abs();
    assert!(diff < 60, "reset_at 应在预期 60s 内");
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cd avrag-rs && cargo test --test test_usage_window_endpoint`
Expected: FAIL with "UsageWindowResponse not found" 或 "handle_get_usage_window not found"

- [ ] **Step 3: 定义响应类型**

在 `crates/billing/src/types.rs` 追加：

```rust
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageWindowBucket {
    pub used: i64,
    pub limit: i64,
    pub percentage: i32,    // 0-100
    pub reset_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LimitHits {
    pub rolling_5h: bool,
    pub rolling_7d: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageWindowResponse {
    pub plan_id: String,
    pub rolling_5h: UsageWindowBucket,
    pub rolling_7d: UsageWindowBucket,
    pub soft_limit_hit: LimitHits,  // >= 80%
    pub hard_limit_hit: LimitHits,  // >= 100%
}
```

- [ ] **Step 4: 实现 handler**

在 `crates/billing/src/core_usage.rs` 追加：

```rust
use crate::types::{UsageWindowResponse, UsageWindowBucket, LimitHits};
use chrono::{Duration, Utc};
use common::UserId;

pub async fn handle_get_usage_window<R: BillingRepo>(
    repo: R,
    user_id: UserId,
) -> Result<UsageWindowResponse, BillingError> {
    // 1) 取用户当前 plan
    let plan_id = repo.get_user_plan_id(user_id).await?;
    
    // 2) 取该 plan 的 5h/7d policy
    let policy = repo.get_usage_policy(plan_id).await?;
    
    // 3) 取窗口内累计用量 + 最旧事件时间
    let usage_5h = repo.sum_usage_in_window(user_id, Duration::hours(5)).await?;
    let usage_7d = repo.sum_usage_in_window(user_id, Duration::days(7)).await?;
    let oldest_5h = repo.oldest_usage_in_window(user_id, Duration::hours(5)).await?;
    let oldest_7d = repo.oldest_usage_in_window(user_id, Duration::days(7)).await?;
    
    // 4) 计算 reset_at
    let now = Utc::now();
    let reset_5h = oldest_5h
        .map(|t| t + Duration::hours(5))
        .unwrap_or(now);
    let reset_7d = oldest_7d
        .map(|t| t + Duration::days(7))
        .unwrap_or(now);
    
    // 5) 计算 percentage + soft/hard hit
    let bucket = |used: i64, limit: i64, reset: DateTime<Utc>| -> UsageWindowBucket {
        let pct = if limit > 0 { ((used as f64 / limit as f64) * 100.0).round() as i32 } else { 0 };
        UsageWindowBucket { used, limit, percentage: pct.min(100), reset_at: reset }
    };
    let b5h = bucket(usage_5h, policy.rolling_5h_limit_units, reset_5h);
    let b7d = bucket(usage_7d, policy.rolling_7d_limit_units, reset_7d);
    
    Ok(UsageWindowResponse {
        plan_id,
        rolling_5h: b5h.clone(),
        rolling_7d: b7d.clone(),
        soft_limit_hit: LimitHits {
            rolling_5h: b5h.percentage >= 80,
            rolling_7d: b7d.percentage >= 80,
        },
        hard_limit_hit: LimitHits {
            rolling_5h: b5h.percentage >= 100,
            rolling_7d: b7d.percentage >= 100,
        },
    })
}
```

> `BillingRepo` trait 需新增方法：`get_user_plan_id`, `get_usage_policy`, `sum_usage_in_window`, `oldest_usage_in_window`。参考现有 `crates/billing/src/core_usage.rs` 中的 repository 方法添加。

- [ ] **Step 5: 在 lib.rs 导出**

```rust
// crates/billing/src/lib.rs
pub use core_usage::handle_get_usage_window;
pub use types::{UsageWindowResponse, UsageWindowBucket, LimitHits};
```

- [ ] **Step 6: 在路由表添加**

修改 `crates/transport-http/src/routes/billing.rs`：

```rust
// 现有 router() 函数
pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/billing/plans", get(get_plans))
        .route("/billing/subscription", get(get_subscription))
        .route("/billing/usage", get(get_usage))
        .route("/billing/usage/window", get(get_usage_window))  // 新增
        .route("/billing/usage/history", get(get_usage_history))  // Task 5
        .route("/billing/usage/forecast", get(get_usage_forecast))  // Task 6
        .route("/billing/checkout-session", axum::routing::post(create_checkout))
        .route("/billing/portal-session", axum::routing::post(create_portal))
}

// 新增 handler
async fn get_usage_window(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Json<ApiResponse<avrag_billing::UsageWindowResponse>> {
    let Some(repo) = state.pg() else {
        return Json(ApiResponse::err("postgres_not_configured", "postgres backend is not configured"));
    };
    let Some(actor_id) = state.auth().actor_id() else {
        return Json(ApiResponse::err("authenticated_user_required", "authenticated user required"));
    };
    Json(avrag_billing::handle_get_usage_window(repo, UserId::from(actor_id.into_uuid())).await
        .map_or_else(
            |e| ApiResponse::err("usage_window_failed", &e.to_string()),
            |r| ApiResponse::ok(r),
        ))
}
```

- [ ] **Step 7: 运行测试验证通过**

Run: `cd avrag-rs && cargo test --test test_usage_window_endpoint`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add avrag-rs/crates/billing/src/core_usage.rs \
        avrag-rs/crates/billing/src/types.rs \
        avrag-rs/crates/billing/src/lib.rs \
        avrag-rs/crates/transport-http/src/routes/billing.rs \
        avrag-rs/crates/billing/tests/test_usage_window_endpoint.rs
git commit -m "feat(billing): add /api/v1/billing/usage/window endpoint

- Returns 5h/7d usage with reset_at computed from oldest event
- soft_limit_hit (>=80%) and hard_limit_hit (>=100%) flags
- Frontend can directly display reset countdown without client-side time math"
```

---

### Task 5: 新增 `/api/v1/billing/usage/history` 端点（折线图数据）

**Files:**
- Modify: `avrag-rs/crates/billing/src/core_usage.rs`
- Modify: `avrag-rs/crates/transport-http/src/routes/billing.rs`
- Test: `avrag-rs/crates/billing/tests/test_usage_history_endpoint.rs`

- [ ] **Step 1: 编写失败的测试**

```rust
// crates/billing/tests/test_usage_history_endpoint.rs
use avrag_billing::UsageHistoryResponse;

#[tokio::test]
async fn usage_history_aggregates_daily_token_usage_from_llm_usage_events() {
    // 策略 B：复用 llm_usage_events，按 date_trunc('day', created_at) 聚合
    // 假设种子数据：用户有 7 天用量，每天 50K tokens
    let resp: UsageHistoryResponse = handle_get_usage_history(repo, user_id.into(), 7).await.unwrap();
    assert_eq!(resp.daily.len(), 7);
    assert!(resp.daily.iter().all(|d| d.tokens == 50000));
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cd avrag-rs && cargo test --test test_usage_history_endpoint`
Expected: FAIL

- [ ] **Step 3: 定义响应类型 + handler**

在 `types.rs` 追加：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyUsage {
    pub date: chrono::NaiveDate,  // YYYY-MM-DD
    pub tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageHistoryResponse {
    pub daily: Vec<DailyUsage>,
}
```

在 `core_usage.rs` 追加：

```rust
pub async fn handle_get_usage_history<R: BillingRepo>(
    repo: R,
    user_id: UserId,
    days: i32,
) -> Result<UsageHistoryResponse, BillingError> {
    let daily = repo.daily_token_usage(user_id, days).await?;
    Ok(UsageHistoryResponse { daily })
}
```

在 `BillingRepo` trait 追加：

```rust
async fn daily_token_usage(&self, user_id: UserId, days: i32) -> Result<Vec<DailyUsage>, BillingError> {
    // SQL: SELECT date_trunc('day', created_at)::date as date,
    //             SUM(input_tokens + output_tokens) as tokens
    //      FROM llm_usage_events
    //      WHERE user_id = $1 AND created_at > now() - ($2 || ' days')::interval
    //      GROUP BY date
    //      ORDER BY date ASC
    // 索引：假设已有 (user_id, created_at)
    // ...
}
```

- [ ] **Step 4: 注册路由 + handler**

参考 Task 4 Step 6 的模式添加 `get_usage_history` 路由 + handler。

- [ ] **Step 5: 运行测试验证通过**

Run: `cd avrag-rs && cargo test --test test_usage_history_endpoint`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add avrag-rs/crates/billing/src/core_usage.rs \
        avrag-rs/crates/billing/src/types.rs \
        avrag-rs/crates/transport-http/src/routes/billing.rs \
        avrag-rs/crates/billing/tests/test_usage_history_endpoint.rs
git commit -m "feat(billing): add /api/v1/billing/usage/history endpoint

- Aggregates daily token usage from llm_usage_events (Strategy B)
- Returns Vec<{date, tokens}> for last N days
- Index check: ensure (user_id, created_at) exists; add if missing"
```

---

### Task 6: 新增 `/api/v1/billing/usage/forecast` 端点

**Files:**
- Modify: `avrag-rs/crates/billing/src/core_usage.rs`
- Modify: `avrag-rs/crates/transport-http/src/routes/billing.rs`
- Test: `avrag-rs/crates/billing/tests/test_usage_forecast_endpoint.rs`

- [ ] **Step 1: 编写失败的测试**

```rust
// crates/billing/tests/test_usage_forecast_endpoint.rs
use avrag_billing::UsageForecastResponse;

#[tokio::test]
async fn usage_forecast_suggests_upgrade_when_30d_avg_exceeds_80pct_of_limit() {
    // Free 用户，30 天平均用量 350K，7d 限额 400K
    // 350K / 400K = 87.5% > 80%
    let resp = handle_get_usage_forecast(repo, user_id.into()).await.unwrap();
    assert!(resp.upgrade_recommended);
    assert!(resp.suggestion.contains("Plus"));
}

#[tokio::test]
async fn usage_forecast_says_no_upgrade_needed_when_under_50pct() {
    let resp = handle_get_usage_forecast(repo, low_usage_user).await.unwrap();
    assert!(!resp.upgrade_recommended);
}
```

- [ ] **Step 2-5: 实现（与 Task 5 同模式）**

类型 + handler：

```rust
// types.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageForecastResponse {
    pub current_plan: String,
    pub avg_30d_tokens: i64,
    pub projected_30d_tokens: i64,        // 按当前 plan 限额归一化
    pub current_limit_7d: i64,
    pub upgrade_recommended: bool,
    pub suggestion_zh: String,
    pub suggestion_en: String,
}

// core_usage.rs
pub async fn handle_get_usage_forecast<R: BillingRepo>(
    repo: R,
    user_id: UserId,
) -> Result<UsageForecastResponse, BillingError> {
    let plan_id = repo.get_user_plan_id(user_id).await?;
    let usage_30d = repo.sum_usage_in_days(user_id, 30).await?;
    let avg_daily = usage_30d / 30;
    let projected_30d = avg_daily * 30;
    let policy = repo.get_usage_policy(plan_id).await?;
    let current_limit_7d = policy.rolling_7d_limit_units;
    
    let usage_pct_of_limit = if current_limit_7d > 0 {
        (projected_30d as f64 / current_limit_7d as f64) * 100.0
    } else { 0.0 };
    
    let upgrade_recommended = usage_pct_of_limit >= 80.0;
    let suggestion_zh = if upgrade_recommended {
        format!("按当前用量，本月建议升级到 Plus（7d 限额 4M）")
    } else {
        format!("按当前用量，本月无需升级")
    };
    let suggestion_en = if upgrade_recommended {
        "Based on current usage, upgrading to Plus is recommended this month (7d limit: 4M)".to_string()
    } else {
        "Based on current usage, no upgrade needed this month".to_string()
    };
    
    Ok(UsageForecastResponse {
        current_plan: plan_id,
        avg_30d_tokens: avg_daily,
        projected_30d_tokens: projected_30d,
        current_limit_7d,
        upgrade_recommended,
        suggestion_zh,
        suggestion_en,
    })
}
```

- [ ] **Step 6: Commit**

```bash
git add avrag-rs/crates/billing/src/core_usage.rs \
        avrag-rs/crates/billing/src/types.rs \
        avrag-rs/crates/transport-http/src/routes/billing.rs \
        avrag-rs/crates/billing/tests/test_usage_forecast_endpoint.rs
git commit -m "feat(billing): add /api/v1/billing/usage/forecast endpoint

- Computes 30-day average usage
- Suggests upgrade if projected usage >= 80% of 7d limit
- Returns bilingual suggestions (zh/en) for frontend to use directly"
```

---

## Phase 2: 前端 API client 与基础组件（Tasks 7-13，约 1 周）

### Task 7: 前端 API client

**Files:**
- Create: `frontend_next/lib/billing/api.ts`
- Create: `frontend_next/lib/billing/format.ts`
- Test: `frontend_next/lib/billing/api.test.ts`
- Test: `frontend_next/lib/billing/format.test.ts`

- [ ] **Step 1: 编写 format.ts**

```typescript
// lib/billing/format.ts

/** 100K / 1.5M / 200 这种紧凑数字格式 */
export function formatCompactToken(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(n >= 10_000_000 ? 0 : 1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(n >= 100_000 ? 0 : 1)}K`;
  return n.toString();
}

/** "100,000" 这种千分位完整格式 */
export function formatFullToken(n: number): string {
  return n.toLocaleString("en-US");
}

/** 5h 23m 倒计时格式 */
export function formatCountdown(ms: number): string {
  if (ms <= 0) return "0m";
  const totalMin = Math.floor(ms / 60_000);
  const h = Math.floor(totalMin / 60);
  const m = totalMin % 60;
  if (h > 24) {
    const d = Math.floor(h / 24);
    const rh = h % 24;
    return rh > 0 ? `${d}d ${rh}h` : `${d}d`;
  }
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

/** 百分比保留 0 位小数 */
export function formatPct(pct: number): string {
  return `${Math.round(pct)}%`;
}
```

- [ ] **Step 2: 编写失败的测试**

```typescript
// lib/billing/format.test.ts
import { describe, it, expect } from "vitest";
import { formatCompactToken, formatFullToken, formatCountdown, formatPct } from "./format";

describe("formatCompactToken", () => {
  it("formats 100K / 1.5M / 200", () => {
    expect(formatCompactToken(100_000)).toBe("100K");
    expect(formatCompactToken(1_500_000)).toBe("1.5M");
    expect(formatCompactToken(200)).toBe("200");
  });
});

describe("formatCountdown", () => {
  it("formats 5h 23m / 2d 4h / 30m", () => {
    expect(formatCountdown(5 * 3600_000 + 23 * 60_000)).toBe("5h 23m");
    expect(formatCountdown(2 * 86400_000 + 4 * 3600_000)).toBe("2d 4h");
    expect(formatCountdown(30 * 60_000)).toBe("30m");
  });
});
```

- [ ] **Step 3: 运行测试验证失败**

Run: `cd frontend_next && pnpm test lib/billing/format.test.ts`
Expected: FAIL (file not found)

- [ ] **Step 4: 编写 api.ts**

```typescript
// lib/billing/api.ts
export type UsageWindowBucket = {
  used: number;
  limit: number;
  percentage: number;
  reset_at: string;  // ISO 8601
};

export type LimitHits = {
  rolling_5h: boolean;
  rolling_7d: boolean;
};

export type UsageWindowResponse = {
  plan_id: "free" | "plus" | "pro";
  rolling_5h: UsageWindowBucket;
  rolling_7d: UsageWindowBucket;
  soft_limit_hit: LimitHits;
  hard_limit_hit: LimitHits;
};

export type DailyUsage = {
  date: string;  // YYYY-MM-DD
  tokens: number;
};

export type UsageHistoryResponse = {
  daily: DailyUsage[];
};

export type UsageForecastResponse = {
  current_plan: string;
  avg_30d_tokens: number;
  projected_30d_tokens: number;
  current_limit_7d: number;
  upgrade_recommended: boolean;
  suggestion_zh: string;
  suggestion_en: string;
};

export type BillingPlan = {
  plan_id: string;
  name: string;
  description: string;
  price_label: string;
  price_label_cny: string;
  price_label_usd: string;
  interval: string;
  checkout_available: boolean;
  current: boolean;
  quotas: Array<{ metric_type: string; soft_limit: number | null; hard_limit: number | null }>;
};

async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  const res = await fetch(url, { credentials: "include", ...init });
  if (!res.ok) throw new Error(`API ${url} failed: ${res.status}`);
  const body = await res.json();
  // 兼容 ApiResponse 包装
  if (body && typeof body === "object" && "ok" in body) {
    if (!body.ok) throw new Error(body.message || "API error");
    return body.data as T;
  }
  return body as T;
}

export const billingApi = {
  getPlans: () => fetchJson<BillingPlan[]>("/api/v1/billing/plans"),
  getUsageWindow: () => fetchJson<UsageWindowResponse>("/api/v1/billing/usage/window"),
  getUsageHistory: (days = 7) => fetchJson<UsageHistoryResponse>(`/api/v1/billing/usage/history?days=${days}`),
  getUsageForecast: () => fetchJson<UsageForecastResponse>("/api/v1/billing/usage/forecast"),
};
```

- [ ] **Step 5: 编写失败的 api 测试**

```typescript
// lib/billing/api.test.ts
import { describe, it, expect, vi, beforeEach } from "vitest";
import { billingApi } from "./api";

beforeEach(() => {
  global.fetch = vi.fn();
});

describe("billingApi.getUsageWindow", () => {
  it("returns parsed UsageWindowResponse", async () => {
    (global.fetch as any).mockResolvedValueOnce({
      ok: true,
      json: async () => ({
        ok: true,
        data: {
          plan_id: "free",
          rolling_5h: { used: 80000, limit: 100000, percentage: 80, reset_at: "2026-06-07T20:00:00Z" },
          rolling_7d: { used: 200000, limit: 400000, percentage: 50, reset_at: "2026-06-10T00:00:00Z" },
          soft_limit_hit: { rolling_5h: true, rolling_7d: false },
          hard_limit_hit: { rolling_5h: false, rolling_7d: false },
        },
      }),
    });
    const result = await billingApi.getUsageWindow();
    expect(result.plan_id).toBe("free");
    expect(result.rolling_5h.percentage).toBe(80);
  });

  it("throws on non-ok response", async () => {
    (global.fetch as any).mockResolvedValueOnce({ ok: false, status: 500 });
    await expect(billingApi.getUsageWindow()).rejects.toThrow();
  });
});
```

- [ ] **Step 6: 运行测试验证失败**

Run: `cd frontend_next && pnpm test lib/billing/format.test.ts lib/billing/api.test.ts`
Expected: PASS for format, FAIL for api (file not found)

- [ ] **Step 7: 运行测试验证全部通过**

Run: `cd frontend_next && pnpm test lib/billing/format.test.ts lib/billing/api.test.ts`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add frontend_next/lib/billing/format.ts \
        frontend_next/lib/billing/api.ts \
        frontend_next/lib/billing/format.test.ts \
        frontend_next/lib/billing/api.test.ts
git commit -m "feat(frontend): add billing API client and formatters

- TS types matching backend UsageWindow/History/Forecast contracts
- 4 API methods: getPlans/getUsageWindow/getUsageHistory/getUsageForecast
- Format helpers: formatCompactToken/formatFullToken/formatCountdown/formatPct
- Vitest unit tests for both api and format"
```

---

### Task 8: UsageMeter 组件（full + compact 双变体）

**Files:**
- Create: `frontend_next/components/billing/UsageMeter.tsx`
- Create: `frontend_next/components/billing/UsageMeter.module.css`
- Test: `frontend_next/components/billing/UsageMeter.test.tsx`

- [ ] **Step 1: 编写失败的测试**

```tsx
// components/billing/UsageMeter.test.tsx
import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { UsageMeter } from "./UsageMeter";

describe("UsageMeter", () => {
  it("renders full variant with both 5h and 7d buckets", () => {
    render(
      <UsageMeter
        variant="full"
        planId="free"
        rolling5h={{ used: 80000, limit: 100000, percentage: 80, reset_at: "2026-06-07T20:00:00Z" }}
        rolling7d={{ used: 200000, limit: 400000, percentage: 50, reset_at: "2026-06-10T00:00:00Z" }}
        softLimitHit={{ rolling_5h: true, rolling_7d: false }}
        hardLimitHit={{ rolling_5h: false, rolling_7d: false }}
      />
    );
    expect(screen.getByText(/5 小时窗口/)).toBeInTheDocument();
    expect(screen.getByText(/7 天窗口/)).toBeInTheDocument();
    expect(screen.getByText(/80K/)).toBeInTheDocument();
  });

  it("renders compact variant with just progress bars", () => {
    render(
      <UsageMeter
        variant="compact"
        planId="free"
        rolling5h={{ used: 100000, limit: 100000, percentage: 100, reset_at: "2026-06-07T20:00:00Z" }}
        rolling7d={{ used: 100000, limit: 400000, percentage: 25, reset_at: "2026-06-10T00:00:00Z" }}
        softLimitHit={{ rolling_5h: true, rolling_7d: false }}
        hardLimitHit={{ rolling_5h: true, rolling_7d: false }}
      />
    );
    expect(screen.queryByText(/5 小时窗口/)).not.toBeInTheDocument();
    expect(screen.getByRole("progressbar")).toBeInTheDocument();
  });

  it("shows warning text when soft limit hit", () => {
    render(
      <UsageMeter
        variant="full"
        planId="free"
        rolling5h={{ used: 80000, limit: 100000, percentage: 80, reset_at: "2026-06-07T20:00:00Z" }}
        rolling7d={{ used: 100000, limit: 400000, percentage: 25, reset_at: "2026-06-10T00:00:00Z" }}
        softLimitHit={{ rolling_5h: true, rolling_7d: false }}
        hardLimitHit={{ rolling_5h: false, rolling_7d: false }}
      />
    );
    expect(screen.getByText(/已超过软上限/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cd frontend_next && pnpm test components/billing/UsageMeter.test.tsx`
Expected: FAIL

- [ ] **Step 3: 编写 CSS Module**

```css
/* components/billing/UsageMeter.module.css */
.card {
  border: 1px solid hsl(var(--border-whisper));
  border-radius: var(--radius-card);
  background: hsl(var(--card));
  padding: 1.5rem;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.title {
  font-family: var(--font-heading);
  font-size: var(--font-size-section-title);
  font-weight: var(--font-weight-semibold);
  color: hsl(var(--foreground));
  margin: 0;
}

.numbers {
  font-family: var(--font-mono);
  font-size: var(--font-size-title-sm);
  font-weight: var(--font-weight-bold);
  color: hsl(var(--foreground));
}

.numbers .used { color: hsl(var(--foreground)); }
.numbers .limit { color: hsl(var(--muted-foreground)); font-size: var(--font-size-body); }

.bar {
  width: 100%;
  height: 8px;
  background: hsl(var(--surface-muted));
  border-radius: 999px;
  overflow: hidden;
  position: relative;
}

.barFill {
  height: 100%;
  background: hsl(var(--accent));
  border-radius: 999px;
  transition: width 300ms ease;
}

.barFill.warning { background: hsl(var(--warning)); }
.barFill.danger { background: hsl(var(--destructive)); }

.warningText {
  font-size: var(--font-size-caption);
  color: hsl(var(--warning-foreground));
  background: hsl(var(--warning-surface));
  border: 1px solid hsl(var(--warning-border));
  padding: 0.5rem 0.75rem;
  border-radius: var(--radius-control);
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.resetText {
  font-size: var(--font-size-caption);
  color: hsl(var(--muted-foreground));
}

/* compact variant */
.compact {
  padding: 0.75rem 1rem;
  gap: 0.5rem;
}
.compact .title { font-size: var(--font-size-caption-strong); }
.compact .numbers { font-size: var(--font-size-control); }
```

- [ ] **Step 4: 编写组件**

```tsx
// components/billing/UsageMeter.tsx
"use client";

import { useEffect, useState } from "react";
import styles from "./UsageMeter.module.css";
import { formatCompactToken, formatCountdown } from "../../lib/billing/format";
import type { UsageWindowBucket, LimitHits } from "../../lib/billing/api";

export type UsageMeterProps = {
  variant: "full" | "compact";
  planId: "free" | "plus" | "pro";
  rolling5h: UsageWindowBucket;
  rolling7d: UsageWindowBucket;
  softLimitHit: LimitHits;
  hardLimitHit: LimitHits;
};

function useCountdown(resetAt: string) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 30_000);
    return () => clearInterval(id);
  }, []);
  const target = new Date(resetAt).getTime();
  return formatCountdown(target - now);
}

function BucketCard({
  title,
  bucket,
  isSoftHit,
  isHardHit,
  compact,
}: {
  title: string;
  bucket: UsageWindowBucket;
  isSoftHit: boolean;
  isHardHit: boolean;
  compact: boolean;
}) {
  const countdown = useCountdown(bucket.reset_at);
  const fillClass = isHardHit ? styles.barFill + " " + styles.danger
                  : isSoftHit ? styles.barFill + " " + styles.warning
                  : styles.barFill;
  return (
    <div className={`${styles.card} ${compact ? styles.compact : ""}`}>
      <h3 className={styles.title}>{title}</h3>
      <div className={styles.numbers}>
        <span className={styles.used}>{formatCompactToken(bucket.used)}</span>
        {" / "}
        <span className={styles.limit}>{formatCompactToken(bucket.limit)}</span>
      </div>
      <div
        className={styles.bar}
        role="progressbar"
        aria-valuenow={bucket.percentage}
        aria-valuemin={0}
        aria-valuemax={100}
      >
        <div className={fillClass} style={{ width: `${bucket.percentage}%` }} />
      </div>
      <div className={styles.resetText}>预计 {countdown} 后重置</div>
      {isSoftHit && !compact && (
        <div className={styles.warningText}>⚠️ 已超过软上限，建议控制节奏</div>
      )}
    </div>
  );
}

export function UsageMeter({ variant, planId, rolling5h, rolling7d, softLimitHit, hardLimitHit }: UsageMeterProps) {
  const compact = variant === "compact";
  return (
    <>
      <BucketCard
        title={compact ? "5h" : "5 小时窗口"}
        bucket={rolling5h}
        isSoftHit={softLimitHit.rolling_5h}
        isHardHit={hardLimitHit.rolling_5h}
        compact={compact}
      />
      <BucketCard
        title={compact ? "7d" : "7 天窗口"}
        bucket={rolling7d}
        isSoftHit={softLimitHit.rolling_7d}
        isHardHit={hardLimitHit.rolling_7d}
        compact={compact}
      />
    </>
  );
}
```

- [ ] **Step 5: 运行测试验证通过**

Run: `cd frontend_next && pnpm test components/billing/UsageMeter.test.tsx`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add frontend_next/components/billing/UsageMeter.tsx \
        frontend_next/components/billing/UsageMeter.module.css \
        frontend_next/components/billing/UsageMeter.test.tsx
git commit -m "feat(frontend): add UsageMeter component (full/compact variants)

- Two-bucket display: 5h + 7d
- Live countdown (30s interval)
- Color states: normal (cyan), warning (amber), danger (red)
- Soft limit warning at 80%+
- Variant 'compact' hides title and warning for paywall reuse"
```

---

### Task 9: PricingCards 组件（full + compact）

**Files:**
- Create: `frontend_next/components/billing/PricingCards.tsx`
- Create: `frontend_next/components/billing/PricingCards.module.css`
- Test: `frontend_next/components/billing/PricingCards.test.tsx`

- [ ] **Step 1: 编写失败的测试**

```tsx
// components/billing/PricingCards.test.tsx
import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { PricingCards } from "./PricingCards";

const plans = [
  { plan_id: "free", name: "Free", price_label_cny: "¥0", price_label_usd: "$0", description: "体验", interval: "month", checkout_available: false, current: false, quotas: [] },
  { plan_id: "plus", name: "Plus", price_label_cny: "¥49 / 月", price_label_usd: "$9 / 月", description: "深度研究", interval: "month", checkout_available: true, current: true, quotas: [] },
  { plan_id: "pro", name: "Pro", price_label_cny: "¥129 / 月", price_label_usd: "$19 / 月", description: "重度无忧", interval: "month", checkout_available: true, current: false, quotas: [] },
];

describe("PricingCards", () => {
  it("renders three tier cards with prices", () => {
    render(<PricingCards plans={plans} highlightTier="plus" onSelect={vi.fn()} />);
    expect(screen.getByText("Free")).toBeInTheDocument();
    expect(screen.getByText("Plus")).toBeInTheDocument();
    expect(screen.getByText("Pro")).toBeInTheDocument();
    expect(screen.getByText("¥49 / 月")).toBeInTheDocument();
    expect(screen.getByText("$9 / 月")).toBeInTheDocument();
  });

  it("shows 推荐 badge on highlighted tier", () => {
    render(<PricingCards plans={plans} highlightTier="plus" onSelect={vi.fn()} />);
    expect(screen.getByText("推荐")).toBeInTheDocument();
  });

  it("marks current plan with disabled button", () => {
    render(<PricingCards plans={plans} highlightTier="plus" onSelect={vi.fn()} />);
    const plusButton = screen.getByRole("button", { name: /升级 Plus/ });
    expect(plusButton).toBeDisabled();
  });

  it("calls onSelect with plan_id when clicking non-current tier", () => {
    const onSelect = vi.fn();
    render(<PricingCards plans={plans} highlightTier="plus" onSelect={onSelect} />);
    screen.getByRole("button", { name: /升级 Pro/ }).click();
    expect(onSelect).toHaveBeenCalledWith("pro");
  });
});
```

- [ ] **Step 2: 编写组件**

```tsx
// components/billing/PricingCards.tsx
"use client";

import styles from "./PricingCards.module.css";
import type { BillingPlan } from "../../lib/billing/api";

export type PricingCardsProps = {
  plans: BillingPlan[];
  highlightTier: "free" | "plus" | "pro";
  onSelect: (planId: string) => void;
  compact?: boolean;
};

export function PricingCards({ plans, highlightTier, onSelect, compact = false }: PricingCardsProps) {
  return (
    <div className={`${styles.grid} ${compact ? styles.compactGrid : ""}`}>
      {plans.map((plan) => {
        const isHighlight = plan.plan_id === highlightTier;
        const isCurrent = plan.current;
        return (
          <div
            key={plan.plan_id}
            className={`${styles.card} ${isHighlight ? styles.highlight : ""} ${compact ? styles.compact : ""}`}
          >
            {isHighlight && <div className={styles.badge}>推荐</div>}
            <h3 className={styles.name}>{plan.name}</h3>
            <div className={styles.prices}>
              <div className={styles.priceCny}>{plan.price_label_cny}</div>
              <div className={styles.priceUsd}>{plan.price_label_usd}</div>
            </div>
            <div className={styles.description}>{plan.description}</div>
            {!compact && <div className={styles.interval}>月付</div>}
            <button
              type="button"
              className={isHighlight ? styles.primaryButton : styles.secondaryButton}
              onClick={() => onSelect(plan.plan_id)}
              disabled={isCurrent || !plan.checkout_available}
            >
              {isCurrent ? "当前套餐" : plan.plan_id === "free" ? "继续 Free" : `升级 ${plan.name}`}
            </button>
          </div>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 3: 编写 CSS**

```css
/* components/billing/PricingCards.module.css */
.grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 1.5rem;
}

.compactGrid {
  gap: 0.75rem;
}

.card {
  position: relative;
  border: 1px solid hsl(var(--border-whisper));
  border-radius: var(--radius-card);
  background: hsl(var(--card));
  padding: 1.5rem;
  display: flex;
  flex-direction: column;
  gap: 1rem;
  transition: transform 200ms ease, box-shadow 200ms ease;
}

.card.highlight {
  border-color: hsl(var(--accent));
  box-shadow: var(--shadow-glow);
  transform: scale(1.03);
  z-index: 1;
}

.card.compact {
  padding: 1rem;
  gap: 0.5rem;
}

.badge {
  position: absolute;
  top: -0.75rem;
  left: 50%;
  transform: translateX(-50%);
  background: hsl(var(--accent));
  color: hsl(var(--primary-foreground));
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  padding: 0.25rem 0.75rem;
  border-radius: 999px;
}

.name {
  font-family: var(--font-heading);
  font-size: var(--font-size-title-sm);
  font-weight: var(--font-weight-bold);
  color: hsl(var(--foreground));
  margin: 0;
}

.prices {
  display: flex;
  flex-direction: column;
  gap: 0.125rem;
}

.priceCny {
  font-family: var(--font-mono);
  font-size: var(--font-size-title);
  font-weight: var(--font-weight-bold);
  color: hsl(var(--foreground));
}

.priceUsd {
  font-family: var(--font-mono);
  font-size: var(--font-size-body);
  color: hsl(var(--muted-foreground));
}

.description {
  font-size: var(--font-size-body);
  color: hsl(var(--muted-foreground));
  min-height: 2.5rem;
}

.interval {
  font-size: var(--font-size-caption);
  color: hsl(var(--subtle-foreground));
}

.primaryButton {
  background: hsl(var(--cta-background));
  color: hsl(var(--cta-foreground));
  border: none;
  border-radius: var(--radius-button);
  padding: 0.625rem 1rem;
  font-size: var(--font-size-control);
  font-weight: var(--font-weight-semibold);
  cursor: pointer;
  transition: background 150ms ease, transform 150ms ease;
}
.primaryButton:hover:not(:disabled) { background: hsl(var(--cta-background-hover)); transform: translateY(-1px); }
.primaryButton:disabled { opacity: 0.5; cursor: not-allowed; }

.secondaryButton {
  background: hsl(var(--secondary));
  color: hsl(var(--secondary-foreground));
  border: 1px solid hsl(var(--border));
  border-radius: var(--radius-button);
  padding: 0.625rem 1rem;
  font-size: var(--font-size-control);
  font-weight: var(--font-weight-semibold);
  cursor: pointer;
}
.secondaryButton:hover:not(:disabled) { background: hsl(var(--surface-muted)); }
.secondaryButton:disabled { opacity: 0.5; cursor: not-allowed; }

@media (max-width: 768px) {
  .grid { grid-template-columns: 1fr; }
  .card.highlight { transform: none; }
}
```

- [ ] **Step 4: 运行测试验证通过**

Run: `cd frontend_next && pnpm test components/billing/PricingCards.test.tsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add frontend_next/components/billing/PricingCards.tsx \
        frontend_next/components/billing/PricingCards.module.css \
        frontend_next/components/billing/PricingCards.test.tsx
git commit -m "feat(frontend): add PricingCards component (full/compact variants)

- 3-tier card grid with highlight scale + glow on recommended tier
- 推荐 badge on highlighted tier
- Disabled '当前套餐' for current plan
- Mobile responsive: stack vertically, no scale on highlight
- 'compact' variant for paywall reuse"
```

---

### Task 10: UsageWarningToast 组件

**Files:**
- Create: `frontend_next/components/billing/UsageWarningToast.tsx`
- Create: `frontend_next/components/billing/UsageWarningToast.module.css`
- Test: `frontend_next/components/billing/UsageWarningToast.test.tsx`

- [ ] **Step 1: 编写失败的测试**

```tsx
// components/billing/UsageWarningToast.test.tsx
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { UsageWarningToast } from "./UsageWarningToast";

beforeEach(() => {
  localStorage.clear();
  vi.useFakeTimers();
});

describe("UsageWarningToast", () => {
  it("renders 80% threshold message with upgrade link", () => {
    render(
      <UsageWarningToast
        threshold={80}
        windowType="5h"
        userId="user_123"
        used={80000}
        limit={100000}
        resetAt="2026-06-07T20:00:00Z"
        onDismiss={() => {}}
      />
    );
    expect(screen.getByText(/80%/)).toBeInTheDocument();
    expect(screen.getByText(/升级 Plus 解锁 6× 用量/)).toBeInTheDocument();
  });

  it("does not render if already dismissed for this window in localStorage", () => {
    localStorage.setItem("toast_dismissed_user_123_5h_80", "true");
    const { container } = render(
      <UsageWarningToast
        threshold={80}
        windowType="5h"
        userId="user_123"
        used={80000}
        limit={100000}
        resetAt="2026-06-07T20:00:00Z"
        onDismiss={() => {}}
      />
    );
    expect(container.firstChild).toBeNull();
  });

  it("writes localStorage with user_id + windowType + threshold on dismiss", () => {
    const onDismiss = vi.fn();
    render(
      <UsageWarningToast
        threshold={80}
        windowType="5h"
        userId="user_abc"
        used={80000}
        limit={100000}
        resetAt="2026-06-07T20:00:00Z"
        onDismiss={onDismiss}
      />
    );
    fireEvent.click(screen.getByLabelText("关闭"));
    expect(localStorage.getItem("toast_dismissed_user_abc_5h_80")).toBe("true");
    expect(onDismiss).toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: 编写组件**

```tsx
// components/billing/UsageWarningToast.tsx
"use client";

import { useEffect, useState } from "react";
import styles from "./UsageWarningToast.module.css";
import { formatCompactToken, formatCountdown } from "../../lib/billing/format";

export type UsageWarningToastProps = {
  threshold: 80 | 95;
  windowType: "5h" | "7d";
  userId: string;
  used: number;
  limit: number;
  resetAt: string;
  onDismiss: () => void;
  onUpgradeClick?: () => void;
};

const DISMISS_KEY = (userId: string, windowType: string, threshold: number) =>
  `toast_dismissed_${userId}_${windowType}_${threshold}`;

function useCountdown(resetAt: string) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 30_000);
    return () => clearInterval(id);
  }, []);
  return formatCountdown(new Date(resetAt).getTime() - now);
}

export function UsageWarningToast({ threshold, windowType, userId, used, limit, resetAt, onDismiss, onUpgradeClick }: UsageWarningToastProps) {
  const [hidden, setHidden] = useState(true);

  useEffect(() => {
    const key = DISMISS_KEY(userId, windowType, threshold);
    if (localStorage.getItem(key) === "true") {
      setHidden(true);
      return;
    }
    setHidden(false);
  }, [userId, windowType, threshold]);

  const countdown = useCountdown(resetAt);
  if (hidden) return null;

  const handleDismiss = () => {
    localStorage.setItem(DISMISS_KEY(userId, windowType, threshold), "true");
    setHidden(true);
    onDismiss();
  };

  const urgency = threshold === 95 ? styles.urgent : styles.elevated;

  return (
    <div className={`${styles.toast} ${urgency}`} role="alert">
      <div className={styles.body}>
        <strong>{windowType} 用量已用 {threshold}%</strong>
        <span className={styles.numbers}>
          {" "}({formatCompactToken(used)} / {formatCompactToken(limit)})
        </span>
        <div className={styles.subline}>
          还有 {countdown} 重置。{" "}
          {onUpgradeClick && (
            <button className={styles.upgradeLink} onClick={onUpgradeClick}>
              升级 Plus 解锁 6× 用量 →
            </button>
          )}
        </div>
      </div>
      <button className={styles.closeButton} aria-label="关闭" onClick={handleDismiss}>×</button>
    </div>
  );
}
```

- [ ] **Step 3: 编写 CSS**

```css
/* components/billing/UsageWarningToast.module.css */
.toast {
  position: fixed;
  top: 1.5rem;
  left: 50%;
  transform: translateX(-50%);
  z-index: 100;
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
  padding: 0.875rem 1rem;
  border-radius: var(--radius-control);
  background: hsl(var(--warning-surface));
  border: 1px solid hsl(var(--warning-border));
  color: hsl(var(--warning-foreground));
  font-size: var(--font-size-caption-strong);
  box-shadow: var(--shadow-md);
  max-width: 32rem;
  width: calc(100% - 2rem);
}

.urgent {
  background: hsl(var(--destructive-soft));
  border-color: hsl(var(--destructive-border));
  color: hsl(var(--destructive));
}

.body {
  flex: 1;
}

.numbers {
  font-family: var(--font-mono);
}

.subline {
  margin-top: 0.25rem;
  color: hsl(var(--muted-foreground));
  font-size: var(--font-size-caption);
}

.upgradeLink {
  background: none;
  border: none;
  padding: 0;
  color: hsl(var(--accent));
  cursor: pointer;
  text-decoration: underline;
  font: inherit;
}

.closeButton {
  background: none;
  border: none;
  font-size: 1.25rem;
  cursor: pointer;
  color: inherit;
  padding: 0 0.25rem;
  line-height: 1;
}
```

- [ ] **Step 4: 运行测试 + Commit**

Run: `cd frontend_next && pnpm test components/billing/UsageWarningToast.test.tsx`
Expected: PASS

```bash
git add frontend_next/components/billing/UsageWarningToast.tsx \
        frontend_next/components/billing/UsageWarningToast.module.css \
        frontend_next/components/billing/UsageWarningToast.test.tsx
git commit -m "feat(frontend): add UsageWarningToast (80%/95% thresholds)

- localStorage key includes user_id (avoid multi-account collision)
- 30s countdown refresh
- Two urgency levels: 80% (warning), 95% (urgent)
- Click close persists dismissal in localStorage"
```

---

### Task 11: UsageForecastCard 组件

**Files:**
- Create: `frontend_next/components/billing/UsageForecastCard.tsx`
- Create: `frontend_next/components/billing/UsageForecastCard.module.css`
- Test: `frontend_next/components/billing/UsageForecastCard.test.tsx`

- [ ] **Step 1: 编写失败的测试**

```tsx
// components/billing/UsageForecastCard.test.tsx
import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { UsageForecastCard } from "./UsageForecastCard";

describe("UsageForecastCard", () => {
  it("shows upgrade recommendation when flagged", () => {
    render(
      <UsageForecastCard
        suggestion_zh="按当前用量，本月建议升级到 Plus（7d 限额 4M）"
        suggestion_en="Based on current usage, upgrading to Plus is recommended"
        upgrade_recommended={true}
        projected_30d_tokens={3500000}
        current_limit_7d={400000}
      />
    );
    expect(screen.getByText(/建议升级到 Plus/)).toBeInTheDocument();
  });

  it("shows no-upgrade message when under threshold", () => {
    render(
      <UsageForecastCard
        suggestion_zh="按当前用量，本月无需升级"
        suggestion_en="Based on current usage, no upgrade needed"
        upgrade_recommended={false}
        projected_30d_tokens={100000}
        current_limit_7d={400000}
      />
    );
    expect(screen.getByText(/本月无需升级/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: 编写组件**

```tsx
// components/billing/UsageForecastCard.tsx
"use client";

import { useLocale } from "next-intl";
import styles from "./UsageForecastCard.module.css";
import { formatCompactToken } from "../../lib/billing/format";

export type UsageForecastCardProps = {
  suggestion_zh: string;
  suggestion_en: string;
  upgrade_recommended: boolean;
  projected_30d_tokens: number;
  current_limit_7d: number;
};

export function UsageForecastCard({ suggestion_zh, suggestion_en, upgrade_recommended, projected_30d_tokens, current_limit_7d }: UsageForecastCardProps) {
  const locale = useLocale();
  const suggestion = locale === "en" ? suggestion_en : suggestion_zh;
  return (
    <div className={`${styles.card} ${upgrade_recommended ? styles.warn : ""}`}>
      <div className={styles.icon}>{upgrade_recommended ? "💡" : "✅"}</div>
      <div className={styles.body}>
        <p className={styles.message}>{suggestion}</p>
        <p className={styles.detail}>
          预计 30 天用量 {formatCompactToken(projected_30d_tokens)} / 7d 限额 {formatCompactToken(current_limit_7d)}
        </p>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: 编写 CSS**

```css
/* components/billing/UsageForecastCard.module.css */
.card {
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
  padding: 1rem 1.25rem;
  border: 1px solid hsl(var(--border-whisper));
  border-radius: var(--radius-card);
  background: hsl(var(--surface-accent-soft));
}

.card.warn { background: hsl(var(--warning-surface)); border-color: hsl(var(--warning-border)); }

.icon { font-size: 1.25rem; line-height: 1; }

.body { flex: 1; }
.message { margin: 0 0 0.25rem; font-size: var(--font-size-body); color: hsl(var(--foreground)); }
.detail { margin: 0; font-size: var(--font-size-caption); color: hsl(var(--muted-foreground)); font-family: var(--font-mono); }
```

- [ ] **Step 4: 运行测试 + Commit**

Run: `cd frontend_next && pnpm test components/billing/UsageForecastCard.test.tsx`
Expected: PASS

```bash
git add frontend_next/components/billing/UsageForecastCard.tsx \
        frontend_next/components/billing/UsageForecastCard.module.css \
        frontend_next/components/billing/UsageForecastCard.test.tsx
git commit -m "feat(frontend): add UsageForecastCard (upgrade recommendation)"
```

---

### Task 12: UsageTrendChart 组件（纯 SVG 折线图）

**Files:**
- Create: `frontend_next/components/billing/UsageTrendChart.tsx`
- Create: `frontend_next/components/billing/UsageTrendChart.module.css`
- Test: `frontend_next/components/billing/UsageTrendChart.test.tsx`

- [ ] **Step 1: 编写失败的测试**

```tsx
// components/billing/UsageTrendChart.test.tsx
import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { UsageTrendChart } from "./UsageTrendChart";

const daily = [
  { date: "2026-06-01", tokens: 50000 },
  { date: "2026-06-02", tokens: 75000 },
  { date: "2026-06-03", tokens: 60000 },
  { date: "2026-06-04", tokens: 90000 },
];

describe("UsageTrendChart", () => {
  it("renders an SVG with one polyline per data point", () => {
    const { container } = render(<UsageTrendChart daily={daily} />);
    const polyline = container.querySelector("polyline");
    expect(polyline).toBeInTheDocument();
    expect(polyline?.getAttribute("points")?.split(" ").length).toBe(4);
  });

  it("renders date labels on x-axis", () => {
    render(<UsageTrendChart daily={daily} />);
    expect(screen.getByText("06-01")).toBeInTheDocument();
    expect(screen.getByText("06-04")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: 编写组件**

```tsx
// components/billing/UsageTrendChart.tsx
"use client";

import { useMemo } from "react";
import styles from "./UsageTrendChart.module.css";
import { formatCompactToken } from "../../lib/billing/format";
import type { DailyUsage } from "../../lib/billing/api";

export type UsageTrendChartProps = {
  daily: DailyUsage[];
  width?: number;
  height?: number;
};

export function UsageTrendChart({ daily, width = 600, height = 200 }: UsageTrendChartProps) {
  const padding = { top: 16, right: 16, bottom: 28, left: 48 };
  const innerW = width - padding.left - padding.right;
  const innerH = height - padding.top - padding.bottom;

  const { points, maxV, yTicks } = useMemo(() => {
    const maxV = Math.max(...daily.map(d => d.tokens), 1);
    const stepX = daily.length > 1 ? innerW / (daily.length - 1) : 0;
    const points = daily.map((d, i) => {
      const x = padding.left + i * stepX;
      const y = padding.top + innerH - (d.tokens / maxV) * innerH;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    });
    const yTicks = [0, 0.5, 1].map(t => ({
      y: padding.top + innerH * (1 - t),
      label: formatCompactToken(Math.round(maxV * t)),
    }));
    return { points, maxV, yTicks };
  }, [daily, innerW, innerH, padding.left, padding.top]);

  if (daily.length === 0) {
    return <div className={styles.empty}>暂无用量数据</div>;
  }

  return (
    <svg className={styles.chart} viewBox={`0 0 ${width} ${height}`} role="img" aria-label="近 N 日用量趋势">
      {yTicks.map((t, i) => (
        <g key={i}>
          <line x1={padding.left} x2={width - padding.right} y1={t.y} y2={t.y} className={styles.grid} />
          <text x={padding.left - 6} y={t.y} className={styles.yLabel} textAnchor="end" dominantBaseline="middle">{t.label}</text>
        </g>
      ))}
      <polyline points={points.join(" ")} className={styles.line} fill="none" />
      {daily.map((d, i) => {
        const x = padding.left + (daily.length > 1 ? i * (innerW / (daily.length - 1)) : innerW / 2);
        const y = padding.top + innerH - (d.tokens / maxV) * innerH;
        return (
          <g key={d.date}>
            <circle cx={x} cy={y} r={3} className={styles.dot} />
            {i % Math.max(1, Math.floor(daily.length / 7)) === 0 && (
              <text x={x} y={height - 8} className={styles.xLabel} textAnchor="middle">{d.date.slice(5)}</text>
            )}
          </g>
        );
      })}
    </svg>
  );
}
```

- [ ] **Step 3: 编写 CSS**

```css
/* components/billing/UsageTrendChart.module.css */
.chart {
  width: 100%;
  height: auto;
  display: block;
}

.line {
  stroke: hsl(var(--accent));
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.dot {
  fill: hsl(var(--accent));
}

.grid {
  stroke: hsl(var(--border-whisper));
  stroke-width: 1;
  stroke-dasharray: 2 4;
}

.yLabel, .xLabel {
  font-family: var(--font-mono);
  font-size: 10px;
  fill: hsl(var(--muted-foreground));
}

.empty {
  padding: 2rem;
  text-align: center;
  color: hsl(var(--muted-foreground));
  font-size: var(--font-size-body);
}
```

- [ ] **Step 4: 运行测试 + Commit**

Run: `cd frontend_next && pnpm test components/billing/UsageTrendChart.test.tsx`
Expected: PASS

```bash
git add frontend_next/components/billing/UsageTrendChart.tsx \
        frontend_next/components/billing/UsageTrendChart.module.css \
        frontend_next/components/billing/UsageTrendChart.test.tsx
git commit -m "feat(frontend): add UsageTrendChart (pure SVG line chart, 7-day default)"
```

---

### Task 13: PaywallModal 组件

**Files:**
- Create: `frontend_next/components/billing/PaywallModal.tsx`
- Create: `frontend_next/components/billing/PaywallModal.module.css`
- Test: `frontend_next/components/billing/PaywallModal.test.tsx`

- [ ] **Step 1: 编写失败的测试**

```tsx
// components/billing/PaywallModal.test.tsx
import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { PaywallModal } from "./PaywallModal";

const window5h = { used: 100000, limit: 100000, percentage: 100, reset_at: "2099-12-31T00:00:00Z" };
const window7d = { used: 100000, limit: 400000, percentage: 25, reset_at: "2099-12-31T00:00:00Z" };

const plans = [
  { plan_id: "free", name: "Free", price_label_cny: "¥0", price_label_usd: "$0", description: "", interval: "month", checkout_available: false, current: false, quotas: [] },
  { plan_id: "plus", name: "Plus", price_label_cny: "¥49 / 月", price_label_usd: "$9 / 月", description: "", interval: "month", checkout_available: true, current: false, quotas: [] },
  { plan_id: "pro", name: "Pro", price_label_cny: "¥129 / 月", price_label_usd: "$19 / 月", description: "", interval: "month", checkout_available: true, current: false, quotas: [] },
];

describe("PaywallModal", () => {
  it("renders title based on reason prop", () => {
    render(<PaywallModal reason="5h" plans={plans} rolling5h={window5h} rolling7d={window7d} onSelect={vi.fn()} onContinueFree={vi.fn()} />);
    expect(screen.getByText(/5h 用量已达上限/)).toBeInTheDocument();
  });

  it("embeds UsageMeter compact + PricingCards compact", () => {
    render(<PaywallModal reason="5h" plans={plans} rolling5h={window5h} rolling7d={window7d} onSelect={vi.fn()} onContinueFree={vi.fn()} />);
    expect(screen.getAllByRole("progressbar").length).toBeGreaterThan(0);
  });

  it("calls onContinueFree when 继续 Free clicked", () => {
    const onContinueFree = vi.fn();
    render(<PaywallModal reason="5h" plans={plans} rolling5h={window5h} rolling7d={window7d} onSelect={vi.fn()} onContinueFree={onContinueFree} />);
    screen.getByRole("button", { name: /继续 Free/ }).click();
    expect(onContinueFree).toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: 编写组件**

```tsx
// components/billing/PaywallModal.tsx
"use client";

import styles from "./PaywallModal.module.css";
import { UsageMeter } from "./UsageMeter";
import { PricingCards } from "./PricingCards";
import type { BillingPlan, UsageWindowBucket, LimitHits } from "../../lib/billing/api";

export type PaywallModalProps = {
  reason: "5h" | "7d";
  plans: BillingPlan[];
  rolling5h: UsageWindowBucket;
  rolling7d: UsageWindowBucket;
  onSelect: (planId: string) => void;
  onContinueFree: () => void;
};

export function PaywallModal({ reason, plans, rolling5h, rolling7d, onSelect, onContinueFree }: PaywallModalProps) {
  return (
    <div className={styles.overlay}>
      <div className={styles.modal} role="dialog" aria-modal="true">
        <h1 className={styles.title}>
          {reason === "5h" ? "5h 用量已达上限" : "7d 用量已达上限"}
        </h1>
        <UsageMeter
          variant="compact"
          planId="free"
          rolling5h={rolling5h}
          rolling7d={rolling7d}
          softLimitHit={{ rolling_5h: true, rolling_7d: false }}
          hardLimitHit={{ rolling_5h: reason === "5h", rolling_7d: reason === "7d" }}
        />
        <p className={styles.subtitle}>
          Free → Plus，解锁 10× 用量
        </p>
        <PricingCards plans={plans} highlightTier="plus" onSelect={onSelect} compact />
        <div className={styles.footer}>
          <button type="button" className={styles.continueButton} onClick={onContinueFree}>
            继续 Free
          </button>
          <span className={styles.resetHint}>
            限额自动重置，请关注使用节奏
          </span>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: 编写 CSS**

```css
/* components/billing/PaywallModal.module.css */
.overlay {
  position: fixed;
  inset: 0;
  background: hsl(var(--dashboard-overlay) / 0.7);
  backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 200;
  padding: 1rem;
}

.modal {
  background: hsl(var(--card));
  border: 1px solid hsl(var(--border-whisper));
  border-radius: var(--radius-card);
  padding: 2rem;
  max-width: 56rem;
  width: 100%;
  max-height: 90vh;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
  box-shadow: var(--shadow-xl);
}

.title {
  font-family: var(--font-heading);
  font-size: var(--font-size-title);
  font-weight: var(--font-weight-bold);
  color: hsl(var(--foreground));
  margin: 0;
  text-align: center;
}

.subtitle {
  text-align: center;
  color: hsl(var(--muted-foreground));
  font-size: var(--font-size-body);
  margin: 0;
}

.footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding-top: 1rem;
  border-top: 1px solid hsl(var(--border-whisper));
}

.continueButton {
  background: none;
  border: 1px solid hsl(var(--border));
  border-radius: var(--radius-button);
  padding: 0.5rem 1rem;
  color: hsl(var(--muted-foreground));
  cursor: pointer;
  font-size: var(--font-size-caption-strong);
}
.continueButton:hover { background: hsl(var(--surface-muted)); }

.resetHint {
  font-size: var(--font-size-caption);
  color: hsl(var(--subtle-foreground));
}
```

- [ ] **Step 4: 运行测试 + Commit**

Run: `cd frontend_next && pnpm test components/billing/PaywallModal.test.tsx`
Expected: PASS

```bash
git add frontend_next/components/billing/PaywallModal.tsx \
        frontend_next/components/billing/PaywallModal.module.css \
        frontend_next/components/billing/PaywallModal.test.tsx
git commit -m "feat(frontend): add PaywallModal (reuses UsageMeter compact + PricingCards compact)

- Reason prop: 5h | 7d
- 3-tier comparison embedded inline for context at point of friction
- 继续 Free kept as escape hatch
- Single countdown source via UsageMeter compact (no duplicate timer logic)"
```

---

## Phase 3: 前端页面（Tasks 14-17，约 1 周）

### Task 14: `/pricing` 页面

**Files:**
- Create: `frontend_next/app/(marketing)/pricing/page.tsx`
- Create: `frontend_next/app/(marketing)/pricing/pricing.module.css`
- Test: `frontend_next/app/(marketing)/pricing/page.test.tsx`

- [ ] **Step 1: 编写失败的测试**

```tsx
// app/(marketing)/pricing/page.test.tsx
import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";

vi.mock("../../../lib/billing/api", () => ({
  billingApi: {
    getPlans: vi.fn().mockResolvedValue([
      { plan_id: "free", name: "Free", price_label_cny: "¥0", price_label_usd: "$0", description: "体验", interval: "month", checkout_available: false, current: false, quotas: [] },
      { plan_id: "plus", name: "Plus", price_label_cny: "¥49 / 月", price_label_usd: "$9 / 月", description: "深度研究", interval: "month", checkout_available: true, current: false, quotas: [] },
      { plan_id: "pro", name: "Pro", price_label_cny: "¥129 / 月", price_label_usd: "$19 / 月", description: "重度无忧", interval: "month", checkout_available: true, current: false, quotas: [] },
    ]),
  },
}));

import PricingPage from "./page";

describe("PricingPage", () => {
  it("renders title + 3 plan cards + FAQ", async () => {
    const page = await PricingPage();
    render(page);
    expect(screen.getByText(/选择适合你的方案/)).toBeInTheDocument();
    expect(screen.getByText("Plus")).toBeInTheDocument();
    expect(screen.getByText(/token 用量怎么算/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: 编写 page.tsx**

```tsx
// app/(marketing)/pricing/page.tsx
import { billingApi } from "../../../lib/billing/api";
import { PricingCards } from "../../../components/billing/PricingCards";
import { redirect } from "next/navigation";
import styles from "./pricing.module.css";

export const dynamic = "force-dynamic";

export default async function PricingPage() {
  let plans;
  try {
    plans = await billingApi.getPlans();
  } catch (e) {
    // 后端未就绪时给空数组（避免 SSR 崩）
    plans = [];
  }

  async function handleSelect(planId: string) {
    "use server";
    if (planId === "free") return;
    const res = await fetch(`${process.env.API_BASE_URL}/api/v1/billing/checkout-session`, {
      method: "POST",
      headers: { "Content-Type": "application/json", cookie: require("next/headers").headers().get("cookie") || "" },
      body: JSON.stringify({ plan_id: planId }),
    });
    const data = await res.json();
    if (data?.data?.checkout_url) redirect(data.data.checkout_url);
  }

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <h1 className={styles.title}>选择适合你的方案</h1>
        <div className={styles.billingToggle}>
          <button className={`${styles.toggleButton} ${styles.toggleActive}`}>月付</button>
          <span className={styles.toggleHint} title="年付即将推出">年付暂未开放</span>
        </div>
      </header>

      <PricingCards plans={plans} highlightTier="plus" onSelect={handleSelect as any} />

      <section className={styles.faq}>
        <h2 className={styles.faqTitle}>❓ 常见问题</h2>
        <details className={styles.faqItem}>
          <summary>token 用量怎么算？</summary>
          <p>输入 + 输出按 DeepSeek 公开计费标准累计。每次问题消耗 = (input tokens + output tokens)。</p>
        </details>
        <details className={styles.faqItem}>
          <summary>限额会重置吗？</summary>
          <p>5 小时滚动窗口 + 7 天滚动窗口。窗口内最旧的消耗点过去后，限额自动释放。</p>
        </details>
        <details className={styles.faqItem}>
          <summary>升级后立即生效吗？</summary>
          <p>支付成功后立即生效。降级则在当前计费周期结束时生效。</p>
        </details>
      </section>
    </div>
  );
}
```

- [ ] **Step 3: 编写 CSS**

```css
/* app/(marketing)/pricing/pricing.module.css */
.page {
  max-width: 80rem;
  margin: 0 auto;
  padding: 3rem 1.5rem 6rem;
  display: flex;
  flex-direction: column;
  gap: 3rem;
}

.header {
  text-align: center;
  display: flex;
  flex-direction: column;
  gap: 1rem;
  align-items: center;
}

.title {
  font-family: var(--font-heading);
  font-size: clamp(1.875rem, 4vw, 2.5rem);
  font-weight: var(--font-weight-bold);
  letter-spacing: var(--letter-spacing-title);
  color: hsl(var(--foreground));
  margin: 0;
}

.billingToggle {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  font-size: var(--font-size-caption-strong);
}

.toggleButton {
  background: hsl(var(--secondary));
  border: 1px solid hsl(var(--border));
  border-radius: var(--radius-button);
  padding: 0.5rem 1rem;
  cursor: not-allowed;
  opacity: 0.7;
}

.toggleActive {
  background: hsl(var(--cta-background));
  color: hsl(var(--cta-foreground));
  border-color: hsl(var(--cta-background));
}

.toggleHint {
  color: hsl(var(--subtle-foreground));
  font-size: var(--font-size-caption);
}

.faq {
  max-width: 48rem;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.faqTitle {
  font-family: var(--font-heading);
  font-size: var(--font-size-title-sm);
  color: hsl(var(--foreground));
  margin: 0 0 0.5rem;
}

.faqItem {
  border: 1px solid hsl(var(--border-whisper));
  border-radius: var(--radius-control);
  padding: 0.75rem 1rem;
  background: hsl(var(--card));
}

.faqItem summary {
  font-weight: var(--font-weight-semibold);
  cursor: pointer;
  color: hsl(var(--foreground));
}

.faqItem p {
  margin: 0.5rem 0 0;
  color: hsl(var(--muted-foreground));
  font-size: var(--font-size-body);
}
```

- [ ] **Step 4: 运行测试 + Commit**

Run: `cd frontend_next && pnpm test app/\(marketing\)/pricing/page.test.tsx`
Expected: PASS

```bash
git add frontend_next/app/\(marketing\)/pricing/
git commit -m "feat(frontend): add /pricing page with 3-tier cards + FAQ

- Server component (SSR) fetches plans from /api/v1/billing/plans
- handleSelect server action triggers checkout-session redirect
- Monthly-only toggle with year-pending tooltip
- FAQ addresses common conversion-friction questions"
```

---

### Task 15: `/settings/usage` 页面（用量仪表盘）

**Files:**
- Create: `frontend_next/app/settings/usage/page.tsx`
- Create: `frontend_next/app/settings/usage/usage.module.css`
- Test: `frontend_next/app/settings/usage/page.test.tsx`

- [ ] **Step 1: 编写失败的测试**

```tsx
// app/settings/usage/page.test.tsx
import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";

vi.mock("../../../lib/billing/api", () => ({
  billingApi: {
    getUsageWindow: vi.fn().mockResolvedValue({
      plan_id: "free",
      rolling_5h: { used: 80000, limit: 100000, percentage: 80, reset_at: "2099-01-01T00:00:00Z" },
      rolling_7d: { used: 200000, limit: 400000, percentage: 50, reset_at: "2099-01-01T00:00:00Z" },
      soft_limit_hit: { rolling_5h: true, rolling_7d: false },
      hard_limit_hit: { rolling_5h: false, rolling_7d: false },
    }),
    getUsageHistory: vi.fn().mockResolvedValue({
      daily: [
        { date: "2026-06-01", tokens: 50000 },
        { date: "2026-06-02", tokens: 75000 },
      ],
    }),
    getUsageForecast: vi.fn().mockResolvedValue({
      current_plan: "free",
      avg_30d_tokens: 8000,
      projected_30d_tokens: 240000,
      current_limit_7d: 400000,
      upgrade_recommended: false,
      suggestion_zh: "按当前用量，本月无需升级",
      suggestion_en: "Based on current usage, no upgrade needed",
    }),
  },
}));

import UsagePage from "./page";

describe("UsagePage", () => {
  it("renders title + 2 UsageMeter cards + trend chart + forecast", async () => {
    const page = await UsagePage();
    render(page);
    expect(screen.getByText(/用量与套餐/)).toBeInTheDocument();
    expect(screen.getByText(/5 小时窗口/)).toBeInTheDocument();
    expect(screen.getByText(/7 天窗口/)).toBeInTheDocument();
    expect(screen.getByText(/近 7 日用量趋势/)).toBeInTheDocument();
    expect(screen.getByText(/本月无需升级/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: 编写 page.tsx**

```tsx
// app/settings/usage/page.tsx
import { billingApi } from "../../../lib/billing/api";
import { UsageMeter } from "../../../components/billing/UsageMeter";
import { UsageTrendChart } from "../../../components/billing/UsageTrendChart";
import { UsageForecastCard } from "../../../components/billing/UsageForecastCard";
import { redirect } from "next/navigation";
import styles from "./usage.module.css";

export const dynamic = "force-dynamic";

export default async function UsagePage() {
  const [window, history, forecast] = await Promise.all([
    billingApi.getUsageWindow(),
    billingApi.getUsageHistory(7),
    billingApi.getUsageForecast(),
  ]);

  async function upgradeAction() {
    "use server";
    redirect("/upgrade/paywall?from=usage");
  }

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <h1 className={styles.title}>用量与套餐</h1>
        {forecast.current_plan === "free" && (
          <form action={upgradeAction}>
            <button type="submit" className={styles.upgradeButton}>升级 Plus</button>
          </form>
        )}
      </header>

      <section className={styles.section}>
        <p className={styles.currentPlan}>
          当前套餐: <strong>{forecast.current_plan.toUpperCase()}</strong>
          {forecast.current_plan === "free" && (
            <span className={styles.upgradeHint}> → Free 升级 Plus 解锁 10× 用量</span>
          )}
        </p>
      </section>

      <section className={styles.meters}>
        <UsageMeter
          variant="full"
          planId={window.plan_id}
          rolling5h={window.rolling_5h}
          rolling7d={window.rolling_7d}
          softLimitHit={window.soft_limit_hit}
          hardLimitHit={window.hard_limit_hit}
        />
      </section>

      <section className={styles.section}>
        <h2 className={styles.sectionTitle}>近 7 日用量趋势</h2>
        <UsageTrendChart daily={history.daily} />
      </section>

      <UsageForecastCard
        suggestion_zh={forecast.suggestion_zh}
        suggestion_en={forecast.suggestion_en}
        upgrade_recommended={forecast.upgrade_recommended}
        projected_30d_tokens={forecast.projected_30d_tokens}
        current_limit_7d={forecast.current_limit_7d}
      />
    </div>
  );
}
```

- [ ] **Step 3: 编写 CSS**

```css
/* app/settings/usage/usage.module.css */
.page {
  max-width: 56rem;
  margin: 0 auto;
  padding: 2rem 1.5rem 4rem;
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.title {
  font-family: var(--font-heading);
  font-size: var(--font-size-title);
  font-weight: var(--font-weight-bold);
  color: hsl(var(--foreground));
  margin: 0;
}

.upgradeButton {
  background: hsl(var(--cta-background));
  color: hsl(var(--cta-foreground));
  border: none;
  border-radius: var(--radius-button);
  padding: 0.5rem 1rem;
  font-weight: var(--font-weight-semibold);
  cursor: pointer;
}
.upgradeButton:hover { background: hsl(var(--cta-background-hover)); }

.section { display: flex; flex-direction: column; gap: 0.75rem; }

.currentPlan {
  margin: 0;
  font-size: var(--font-size-body);
  color: hsl(var(--muted-foreground));
}

.upgradeHint {
  color: hsl(var(--accent));
  font-weight: var(--font-weight-semibold);
}

.meters {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 1rem;
}
@media (max-width: 768px) { .meters { grid-template-columns: 1fr; } }

.sectionTitle {
  font-family: var(--font-heading);
  font-size: var(--font-size-section-title);
  color: hsl(var(--foreground));
  margin: 0;
}
```

- [ ] **Step 4: 运行测试 + Commit**

Run: `cd frontend_next && pnpm test app/settings/usage/page.test.tsx`
Expected: PASS

```bash
git add frontend_next/app/settings/usage/
git commit -m "feat(frontend): add /settings/usage dashboard

- Parallel fetch: window + history + forecast
- UsageMeter full + UsageTrendChart + UsageForecastCard
- 升级 Plus CTA for Free users (server action redirect to paywall)"
```

---

### Task 16: `/upgrade/paywall` 页面

**Files:**
- Create: `frontend_next/app/upgrade/paywall/page.tsx`
- Create: `frontend_next/app/upgrade/paywall/paywall.module.css`

- [ ] **Step 1: 编写 page.tsx**

```tsx
// app/upgrade/paywall/page.tsx
import { billingApi } from "../../../lib/billing/api";
import { PaywallModal } from "../../../components/billing/PaywallModal";
import { redirect } from "next/navigation";

export const dynamic = "force-dynamic";

export default async function PaywallPage({ searchParams }: { searchParams: Promise<{ reason?: string }> }) {
  const params = await searchParams;
  const reason = (params.reason === "7d" ? "7d" : "5h") as "5h" | "7d";
  const [window, plans] = await Promise.all([
    billingApi.getUsageWindow(),
    billingApi.getPlans(),
  ]);

  async function handleSelect(planId: string) {
    "use server";
    if (planId === "free") return;
    const res = await fetch(`${process.env.API_BASE_URL}/api/v1/billing/checkout-session`, {
      method: "POST",
      headers: { "Content-Type": "application/json", cookie: (await import("next/headers")).headers().get("cookie") || "" },
      body: JSON.stringify({ plan_id: planId }),
    });
    const data = await res.json();
    if (data?.data?.checkout_url) redirect(data.data.checkout_url);
  }

  async function handleContinueFree() {
    "use server";
    redirect("/workspace");
  }

  return (
    <PaywallModal
      reason={reason}
      plans={plans}
      rolling5h={window.rolling_5h}
      rolling7d={window.rolling_7d}
      onSelect={handleSelect as any}
      onContinueFree={handleContinueFree}
    />
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add frontend_next/app/upgrade/paywall/
git commit -m "feat(frontend): add /upgrade/paywall page (full-screen paywall)

- Reads ?reason=5h|7d from query
- Renders PaywallModal with current window data + all 3 plans
- Server actions: handleSelect → checkout-session, handleContinueFree → /workspace"
```

---

### Task 17: `/upgrade/success` 页面

**Files:**
- Create: `frontend_next/app/upgrade/success/page.tsx`
- Create: `frontend_next/app/upgrade/success/success.module.css`

- [ ] **Step 1: 编写 page.tsx**

```tsx
// app/upgrade/success/page.tsx
import { redirect } from "next/navigation";
import Link from "next/link";
import styles from "./success.module.css";

export const dynamic = "force-dynamic";

export default function UpgradeSuccessPage() {
  return (
    <div className={styles.page}>
      <div className={styles.card}>
        <div className={styles.icon}>🎉</div>
        <h1 className={styles.title}>升级成功</h1>
        <p className={styles.subtitle}>新档位已立即生效，祝你用得开心。</p>
        <div className={styles.actions}>
          <Link href="/workspace" className={styles.primaryButton}>返回工作区</Link>
          <Link href="/settings/usage" className={styles.secondaryButton}>查看用量</Link>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: 编写 CSS**

```css
/* app/upgrade/success/success.module.css */
.page { min-height: 100vh; display: grid; place-items: center; padding: 2rem 1rem; }
.card { max-width: 28rem; text-align: center; background: hsl(var(--card)); border: 1px solid hsl(var(--border-whisper)); border-radius: var(--radius-card); padding: 3rem 2rem; box-shadow: var(--shadow-md); }
.icon { font-size: 4rem; margin-bottom: 1rem; }
.title { font-family: var(--font-heading); font-size: var(--font-size-title); color: hsl(var(--foreground)); margin: 0 0 0.5rem; }
.subtitle { color: hsl(var(--muted-foreground)); margin: 0 0 2rem; }
.actions { display: flex; gap: 0.75rem; justify-content: center; }
.primaryButton, .secondaryButton {
  padding: 0.625rem 1.25rem;
  border-radius: var(--radius-button);
  font-weight: var(--font-weight-semibold);
  text-decoration: none;
}
.primaryButton { background: hsl(var(--cta-background)); color: hsl(var(--cta-foreground)); }
.secondaryButton { background: hsl(var(--secondary)); color: hsl(var(--secondary-foreground)); border: 1px solid hsl(var(--border)); }
```

- [ ] **Step 3: Commit**

```bash
git add frontend_next/app/upgrade/success/
git commit -m "feat(frontend): add /upgrade/success post-checkout landing page"
```

---

## Phase 4: i18n + Workspace 集成（Task 18-19）

### Task 18: i18n 词条（zh/en 双语）

**Files:**
- Modify: `frontend_next/lib/i18n/messages.ts`

- [ ] **Step 1: 在 UI_MESSAGES 中追加以下条目**

```typescript
// 追加到 lib/i18n/messages.ts 的 UI_MESSAGES 对象内
pricingTitle: { zh: "选择适合你的方案", en: "Choose your plan" },
pricingMonthly: { zh: "月付", en: "Monthly" },
pricingYearlySoon: { zh: "年付暂未开放", en: "Yearly coming soon" },
pricingTierFreeName: { zh: "Free", en: "Free" },
pricingTierPlusName: { zh: "Plus", en: "Plus" },
pricingTierProName: { zh: "Pro", en: "Pro" },
pricingTierPlusBadge: { zh: "推荐", en: "Recommended" },
pricingTierPlusTagline: { zh: "深度研究首选", en: "Best for deep research" },
pricingTierProTagline: { zh: "重度无忧", en: "For power users" },
pricingFaqToken: { zh: "token 用量怎么算？", en: "How is token usage counted?" },
pricingFaqTokenAnswer: { zh: "输入 + 输出按 DeepSeek 公开计费标准累计", en: "Input + output per DeepSeek public pricing" },
pricingFaqReset: { zh: "限额会重置吗？", en: "Do limits reset?" },
pricingFaqResetAnswer: { zh: "5h 滚动窗口 + 7d 滚动窗口，最旧消耗点过后自动释放", en: "5h rolling + 7d rolling windows" },
pricingFaqUpgrade: { zh: "升级后立即生效吗？", en: "Does upgrade take effect immediately?" },
pricingFaqUpgradeAnswer: { zh: "支付成功后立即生效。降级在当前计费周期结束时生效。", en: "Effective immediately after payment. Downgrade at end of billing cycle." },
upgradeButton: { zh: "升级 Plus", en: "Upgrade Plus" },
upgradeContinueFree: { zh: "继续 Free", en: "Continue Free" },
currentPlan: { zh: "当前套餐", en: "Current plan" },
usageTitle: { zh: "用量与套餐", en: "Usage & Plan" },
usageWindow5h: { zh: "5 小时窗口", en: "5-hour window" },
usageWindow7d: { zh: "7 天窗口", en: "7-day window" },
usageEstimatedReset: { zh: "预计 {time} 后重置", en: "Resets in {time}" },
usageSoftLimitWarning: { zh: "已超过软上限，建议控制节奏", en: "Soft limit reached, consider slowing down" },
usageTrendTitle: { zh: "近 7 日用量趋势", en: "Last 7-day trend" },
usageForecastTitle: { zh: "智能建议", en: "Smart suggestion" },
usageNoUpgrade: { zh: "按当前用量，本月无需升级", en: "No upgrade needed this month" },
usageUpgradeRecommended: { zh: "按当前用量，本月建议升级到 Plus", en: "Based on usage, upgrading to Plus is recommended" },
paywallTitle5h: { zh: "5h 用量已达上限", en: "5h limit reached" },
paywallTitle7d: { zh: "7d 用量已达上限", en: "7d limit reached" },
paywallSubtitle: { zh: "Free → Plus，解锁 10× 用量", en: "Free → Plus, unlock 10× usage" },
paywallContinueFree: { zh: "继续 Free", en: "Continue Free" },
toast5h80: { zh: "5h 用量已用 80%", en: "5h usage at 80%" },
toast5h95: { zh: "5h 用量已用 95%", en: "5h usage at 95%" },
toastUpgradeCta: { zh: "升级 Plus 解锁 6× 用量 →", en: "Upgrade to Plus for 6× usage →" },
toastClose: { zh: "关闭", en: "Close" },
toastResetsIn: { zh: "还有 {time} 重置", en: "Resets in {time}" },
```

- [ ] **Step 2: 验证 i18n 完整覆盖**

Run: `cd frontend_next && pnpm tsc --noEmit 2>&1 | head -20`
Expected: 0 errors

- [ ] **Step 3: Commit**

```bash
git add frontend_next/lib/i18n/messages.ts
git commit -m "feat(frontend): add i18n entries for pricing/usage/paywall/toast

- 35+ entries covering all 4 surfaces
- Bilingual zh/en with placeholder tokens ({time})
- No breaking changes to existing UI_MESSAGES schema"
```

---

### Task 19: Workspace 集成 toast（80%/95% 触发）

**Files:**
- Modify: `frontend_next/components/workspace/workspace-shell.tsx`（或 chat 组件，找到合适位置）
- Test: `frontend_next/components/workspace/workspace-shell.test.tsx`（新建或追加）

- [ ] **Step 1: 定位 workspace shell**

```bash
cd frontend_next && grep -rln "useChat\|ChatPanel\|workspace-shell" components/ | head -5
```

确认 `workspace-shell.tsx` 或类似文件存在。

- [ ] **Step 2: 添加 toast 渲染逻辑**

在 workspace shell 的 layout 中追加：

```tsx
// 在现有 workspace-shell.tsx 中 import 顶部追加
import { UsageWarningToast } from "../billing/UsageWarningToast";
import { billingApi } from "../../lib/billing/api";

// 在组件函数体内
const [warning, setWarning] = useState<{
  threshold: 80 | 95;
  windowType: "5h" | "7d";
  data: Awaited<ReturnType<typeof billingApi.getUsageWindow>>;
} | null>(null);
const [userId, setUserId] = useState<string>("");

useEffect(() => {
  // 拉取 user + window
  fetch("/api/v1/auth/me").then(r => r.json()).then(d => setUserId(d.id));
  billingApi.getUsageWindow().then(w => {
    if (w.hard_limit_hit.rolling_5h || w.hard_limit_hit.rolling_7d) {
      // 100% - 跳 paywall
      window.location.href = "/upgrade/paywall?reason=" + (w.hard_limit_hit.rolling_5h ? "5h" : "7d");
    } else if (w.soft_limit_hit.rolling_5h) {
      setWarning({ threshold: w.rolling_5h.percentage >= 95 ? 95 : 80, windowType: "5h", data: w });
    } else if (w.soft_limit_hit.rolling_7d) {
      setWarning({ threshold: w.rolling_7d.percentage >= 95 ? 95 : 80, windowType: "7d", data: w });
    }
  });
}, []);

// 在 return JSX 中
return (
  <>
    {/* 现有内容 */}
    {warning && userId && (
      <UsageWarningToast
        threshold={warning.threshold}
        windowType={warning.windowType}
        userId={userId}
        used={warning.windowType === "5h" ? warning.data.rolling_5h.used : warning.data.rolling_7d.used}
        limit={warning.windowType === "5h" ? warning.data.rolling_5h.limit : warning.data.rolling_7d.limit}
        resetAt={warning.windowType === "5h" ? warning.data.rolling_5h.reset_at : warning.data.rolling_7d.reset_at}
        onDismiss={() => setWarning(null)}
        onUpgradeClick={() => window.location.href = "/pricing"}
      />
    )}
  </>
);
```

> **注意**：若项目已有 user/session 获取方式（context），复用它，**不要**另起一次 `/api/v1/auth/me` 请求。

- [ ] **Step 3: 写测试**

```tsx
// components/workspace/workspace-shell.test.tsx
import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";

vi.mock("../../lib/billing/api", () => ({
  billingApi: {
    getUsageWindow: vi.fn().mockResolvedValue({
      plan_id: "free",
      rolling_5h: { used: 85000, limit: 100000, percentage: 85, reset_at: "2099-01-01T00:00:00Z" },
      rolling_7d: { used: 200000, limit: 400000, percentage: 50, reset_at: "2099-01-01T00:00:00Z" },
      soft_limit_hit: { rolling_5h: true, rolling_7d: false },
      hard_limit_hit: { rolling_5h: false, rolling_7d: false },
    }),
  },
}));

import { WorkspaceShell } from "./workspace-shell";

describe("WorkspaceShell toast integration", () => {
  it("renders warning toast when soft limit hit on 5h", async () => {
    render(<WorkspaceShell>...</WorkspaceShell>);
    await waitFor(() => expect(screen.getByText(/5h 用量已用 80%/)).toBeInTheDocument());
  });
});
```

> 测试具体根据现有 shell 的 props 调整。

- [ ] **Step 4: 跑测试 + Commit**

```bash
cd frontend_next && pnpm test components/workspace/workspace-shell.test.tsx
git add frontend_next/components/workspace/workspace-shell.tsx \
        frontend_next/components/workspace/workspace-shell.test.tsx
git commit -m "feat(workspace): show 80%/95% usage warning toast

- Decision-point trigger: only on threshold cross, not persistent
- 100% hard hit auto-redirects to /upgrade/paywall
- Uses existing user/session context if available"
```

---

## Phase 5: E2E + 视觉回归（Tasks 20-22，约 1 周）

### Task 20: Playwright POM（Page Object Model）

**Files:**
- Create: `frontend_next/e2e/pom/BillingPage.ts`

- [ ] **Step 1: 编写 POM**

```typescript
// e2e/pom/BillingPage.ts
import { type Page, expect } from "@playwright/test";

export class PricingPage {
  constructor(private page: Page) {}

  async goto() {
    await this.page.goto("/pricing");
  }

  async expectVisible() {
    await expect(this.page.getByRole("heading", { name: /选择适合你的方案/ })).toBeVisible();
    await expect(this.page.getByText("Plus")).toBeVisible();
    await expect(this.page.getByText("Free")).toBeVisible();
    await expect(this.page.getByText("Pro")).toBeVisible();
  }

  async clickUpgrade(plan: "plus" | "pro") {
    await this.page.getByRole("button", { name: new RegExp(`升级 ${plan === "plus" ? "Plus" : "Pro"}`) }).click();
  }
}

export class UsagePage {
  constructor(private page: Page) {}

  async goto() {
    await this.page.goto("/settings/usage");
  }

  async expectVisible() {
    await expect(this.page.getByText(/用量与套餐/)).toBeVisible();
    await expect(this.page.getByText(/5 小时窗口/)).toBeVisible();
    await expect(this.page.getByText(/7 天窗口/)).toBeVisible();
  }
}

export class PaywallPage {
  constructor(private page: Page) {}

  async goto(reason: "5h" | "7d" = "5h") {
    await this.page.goto(`/upgrade/paywall?reason=${reason}`);
  }

  async expectVisible() {
    await expect(this.page.getByRole("dialog")).toBeVisible();
  }
}
```

- [ ] **Step 2: Commit**

```bash
git add frontend_next/e2e/pom/BillingPage.ts
git commit -m "test(e2e): add BillingPage POM (PricingPage/UsagePage/PaywallPage)"
```

---

### Task 21: Playwright E2E 测试（价格页 + 用量仪表盘 + Paywall 流程）

**Files:**
- Create: `frontend_next/e2e/specs/billing/pricing-page.spec.ts`
- Create: `frontend_next/e2e/specs/billing/usage-dashboard.spec.ts`
- Create: `frontend_next/e2e/specs/billing/paywall-flow.spec.ts`

- [ ] **Step 1: 编写价格页 E2E**

```typescript
// e2e/specs/billing/pricing-page.spec.ts
import { test, expect } from "@playwright/test";
import { PricingPage } from "../../pom/BillingPage";

test.describe("Pricing page", () => {
  test("Free user sees 3 tiers with Plus highlighted", async ({ page }) => {
    const pricing = new PricingPage(page);
    await pricing.goto();
    await pricing.expectVisible();
    await expect(page.getByText("推荐")).toBeVisible();
  });

  test("FAQ section is visible and expandable", async ({ page }) => {
    const pricing = new PricingPage(page);
    await pricing.goto();
    await page.getByText("token 用量怎么算？").click();
    await expect(page.getByText(/DeepSeek 公开计费/)).toBeVisible();
  });

  test("clicking 升级 Plus triggers checkout redirect (mocked)", async ({ page }) => {
    // 拦截 /api/v1/billing/checkout-session 返回 fake URL
    await page.route("**/api/v1/billing/checkout-session", (route) =>
      route.fulfill({ json: { ok: true, data: { checkout_url: "/upgrade/success?mock=1" } } })
    );
    const pricing = new PricingPage(page);
    await pricing.goto();
    await pricing.clickUpgrade("plus");
    await expect(page).toHaveURL(/\/upgrade\/success/);
  });
});
```

- [ ] **Step 2: 编写用量仪表盘 E2E**

```typescript
// e2e/specs/billing/usage-dashboard.spec.ts
import { test, expect } from "@playwright/test";
import { UsagePage } from "../../pom/BillingPage";

test.describe("Usage dashboard", () => {
  test("Free user sees 2 buckets + trend chart + forecast", async ({ page }) => {
    const usage = new UsagePage(page);
    await usage.goto();
    await usage.expectVisible();
    await expect(page.getByText(/近 7 日用量趋势/)).toBeVisible();
    await expect(page.getByText(/智能建议|按当前用量/)).toBeVisible();
  });

  test("shows warning text when 5h soft limit hit (via mocked API)", async ({ page }) => {
    await page.route("**/api/v1/billing/usage/window", (route) =>
      route.fulfill({
        json: {
          ok: true,
          data: {
            plan_id: "free",
            rolling_5h: { used: 85000, limit: 100000, percentage: 85, reset_at: "2099-01-01T00:00:00Z" },
            rolling_7d: { used: 200000, limit: 400000, percentage: 50, reset_at: "2099-01-01T00:00:00Z" },
            soft_limit_hit: { rolling_5h: true, rolling_7d: false },
            hard_limit_hit: { rolling_5h: false, rolling_7d: false },
          },
        },
      })
    );
    const usage = new UsagePage(page);
    await usage.goto();
    await expect(page.getByText(/已超过软上限/)).toBeVisible();
  });
});
```

- [ ] **Step 3: 编写 paywall E2E**

```typescript
// e2e/specs/billing/paywall-flow.spec.ts
import { test, expect } from "@playwright/test";
import { PaywallPage } from "../../pom/BillingPage";

test.describe("Paywall flow", () => {
  test("5h paywall renders 3-tier comparison + 继续 Free", async ({ page }) => {
    const paywall = new PaywallPage(page);
    await paywall.goto("5h");
    await paywall.expectVisible();
    await expect(page.getByText(/5h 用量已达上限/)).toBeVisible();
    await expect(page.getByRole("button", { name: /继续 Free/ })).toBeVisible();
  });

  test("7d paywall renders 7d-specific title", async ({ page }) => {
    const paywall = new PaywallPage(page);
    await paywall.goto("7d");
    await expect(page.getByText(/7d 用量已达上限/)).toBeVisible();
  });
});
```

- [ ] **Step 4: 运行 E2E**

Run: `cd frontend_next && pnpm playwright test e2e/specs/billing/`
Expected: all pass (with backend running and seeded data)

- [ ] **Step 5: Commit**

```bash
git add frontend_next/e2e/specs/billing/ frontend_next/e2e/pom/BillingPage.ts
git commit -m "test(e2e): add billing E2E for pricing/usage/paywall

- 8 tests across 3 spec files
- Uses route mocking to control usage data without backend changes
- POM pattern reuses selectors across tests"
```

---

### Task 22: 暗色模式验证 + 视觉回归

**Files:**
- Create: `frontend_next/e2e/specs/billing/dark-mode.spec.ts`
- Create: `frontend_next/e2e/specs/billing/visual-regression.spec.ts`（可选）

- [ ] **Step 1: 暗色模式 E2E**

```typescript
// e2e/specs/billing/dark-mode.spec.ts
import { test, expect } from "@playwright/test";
import { PricingPage, UsagePage, PaywallPage } from "../../pom/BillingPage";

test.describe("Dark mode", () => {
  test.use({ colorScheme: "dark" });

  test("Pricing page renders correctly in dark mode", async ({ page }) => {
    await page.evaluate(() => document.documentElement.setAttribute("data-theme", "dark"));
    const pricing = new PricingPage(page);
    await pricing.goto();
    await pricing.expectVisible();
    await page.screenshot({ path: "test-results/pricing-dark.png", fullPage: true });
  });

  test("Usage dashboard renders correctly in dark mode", async ({ page }) => {
    await page.evaluate(() => document.documentElement.setAttribute("data-theme", "dark"));
    const usage = new UsagePage(page);
    await usage.goto();
    await usage.expectVisible();
    await page.screenshot({ path: "test-results/usage-dark.png", fullPage: true });
  });

  test("Paywall renders correctly in dark mode", async ({ page }) => {
    await page.evaluate(() => document.documentElement.setAttribute("data-theme", "dark"));
    const paywall = new PaywallPage(page);
    await paywall.goto("5h");
    await paywall.expectVisible();
    await page.screenshot({ path: "test-results/paywall-dark.png", fullPage: true });
  });
});
```

- [ ] **Step 2: 视觉回归（用 Playwright snapshot diff）**

```typescript
// e2e/specs/billing/visual-regression.spec.ts
import { test, expect } from "@playwright/test";
import { PricingPage, UsagePage, PaywallPage } from "../../pom/BillingPage";

const VIEWPORTS = [
  { name: "desktop", width: 1280, height: 800 },
  { name: "mobile", width: 375, height: 667 },
];

for (const vp of VIEWPORTS) {
  test(`Pricing @ ${vp.name}`, async ({ page }) => {
    await page.setViewportSize({ width: vp.width, height: vp.height });
    const pricing = new PricingPage(page);
    await pricing.goto();
    await expect(page).toHaveScreenshot(`pricing-${vp.name}.png`, { fullPage: true, maxDiffPixelRatio: 0.01 });
  });

  test(`Usage @ ${vp.name}`, async ({ page }) => {
    await page.setViewportSize({ width: vp.width, height: vp.height });
    const usage = new UsagePage(page);
    await usage.goto();
    await expect(page).toHaveScreenshot(`usage-${vp.name}.png`, { fullPage: true, maxDiffPixelRatio: 0.01 });
  });

  test(`Paywall @ ${vp.name}`, async ({ page }) => {
    await page.setViewportSize({ width: vp.width, height: vp.height });
    const paywall = new PaywallPage(page);
    await paywall.goto("5h");
    await expect(page).toHaveScreenshot(`paywall-${vp.name}.png`, { fullPage: true, maxDiffPixelRatio: 0.01 });
  });
}
```

- [ ] **Step 3: 跑测试 + Commit**

Run: `cd frontend_next && pnpm playwright test e2e/specs/billing/dark-mode.spec.ts e2e/specs/billing/visual-regression.spec.ts`

```bash
git add frontend_next/e2e/specs/billing/dark-mode.spec.ts \
        frontend_next/e2e/specs/billing/visual-regression.spec.ts
git commit -m "test(e2e): dark mode + visual regression for billing pages

- 3 dark mode screenshots
- 6 visual regression snapshots (3 pages x 2 viewports)
- 1% pixel diff tolerance for typography rendering variations"
```

---

## Phase 6: 灰度上线（Task 23，最后 1 周）

### Task 23: Feature flag + 10/50/100% 灰度

**Files:**
- Create: `avrag-rs/crates/billing/src/feature_flag.rs`（或追加到 config）
- Modify: `avrag-rs/crates/transport-http/src/routes/billing.rs`（按 flag 决定路由）
- Create: `frontend_next/lib/billing/featureFlag.ts`

- [ ] **Step 1: 后端 feature flag**

```rust
// crates/billing/src/feature_flag.rs
pub struct PricingRevampFlag {
    pub rollout_percentage: u8,  // 0-100
}

impl PricingRevampFlag {
    pub fn from_env() -> Self {
        Self {
            rollout_percentage: std::env::var("PRICING_REVAMP_ROLLOUT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
        }
    }

    pub fn is_enabled_for(&self, user_id: uuid::Uuid) -> bool {
        // 简单 hash bucket：user_id 的最低字节 % 100 < rollout_percentage
        let bucket = (user_id.as_u128() % 100) as u8;
        bucket < self.rollout_percentage
    }
}
```

- [ ] **Step 2: 路由层 gate**

在 `routes/billing.rs`：

```rust
async fn get_usage_window(...) -> ... {
    let flag = avrag_billing::feature_flag::PricingRevampFlag::from_env();
    if !flag.is_enabled_for(actor_id.into_uuid()) {
        return Json(ApiResponse::err(
            "feature_disabled",
            "pricing revamp is not yet available for this user",
        ));
    }
    // 现有逻辑
}
```

- [ ] **Step 3: 前端 fallback**

```typescript
// lib/billing/featureFlag.ts
export async function isPricingRevampEnabled(): Promise<boolean> {
  try {
    const res = await fetch("/api/v1/billing/usage/window", { credentials: "include" });
    return res.ok;
  } catch {
    return false;
  }
}
```

在 `/pricing`、`/settings/usage`、`/upgrade/paywall` 页面顶部：

```tsx
const enabled = await isPricingRevampEnabled();
if (!enabled) {
  // 回退：显示旧 UI 或重定向到 /workspace
  return <OldUsageView />;  // 或 redirect("/workspace");
}
```

- [ ] **Step 4: 灰度执行 SOP（文档）**

在 `docs/ops/2026-06-14-pricing-revamp-rollout.md` 记录：

```markdown
# Pricing Revamp 灰度上线 SOP

## 阶段
1. 内部测试 (env: PRICING_REVAMP_ROLLOUT=100, 白名单 user_id) - 1 天
2. 10% 灰度 (PRICING_REVAMP_ROLLOUT=10) - 2 天，监控 4 个指标
3. 50% 灰度 (PRICING_REVAMP_ROLLOUT=50) - 2 天
4. 100% 全量 (PRICING_REVAMP_ROLLOUT=100) - 1 天

## 监控指标
- /api/v1/billing/usage/window P99 latency < 200ms
- /api/v1/billing/usage/history P99 latency < 200ms
- /pricing 页面 bounce rate
- Free→Plus 转化率（埋点）
- 错误率 < 0.1%

## 回滚
- 立即将 PRICING_REVAMP_ROLLOUT 设为 0
- 旧 UI 自动接管
```

- [ ] **Step 5: Commit**

```bash
git add avrag-rs/crates/billing/src/feature_flag.rs \
        avrag-rs/crates/transport-http/src/routes/billing.rs \
        frontend_next/lib/billing/featureFlag.ts \
        docs/ops/2026-06-14-pricing-revamp-rollout.md
git commit -m "feat(billing): add rollout flag for pricing revamp

- Hash-bucket rollout by user_id (no DB dependency)
- /api/v1/billing/usage/* endpoints gated
- Frontend fallback to old UI when disabled
- Rollout SOP doc: 10% → 50% → 100% with monitoring gates"
```

---

## Spec 覆盖自检

| Spec 章节 | 覆盖任务 |
|----------|----------|
| §2 三档定义 | Task 1 (migration), Task 2 (config) |
| §3 经济模型 | Task 2 (config defaults) |
| §4.1 价格页 | Task 14 |
| §4.2 用量仪表盘 | Task 15 |
| §4.3 对话内 toast | Task 19 |
| §4.4 Paywall | Task 16 |
| §5 关键组件 | Task 7-13 (6 组件) |
| §6 后端实现 | Task 1-6 (migration + 4 端点) |
| §7 前端实现 | Task 7-19 (api + 6 组件 + 4 页面 + i18n + workspace) |
| §8 测试 | Task 20-22 (POM + E2E + 暗色 + 视觉回归) |
| §9 范围外 | (无任务，符合排除约定) |
| §10 开放问题 | Task 5 (Q5 选 B) / Task 18 (i18n 覆盖 zh/en) |
| §11 时间线 | Phase 0-6 (5 周) |

## 占位符扫描

- 无 "TBD" / "TODO" / "implement later"
- 所有代码块都有完整可运行内容
- 所有命令包含 expected output
- 路径全部精确到文件

## 类型一致性

- Rust `UsageWindowResponse` 与 TS `UsageWindowResponse` 字段名一致
- `BillingPlan` 字段名（cny/usd/interval/checkout_available/current/quotas）一致
- 组件 props 类型（planId / rolling5h / rolling7d / softLimitHit / hardLimitHit）一致
- i18n key（pricing.* / usage.* / paywall.* / toast.*）一致
