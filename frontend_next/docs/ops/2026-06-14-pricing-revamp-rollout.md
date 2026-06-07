# Pricing Revamp 灰度上线 SOP

## 环境变量（前后端协同）

| 变量 | 作用 |
|------|------|
| `PRICING_REVAMP_ROLLOUT`（后端） | 0–100，按 user_id hash 桶灰度； gated API 对桶外用户返回 `feature_disabled` |
| `NEXT_PUBLIC_PRICING_REVAMP_ENABLED=1`（前端） | 构建时总开关；为 0 时所有新 UI 路由 SSR 重定向到旧页面 |

**阶段 2–3（10%/50% 灰度）**：前端 env 保持 `=1`，桶外用户由客户端 `isPricingRevampEnabled()` 探测 `/billing/usage/window` 后隐藏用量/付费墙 UI，避免「新 UI + API 拒绝」的 broken UX。

**匿名 `/pricing`**：仅 env 门控（营销页不调用 usage API）；env=0 时 SSR 重定向。

**已登录页面**（`/settings/usage`、`/upgrade/paywall`、workspace）：env + API 探测，与后端桶一致。

## 阶段

1. 内部测试 (`PRICING_REVAMP_ROLLOUT=100`, 白名单 user_id) — 1 天
2. 10% 灰度 (`PRICING_REVAMP_ROLLOUT=10`, `NEXT_PUBLIC_PRICING_REVAMP_ENABLED=1`) — 2 天，监控 4 个指标
3. 50% 灰度 (`PRICING_REVAMP_ROLLOUT=50`) — 2 天
4. 100% 全量 (`PRICING_REVAMP_ROLLOUT=100`) — 1 天

## 监控指标

- `/api/v1/billing/usage/window` P99 latency < 200ms
- `/api/v1/billing/usage/history` P99 latency < 200ms
- `/pricing` 页面 bounce rate
- Free→Plus 转化率（埋点）
- 错误率 < 0.1%

## 回滚

- 立即将 `PRICING_REVAMP_ROLLOUT` 设为 `0`
- 前端同步设置 `NEXT_PUBLIC_PRICING_REVAMP_ENABLED=0` 并重新部署
- 旧 UI 自动接管

## E2E / 本地联调

- 前端：`NEXT_PUBLIC_PRICING_REVAMP_ENABLED=1`（见 `e2e/setup-env.ts`）
- 后端：`PRICING_REVAMP_ROLLOUT=100`（Playwright webServer 启动 avrag-api 时需配置，否则 E2E 用户可能收 `feature_disabled`）

## 相关文档

- 设计与任务清单：[`docs/superpowers/plans/2026-06-07-pricing-tiers-revamp-plan.md`](../../../docs/superpowers/plans/2026-06-07-pricing-tiers-revamp-plan.md)
- 规格说明：[`docs/superpowers/specs/2026-06-07-pricing-tiers-revamp-design.md`](../../../docs/superpowers/specs/2026-06-07-pricing-tiers-revamp-design.md)
