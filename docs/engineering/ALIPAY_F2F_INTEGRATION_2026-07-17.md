# 支付宝当面付集成（F2F / alipay.trade.precreate）

**日期**: 2026-07-17
**状态**: **Done**（后端修补 + 前端扫码闭环 + 生产网关签名链路实测通过：`alipay_precreate_smoke` 返回 qr_code）
**关联**: [STRIPE_BILLING_REMOVAL_2026-07-13.md](./STRIPE_BILLING_REMOVAL_2026-07-13.md)、[DEEPSEEK_STYLE_USAGE_BILLING_DESIGN_2026-07-13.md](./DEEPSEEK_STYLE_USAGE_BILLING_DESIGN_2026-07-13.md)（冻结价格 ¥49/¥129）

---

## 0. 一句话

国内用户在定价页/Paywall 选择 Plus/Pro → 后端调 `alipay.trade.precreate` 生成二维码 → 用户支付宝 App 扫码付款 → 支付宝异步 notify 命中 `/webhooks/alipay` → 订单置 paid 并开通 **30 天**订阅 → 前端轮询订单状态收敛 UI。

**产品语义**：当面付 = 单笔支付，**不自动续费**；每笔 `TRADE_SUCCESS` 将订阅期刷新为 `now() + 30 days`，到期后需再次扫码购买（到期回退由既有 `expire_subscriptions` 维护任务处理）。

## 1. 端到端时序

```text
Pricing/Paywall (zh-CN → provider=alipay)
  → POST /api/v1/billing/checkout-session {plan_id, provider:"alipay"}
  → BillingService::create_checkout
      · 校验 consent + plan 价格（ALIPAY_PRICE_*）
      · out_trade_no = uuid；insert billing_orders(status='pending', amount_cents, 'CNY')
      → AlipayClient::create_precreate_order (RSA2 签名, 东八区 timestamp)
      ← qr_code
  ← {qr_code, order_id}
  → 前端 AlipayQrDialog 渲染二维码（qrcode 包本地生成图）
      · 每 2s 轮询 GET /api/v1/billing/orders/{order_id}（仅本人订单，RLS）
支付宝 → POST /webhooks/alipay (form-encoded)
  → 验签（支付宝公钥 RSA2）→ app_id 匹配校验 → 幂等 lease
  → process_webhook_event: TRADE_SUCCESS/TRADE_FINISHED
      · total_amount 与 billing_orders.amount_cents 比对（防伪造/窜单）
      · billing_orders → 'paid'
      · subscriptions upsert：active，period = now() ~ now()+30d
      · billing_outbox(subscription.paid) + 应用内通知（+ desktop 证书履约）
  ← 纯文本 "success"（支付宝硬性要求，否则按失败重投）
前端轮询到 status='paid' → 成功提示 + 刷新方案
```

## 2. 代码锚点

| 主题 | 文件 |
|------|------|
| RSA2 签名/验签 + precreate（timestamp 固定东八区） | `avrag-rs/crates/billing/src/alipay_client.rs` |
| checkout provider=alipay 分支 / 订单状态查询 / notify app_id 校验 / UTF-8 安全 percent_decode | `avrag-rs/crates/billing/src/service.rs` |
| notify 金额比对（total_amount vs amount_cents） | `avrag-rs/crates/app-bootstrap/src/adapters/billing_sql/core_webhooks/process.rs` |
| 订单状态 port + PG 实现（set_current_user + 本人过滤） | `app-core/src/billing_store.rs`、`app-bootstrap/src/adapters/pg_billing_store.rs` |
| 路由 `GET /api/v1/billing/orders/{order_id}` | `transport-http/src/routes/billing.rs`、`app-bootstrap/src/product_apps/billing.rs` |
| notify 返回纯文本 success | `transport-http/src/lib_impl/infra_handlers.rs`（Alipay 且 ok 时） |
| 前端二维码弹窗 + 轮询 | `frontend_next/components/billing/AlipayQrDialog.tsx` |
| locale → provider（zh-CN=alipay） | `frontend_next/lib/billing/provider.ts` |
| 定价页 / Paywall 接入 | `frontend_next/app/(marketing)/pricing/pricing-page-client.tsx`、`app/(app)/upgrade/paywall/paywall-page-client.tsx` |

## 3. 安全语义（本次补齐）

| 校验 | 位置 | 说明 |
|------|------|------|
| RSA2 验签 | `AlipayClient::verify_signature` | 支付宝公钥；参数排序、剔除 sign/sign_type |
| **请求签名含 sign_type** | `AlipayClient::sign` | 请求签名只剔除 `sign`（**保留 sign_type**）；notify 验签才同时剔除两者——两者规则不同，写反即「验签出错」 |
| **公共参数走 URL query** | `create_precreate_order` | 支付宝要求 app_id/charset/sign 等公共参数放 URL 查询串（`.query()`），放 POST form body 会被拒 |
| **UTF-8 percent_decode** | `service.rs` | 字节级解码（旧实现按 Latin-1 逐字节转 char，中文 subject 会验签失败） |
| **app_id 匹配** | `service.rs` | notify 的 app_id 必须等于 `ALIPAY_APP_ID`，否则拒收 |
| **金额比对** | `process.rs` | notify `total_amount` 换算分后必须等于订单 `amount_cents`，不符 bail（支付宝会重投，运维据此告警） |
| 订单归属 | `pg_billing_store.rs` | 状态查询强制 `user_id` 过滤 + RLS `set_current_user` |
| 幂等 | lease (`claim_webhook_with_lease`) | notify_id 去重；重投安全 |
| **IP 白名单** | 支付宝控制台（应用→开发设置） | 开启时只放行名单内出口 IP；生产填**生产服务器**出口 IP，本地调试可临时加本机公网 IP |

## 4. 配置（部署 env）

```text
ALIPAY_APP_ID=2021xxxxxxxxxxxx            # 开放平台应用（已签约当面付）
ALIPAY_PRIVATE_KEY=<应用私钥, PEM 或纯 base64>
ALIPAY_PUBLIC_KEY=<支付宝公钥>             # 上传应用公钥后换取，注意不是应用公钥
ALIPAY_GATEWAY_URL=https://openapi.alipay.com/gateway.do
ALIPAY_NOTIFY_URL=https://<公网域名>/webhooks/alipay
ALIPAY_PRICE_PLUS=49.00                   # 冻结价（设计 §5.1）
ALIPAY_PRICE_PRO=129.00
```

- 沙箱：`ALIPAY_GATEWAY_URL=https://openapi-sandbox.dl.alipaydev.com/gateway.do` + 沙箱 AppID/密钥 + 沙箱买家版 App 扫码。
- `ALIPAY_NOTIFY_URL` 缺省时回退 `${AVRAG_PUBLIC_BASE_URL}/webhooks/alipay`；生产必须显式配置公网地址。
- 未配置 `ALIPAY_APP_ID` 时 checkout 返回 `billing_unconfigured`。

## 5. 冒烟步骤

1. 沙箱 env 启动后端；zh-CN locale 打开 `/pricing`，登录并勾选支付协议。
2. 点 Plus/Pro → 弹二维码（网络面板可见 checkout-session 返回 `qr_code`/`order_id`）。
3. 沙箱买家版 App 扫码付款 → 观察 `/webhooks/alipay` 200 且响应体为 `success`。
4. `billing_orders.status='paid'`；`subscriptions` 出现 alipay 行且 period_end ≈ +30d。
5. 前端 2s 内轮询到 paid → 成功 UI，方案刷新。
6. 生产：用 0.01 元真实单重复 2–5（可在支付宝商家中心核对账单）。

## 6. 上线 checklist（用户侧 / 支付宝开放平台）

- [ ] 应用已签约**当面付**（`alipay.trade.precreate` 权限随之可用）。
- [ ] 接口加签方式 = **公钥模式（RSA2）**：上传应用公钥 → 换取支付宝公钥。
- [ ] **IP 白名单**（若开启）：加入生产服务器出口 IP；本地联调临时加本机公网 IP，联调后移除。
- [ ] env 六项按 §4 配置（生产网关 + 公网 notify url）。
- [ ] 沙箱全链路冒烟通过 → 生产 0.01 元真实单验证。
- [ ] 定价页 zh-CN 出二维码、en 走 Creem（不回归）。
- [ ] 签名链路冒烟：`cd avrag-rs && cargo run -p avrag-billing --example alipay_precreate_smoke -- .env` 返回 `OK — qr_code:`。

## 7. 非目标（本期不做）

- `alipay.trade.query` 主动对账 / `alipay.trade.close` 关单 / 退款 `alipay.trade.refund`。
- `TRADE_CLOSED` 分支与 pending 订单过期清理（运维 wave）。
- 支付宝周期扣款（自动续费需另行签约 + 产品决策）。
- 电脑网站支付 / 手机网站支付（后续如需：扩展 `alipay_client.rs` 一个方法 + checkout 分支）。
- 营销素材 `$3.19/$5.99` SVG 价格图刷新（另开文案/资产 wave）。

## 8. 验证命令

```bash
cd avrag-rs && cargo test -p avrag-billing
cd avrag-rs && cargo check -p app-core -p app-bootstrap -p transport-http
cd frontend_next && pnpm exec vitest run tests/billing tests/settings
```

> e2e 说明：`e2e/specs/billing/pricing-page.spec.ts` 的 Playwright 全栈环境本次未跑；
> 支付宝扫码链路已由 `tests/billing/alipay-checkout.test.tsx`（mock checkout + 轮询 paid）覆盖，
> 真机验证按 §5 沙箱冒烟执行。
