# E3.4 任务记录：商业交易、定价、充值、拦截墙与成功页面 (`/pricing`, `/upgrade/*`, `/desktop/buy`)

日期：2026-09-05
负责人：Agent / Solo Trunk
关联计划：[2026-09-05-development-execution-plan.md](2026-09-05-development-execution-plan.md) §6 (E3.4)
门禁目标：E3.4 切片完成

---

## 1. 任务目标与交付范围

交付商业化交易与定价路由族（共 4 个端点）：
1. **定价与会员充值 (`/pricing`)**：
   - 公开营销展示：Free、Pro 会员月付/年付套餐对比表，功能权益清单；
   - 钱包充值面板 (`#topup`)：支持 `topup_50`、`topup_100`、`topup_200` 套餐，集成支付宝（Alipay）/ Creem 测试模式发起支付跳转；
   - 遵从权威 IA：`/pricing` 是充值与订阅的 Canonical 页面。
2. **拦截墙解释页 (`/upgrade/paywall`)**：
   - 当用户额度耗尽、或触发需要 Pro 会员的高级能力时展示；
   - 说明当前用量限制，提供明确的升级通道（导向 `/pricing` 或直接选包结账）；
   - 不作为第二套支付宿主，仅作为解释引导。
3. **支付成功回跳与状态轮询 (`/upgrade/success`)**：
   - 支付完成后的回调落地页，读取 `session_id` / `order_no`，轮询订单状态或展示“支付成功，权益已到账”；
   - 提供快速返回工作台或对话入口。
4. **桌面端购买引导 (`/desktop/buy`)**：
   - 桌面客户端专属买断/订阅说明页，引导在浏览器中完成安全结账。
5. **安全与测试模式不变量**：
   - 严格使用既有测试/沙箱模式（如 `creem` 测试 token 或模拟支付回调），**严禁发起真实扣款**；
   - 套餐价格和配置完全源于后端契约与现行商业定义，不重造商业模式。

---

## 2. 执行步骤

| 次序 | 工作项 | 状态 | 产物与证据 |
|---|---|---|---|
| E3.4.1 | SDK 交易与支付 REST 接口补齐 | 已完成 | `web-sdk/src/billing_api.rs`, 测试 3/3 passed |
| E3.4.2 | 实现定价与充值页面 (`/pricing`) | 已完成 | `pricing_page.rs`, 权益卡片与 `#topup` 面板 |
| E3.4.3 | 实现拦截墙与成功回跳页 (`/upgrade/paywall`, `/upgrade/success`) | 已完成 | `paywall_page.rs`, `success_page.rs` |
| E3.4.4 | 实现桌面端购买引导页 (`/desktop/buy`) | 已完成 | `desktop_buy_page.rs` 免费客户端与云端升级说明 |
| E3.4.5 | 路由挂载、样式集成与自动化测试断言 | 已完成 | `billing-journey.spec.ts` 3/3, 全量 38/38 passed |
| E3.4.6 | 验证收敛、图谱更新与本地提交 | 已完成 | 图谱已更新 (19 files)，本地提交 `82076d9b` (E3.4 达成) |
