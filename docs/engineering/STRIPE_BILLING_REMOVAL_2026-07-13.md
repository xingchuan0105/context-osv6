# Stripe 支付方案移除（产品硬切）

**日期**: 2026-07-13  
**状态**: **Done**（代码路径移除；库表 residual 列名保留）  
**关联**: [PRODUCT_UI_CHROME_AUDIT_2026-07-13.md](./PRODUCT_UI_CHROME_AUDIT_2026-07-13.md)、[PRODUCT_UI_CHROME_AND_BILLING_DEV_PLAN_2026-07-13.md](./PRODUCT_UI_CHROME_AND_BILLING_DEV_PLAN_2026-07-13.md)（执行波次）、ADR-0001（用户级账单，历史文案含 Stripe）、多提供方迁移 0036  

---

## 0. 决策

| 项 | 结论 |
|----|------|
| 产品支付 | **仅 Creem（国际卡）+ Alipay（国内）** |
| Stripe Checkout | **禁止**（API 返回 `billing_provider_removed`） |
| Stripe Customer Portal | **禁止**（`portal-session` 固定 unavailable；前端不再依赖外部门户） |
| Stripe Webhook | **拒绝**（`/webhooks/stripe` → **410 Gone**；处理层 bail） |
| 配置 / 客户端 | **删除** `StripeClient`、`STRIPE_*` 配置字段、`stripe_client.rs` |
| 历史库表 | **保留** `users.stripe_customer_id`、`subscriptions.stripe_*` 等列名（residual，只读/兼容旧行） |

---

## 1. 产品影响

### 1.1 结账

- `POST /api/v1/billing/checkout-session`  
  - `provider=creem` | `alipay`（或默认 Creem）→ 正常  
  - `provider=stripe` → **`billing_provider_removed`**

### 1.2 管理订阅

- **不再**跳转 Stripe Customer Portal。  
- Settings →「管理订阅」：展开**应用内方案列表**；「更换方案」→ `/pricing`（Creem/支付宝结账）。  
- `POST /api/v1/billing/portal-session` 仍存在（兼容旧客户端），恒返回 `billing_portal_unavailable`。

### 1.3 Webhook

| 路径 | 行为 |
|------|------|
| `/webhooks/creem` | 有效 |
| `/webhooks/alipay` | 有效 |
| `/webhooks/stripe` | **410 Gone** + `billing_provider_removed` |

运维：从 Stripe Dashboard 删除指向本产品的 webhook 端点；`.env` 中 **删除/忽略** 一切 `STRIPE_*`。

---

## 2. 代码变更清单（本切）

| 区域 | 变更 |
|------|------|
| `avrag-rs/crates/billing/src/stripe_client.rs` | **删除文件** |
| `billing/src/lib.rs` | 去掉 `StripeClient` 导出 |
| `billing/src/service.rs` | 无 `stripe` 字段；checkout/portal/webhook 拒 Stripe |
| `billing/src/core.rs` | 删除 `ensure_customer` / `load_customer_id` 门户辅助 |
| `app-core/src/billing_domain.rs` | 去掉 `stripe_secret_key` 等配置与 `stripe_enabled`；`plan_id_by_price_id` 仅 Creem |
| `billing_sql/.../process.rs` | Stripe 分支直接 bail |
| `transport-http/.../infra_handlers.rs` | Stripe webhook 410；OpenAPI 路径改为 creem/alipay |
| `frontend_next/.../settings-billing-panel.tsx` | 管理订阅 = 应用内方案；不调 portal |
| `frontend_next/.../client.ts` | `CheckoutRequest.provider` 仅 `creem` \| `alipay` |

### 2.1 有意未删（residual）

| 项 | 原因 |
|----|------|
| DB 列 `stripe_customer_id` / `stripe_subscription_id` / `stripe_price_id` | 历史迁移已落地；硬删需单独迁移与数据迁移波次 |
| `BillingProvider::Stripe` 枚举值 | 反序列化旧 `billing_provider='stripe'` 行；**不得**再作结账目标 |
| `save_stripe_customer_id` / `load_customer_id` 端口 | 仍可能被存储适配器实现；**无产品调用** |
| 旧 migration SQL 文件 | 不可改写历史 |
| `archive/`、旧 PRD 副本 | 归档，不作为产品真相 |

后续若要 **schema 硬删列**：单独 O-wave + 迁移（将旧 stripe 行标记 `canceled` 后 DROP COLUMN）。

---

## 3. 配置清单（运维）

**删除 / 勿再配置：**

```text
STRIPE_SECRET_KEY
STRIPE_WEBHOOK_SECRET
STRIPE_PRICE_PRO / STRIPE_PRICE_PRO_MONTHLY / STRIPE_PRICE_ID
STRIPE_PRICE_PLUS / STRIPE_PRICE_ENTERPRISE
```

**保留（产品支付）：**

```text
CREEM_API_KEY
CREEM_WEBHOOK_SECRET
CREEM_PRODUCT_PRO / CREEM_PRODUCT_PLUS / CREEM_PRODUCT_DESKTOP_*
CREEM_PRICE_PRO / CREEM_PRICE_PLUS
ALIPAY_APP_ID / ALIPAY_PRIVATE_KEY / ALIPAY_PUBLIC_KEY / ALIPAY_GATEWAY_URL / ALIPAY_NOTIFY_URL
ALIPAY_PRICE_*
PUBLIC_APP_BASE_URL
```

---

## 4. 验证

```bash
# 单元
cd avrag-rs && cargo test -p avrag-billing --lib

# 编译面
cargo check -p avrag-billing -p app-bootstrap -p transport-http

# 前端
cd frontend_next && pnpm exec vitest run tests/settings/settings-surface.test.tsx
```

手工：

1. Settings → 账单 → **管理订阅** → 出现方案列表，**不**跳 Stripe。  
2. **更换方案** → `/pricing`。  
3. `POST /webhooks/stripe` → 410。  
4. Checkout 仅 Creem/Alipay。

---

## 5. 文档同步

| 文档 | 动作 |
|------|------|
| 本文件 | 权威移除记录 |
| `PRODUCT_UI_CHROME_AUDIT_2026-07-13.md` | §4 管理订阅改为「无外部门户 / 应用内」 |
| `FUNCTIONAL_ACCEPTANCE_CHECKLIST.md` / `GAP_ANALYSIS.md` | 历史「Stripe 全链路」表述过时，以本文件为准 |
| ADR-0001 | 保留；文中 Stripe 指历史实现，产品支付见本文件 |

---

## 6. 禁止回归

- **禁止**重新引入 `StripeClient`、Stripe Checkout、Stripe Portal、Stripe webhook 验签处理。  
- **禁止**在 UI 文案中出现「Stripe 账单门户」作为默认路径。  
- 新支付能力只扩展 **Creem / Alipay**（或未来明确 ADR 的第三方，**不得**静默加回 Stripe）。
