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
- 前端同步设置 `NEXT_PUBLIC_PRICING_REVAMP_ENABLED=0`
- 旧 UI 自动接管

## 相关文档

- 设计与任务清单：[`docs/superpowers/plans/2026-06-07-pricing-tiers-revamp-plan.md`](../../../docs/superpowers/plans/2026-06-07-pricing-tiers-revamp-plan.md)
- 规格说明：[`docs/superpowers/specs/2026-06-07-pricing-tiers-revamp-design.md`](../../../docs/superpowers/specs/2026-06-07-pricing-tiers-revamp-design.md)
