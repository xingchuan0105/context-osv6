# Pricing Tiers Revamp & Frontend Display Logic — Design Spec

> Date: 2026-06-07
> Status: approved (brainstorming phase)
> Scope: Backend (avrag-rs) quota + config; Frontend (frontend_next) pricing page, usage dashboard, in-conversation toast, paywall
> Reference products: Kimi Code Coding Plan, Moonshot token plan, DeepSeek public pricing
> Architecture: extend existing `crates/billing`; add `app/(marketing)/pricing`, `app/settings/usage`, `app/upgrade/paywall` routes in frontend_next

## 1. 背景与目标

### 1.1 现状（截至本设计）
- `crates/billing/src/types.rs` 已定义 `PLAN_FREE`/`PLAN_PLUS`/`PLAN_PRO` 与 `BillingPlan`/`BillingPlanQuota` 类型
- 已有 Stripe / Creem / Alipay 三个支付 provider
- 已有 "Lazy Billing Downgrade" 机制（订阅过期自动降回 Free）
- `quota_limits` 表迁移 0035/0036 后数值：
  - **Free**：pages 100/500, embed 100K/500K, llm_in 50K/100K, llm_out 25K/50K, storage 1GB/5GB
  - **Plus**：pages 5K/10K, embed 5M/10M, llm_in 500K/1M, llm_out 250K/500K, storage 5GB/10GB
  - **Pro**：pages / embed / llm_in / llm_out / storage 全部 NULL（无限）
- 价格仍为 placeholder：`$20/月` / `Contact sales` / Alipay 写死 20.00/100.00
- 三档**仅以"用量上限"区分**，没有功能差异
- Pro 与 Plus 的差别只剩"无限 vs 限额"，**升级理由薄弱**

### 1.2 目标
1. **拉高 Free → Plus 转化率**（核心 KPI）
2. 三档定位**清晰可感**：Free 体验完整但会被卡，Plus 日常够用，Pro 重度无忧
3. 定价**真实产品化**（脱离 placeholder，对标 Kimi Code / Moonshot 区间）
4. 前端展示逻辑**到最佳实践**：让用户**看得见** token 用量与升级价值
5. **70% 毛利率**为地板（实际模型可至 ~90%）

### 1.3 关键约束
- 目标用户：**个人知识工作者 / 独立研究者**（1 人 1 份）
- 三档**容量（pages / storage）完全相同**
- 三档**功能（RAG / 多模态 / 工具）完全相同**
- **唯一差异**：5h 滚动限额 + 7d 滚动限额（token 用量）
- 计费周期：**月付**（年付暂不开放）
- 货币：双币种 **CNY（¥）/ USD（$）**
- 支付渠道：Stripe（美元信用卡）/ Creem（国际信用卡）/ Alipay（CNY）
- 默认模型：**DeepSeek V4 Flash**（V4 Pro 留作未来"高级模型"加价项，本设计不含）

## 2. 目标三档定义

### 2.1 限额表（5h 滚动 + 7d 滚动，单位：token）

| 档位 | 5h 限额 | 7d 限额 | 5h:7d 比值 | 直观体验 |
|------|---------|---------|------------|----------|
| **Free** | 100,000 | 400,000 | 1 : 4 | 一周约 60 个轻量问题 |
| **Plus ⭐推荐** | 600,000 | 4,000,000 | 1 : 6.7 | 一周约 450 个问题，深度研究不卡 |
| **Pro** | 2,500,000 | 15,000,000 | 1 : 6 | 一周约 1800 个问题，重度无忧 |

**5h:7d 比值设计意图（显式说明）**：

- **Free 1:4 偏紧周内**——故意让 Free 用户先撞 7d 墙（周内累计焦虑），再撞 5h 墙（短时爆发焦虑）。这创造"持续被卡"的体感，是 Free→Plus 转化的核心动力。
- **Plus 1:6.7 更平衡**——付费后给到 5h 充裕感（鼓励深度集中研究），但周内依然有边界（防止滥用）。
- **Pro 1:6 略偏 5h**——5h 跳变 (4.2×) > 7d 跳变 (3.75×) 是有意的：Pro 卖给"重 burst 用户"（连续几个小时的深度工作），不卖给"全周稳定跑"用户。

**跳变不对称的设计意图**：

- **Free→Plus**：7d 跳变 (10×) > 5h 跳变 (6×)——Plus 主打"周内焦虑消除"
- **Plus→Pro**：5h 跳变 (4.2×) > 7d 跳变 (3.75×)——Pro 主打"重 burst 解放"
- 两个升级路径方向**故意不同**：不同用户痛点对应不同升级叙事，避免"Plus/Pro 是同一档只是贵"的感觉。

### 2.2 价格表（双币种）

| 档位 | CNY | USD | 定位语 |
|------|-----|-----|--------|
| Free | ¥0 | $0 | 体验核心功能，限额会被触发 |
| **Plus ⭐** | **¥49 / 月** | **$9 / 月** | 主力档，深度研究首选 |
| Pro | ¥129 / 月 | $19 / 月 | 重度使用，限额基本不会触发 |

### 2.3 跳变倍数（用于"升级价值"话术）

| 升级路径 | 5h 跳变 | 7d 跳变 | 心理感知 |
|----------|---------|---------|----------|
| Free → Plus | 6× | 10× | 显著 |
| Plus → Pro | 4.2× | 3.75× | 明显 |
| Free → Pro | 25× | 37.5× | 锚定 |

## 3. 经济模型（70% 毛利率倒推）

### 3.1 成本基线：DeepSeek V4 Flash

| 计费项 | 单价（元/M tokens）| 假设 |
|--------|---------------------|------|
| 输入（缓存命中）| 0.02 | 系统 prompt 反复命中 |
| 输入（缓存未命中）| 1.00 | 用户问题 / RAG 上下文 |
| 输出 | 2.00 | 模型回答 |
| **混合（50% 缓存命中, I:O=3:1）** | **≈ 0.90** | 实际经验值 |

> 数据来源：DeepSeek 公开定价（https://api-docs.deepseek.com/zh-cn/quick_start/pricing/）。

### 3.2 毛利率倒推公式

```
GM = 1 − (月成本 ÷ 月收入) ≥ 0.70
⇒ 月成本 ≤ 月收入 × 0.30
⇒ 7d 限额上限 = (月收入 × 0.30) ÷ (0.9 元/M tokens) × 1/4.3 周
```

### 3.3 各档成本与毛利率

| 档位 | 月价 | 月收入 | 7d 限额 | 月成本 @ 100% | 月成本 @ 50% | GM @ 100% | GM @ 50% |
|------|------|--------|---------|----------------|---------------|------------|----------|
| Free | ¥0 | — | 400K | ¥0.36 (我们承担) | — | -100% (获客成本) | — |
| **Plus** | **¥49** | 49 | 4M | ¥3.60 | ¥1.80 | **92.6%** | **96.3%** |
| **Pro** | **¥129** | 129 | 15M | ¥13.50 | ¥6.75 | **89.5%** | **94.8%** |

> **70% GM 是地板**，实际可达 ~90%。安全垫留给：V4 Pro 混入（成本 ×3-4）、重度用户、价格变动。

## 4. 前端展示逻辑

### 4.1 价格对比页 `/pricing`

**布局**（桌面 3 列并排，移动端**垂直堆叠 + Plus 卡片 sticky CTA**）：

```
┌──────────────────────────────────────────────────────────────┐
│                  选择适合你的方案                              │
│              [月付] 月付（年付暂未开放）                       │
│                                                                │
│   ┌──────────┐  ┌─────────────────┐  ┌──────────┐             │
│   │  Free    │  │  Plus    ⭐推荐 │  │   Pro    │             │
│   │          │  │                  │  │          │             │
│   │  ¥0      │  │  ¥49 / 月        │  │  ¥129/月 │             │
│   │          │  │  $9 / 月         │  │  $19/月  │             │
│   │          │  │                  │  │          │             │
│   │ • 5h     │  │ • 5h  600K     │  │ • 5h 2.5M│             │
│   │   100K   │  │ • 7d  4M       │  │ • 7d  15M│             │
│   │ • 7d     │  │                  │  │          │             │
│   │   400K   │  │  ━━━━━━━━━━━━━  │  │ ━━━━━━━━ │             │
│   │          │  │  深度研究首选     │  │ 重度无忧 │             │
│   │ [免费使用]│  │ [升级 Plus]     │  │ [升级 Pro]│             │
│   └──────────┘  └─────────────────┘  └──────────┘             │
│                                                                │
│  ❓ 常见问题                                                    │
│  Q: token 用量怎么算？   A: 输入+输出，按 DeepSeek 公开计费…     │
│  Q: 限额会重置吗？       A: 5h 滚动窗口，7d 滚动窗口…           │
└──────────────────────────────────────────────────────────────┘
```

**关键设计点**：
- Plus 卡片：青色发光边框 + 放大 1.03× + "推荐"角标
- 桌面端：3 列等宽并排
- 移动端（< 768px）：**垂直堆叠**（3 张卡顺序 Free → Plus → Pro），Plus 卡片底部 **sticky CTA**（"升级 Plus ¥49/月"按钮固定在 viewport 底部），便于用户**在浏览对比时一键升级**
- 顶部"月付"切换器**仅展示**（hover tooltip "年付即将推出"）
- 已有 design tokens（`--accent` 青色、`--radius-card`、`--shadow-glow`）可直接复用

### 4.2 用量仪表盘 `/settings/usage`

**布局**（两段式：实时大卡 + 历史趋势 + 智能建议）：

```
┌──────────────────────────────────────────────────────────────┐
│  用量与套餐                                          [升级 Plus]│
│                                                                │
│  当前套餐:  Free  →  Free 升级 Plus 解锁 10× 用量              │
│                                                                │
│  ┌──── 5 小时窗口 ──────────────────────────────┐             │
│  │  78,432 / 100,000 tokens                       │             │
│  │  ████████████████████░░░░░  78%                │             │
│  │  预计 2h 14m 后重置                             │             │
│  │  ⚠️  已超过软上限，建议控制节奏                  │             │
│  └────────────────────────────────────────────────┘             │
│                                                                │
│  ┌──── 7 天窗口 ──────────────────────────────┐             │
│  │  142,001 / 400,000 tokens                      │             │
│  │  ████████░░░░░░░░░░░░░░░░  35%                 │             │
│  │  预计 4d 9h 后重置                              │             │
│  └────────────────────────────────────────────────┘             │
│                                                                │
│  近 7 日用量趋势                                               │
│  ┌──────────────────────────────────────────────┐             │
│  │  ▁▂▃▅█▆▄    (折线图：每日 token 消耗)        │             │
│  │  06-01 06-02 ... 06-07                         │             │
│  └──────────────────────────────────────────────┘             │
│                                                                │
│  💡 升级 Plus 后预计可省 ¥0 (按当前用量，本月无需升级)         │
└──────────────────────────────────────────────────────────────┘
```

**关键设计点**：
- 双大数字卡（5h / 7d），双层进度条：浅色 = 已用，深色 = 软上限警示
- **实时倒计时**：基于滚动窗口内**最旧消耗点**计算重置时间
- 折线图：近 7 日每日 token 消耗
- 智能建议：根据近 30 天均值**预测本月是否需要升级**

### 4.3 对话内 80% / 95% 提示 toast（替代常驻顶栏计量器）

> **设计取舍**：明确**不**在 Workspace 顶栏放常驻计量器，保持工作区洁净。改用"决策点触达"：仅在跨过 80% / 95% 阈值时弹 toast。

**布局**（屏幕顶部居中，1 行 + 可选副行）：

```
┌──────────────────────────────────────────────────────────────┐
│  5h 用量已用 80%（80K / 100K）                            [×] │
│  还有 4h 32m 重置。  升级 Plus 解锁 6× 用量 →                  │
└──────────────────────────────────────────────────────────────┘
```

**触发规则**：
- 跨过 **80%** 阈值：浅琥珀色 toast，**每窗口只弹一次**，可关闭
- 跨过 **95%** 阈值：橙红色 toast，**更显眼**
- 跨过 **100%** 阈值：自动跳转 Paywall 页（见 4.4）

**关键设计点**：
- **不打扰对话流**（不像顶栏常驻）
- **到决策点才出现**（"决策点触达"原则）
- toast 关闭状态在 localStorage 记录（同一窗口不再弹）
- **localStorage 键名必须带 `user_id` 前缀**：`toast_dismissed_{user_id}_{window_type}_{threshold}`，例如 `toast_dismissed_user_abc_5h_80`。否则同一浏览器多账号切换时，A 账号关闭的 toast 会被 B 账号继承，造成混乱。

### 4.4 限流 Paywall `app/upgrade/paywall`

**布局**（模态/全屏，缩略版 3 档对比 + 倒计时）：

```
┌──────────────────────────────────────────────────────────────┐
│                                                                │
│         5h 用量已达上限                                         │
│         100,000 / 100,000 tokens                                │
│                                                                │
│         ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━                    │
│                                                                │
│  Free  →  Plus，解锁 10× 用量                                  │
│                                                                │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐  │
│  │  5h 100K       │  │  5h  600K     ⭐ │  │  5h  2.5M     │  │
│  │  7d 400K       │  │  7d  4M        │  │  7d  15M      │  │
│  │  ¥0            │  │  ¥49 / 月      │  │  ¥129 / 月    │  │
│  │                │  │  [升级 Plus]    │  │  [升级 Pro]   │  │
│  │  [继续 Free]   │  │                │  │                │  │
│  └────────────────┘  └────────────────┘  └────────────────┘  │
│                                                                │
│  4h 50m 后限额自动重置                                          │
└──────────────────────────────────────────────────────────────┘
```

**关键设计点**：
- **痛点时刻嵌入 3 档对比**（缩略版），让用户在情绪点上直接看到 Plus 价值
- "继续 Free" 按钮**置于左侧次要位置**（不强制升级）
- 显示**重置倒计时**（如果用户想等）
- 升级按钮直接跳 Creem / Stripe / Alipay checkout（按 user.region 路由）
- **复用 `<UsageMeter variant="compact" />`** 嵌入 paywall 顶部的"已用/限额"展示与倒计时，**不要单独维护一套倒计时逻辑**（避免状态不一致）

## 5. 关键组件清单

| 组件 | 路径 | 复用 |
|------|------|------|
| `<UsageMeter variant="full\|compact" />` | `components/billing/UsageMeter.tsx` | full 用于 4.2 仪表盘双卡，compact 用于 4.4 paywall 顶部 |
| `<PricingCards currentPlan highlightTier compact? />` | `components/billing/PricingCards.tsx` | full 用于 4.1 价格页，compact 用于 4.4 paywall |
| `<PaywallModal reason="5h\|7d" />` | `components/billing/PaywallModal.tsx` | 用于 4.4 paywall |
| `<UsageForecastCard />` | `components/billing/UsageForecastCard.tsx` | 用于 4.2 仪表盘底部 |
| `<UsageWarningToast threshold={80\|95} />` | `components/billing/UsageWarningToast.tsx` | 用于 4.3 对话内 toast |
| `<UsageTrendChart days={7} />` | `components/billing/UsageTrendChart.tsx` | 用于 4.2 折线图 |

> 全部放 `components/billing/` 命名空间，CSS Modules 跟随组件。

## 6. 后端实现

### 6.1 数据库迁移

**`avrag-rs/migrations/0037_pricing_revamp.up.sql`**：

```sql
-- 更新 quota_limits：刷新 5h / 7d 限额
-- 注：原 schema 已有 plan_id, metric_type, soft_limit, hard_limit
-- 本次 metric 改为按 token 计量（仅 llm_input_tokens + llm_output_tokens）

INSERT INTO quota_limits (plan_id, metric_type, soft_limit, hard_limit) VALUES
    ('free', 'pages_processed', 100, 500),         -- 容量不变
    ('free', 'embedding_tokens', 100000, 500000),  -- 容量不变
    ('free', 'storage_bytes', 1073741824, 5368709120),  -- 容量不变
    ('free', 'llm_input_tokens', 50000, 100000),   -- 容量不变
    ('free', 'llm_output_tokens', 25000, 50000),   -- 容量不变
    ('plus', 'pages_processed', 5000, 10000),       -- 容量不变
    ('plus', 'embedding_tokens', 5000000, 10000000),-- 容量不变
    ('plus', 'storage_bytes', 5368709120, 10737418240),  -- 容量不变
    ('plus', 'llm_input_tokens', 500000, 1000000),  -- 容量不变
    ('plus', 'llm_output_tokens', 250000, 500000)   -- 容量不变
ON CONFLICT (plan_id, metric_type) DO UPDATE
SET soft_limit = EXCLUDED.soft_limit, hard_limit = EXCLUDED.hard_limit;

-- 5h 滚动限额（新增 policy 表，已存在 usage_limit_plan_policies）
INSERT INTO usage_limit_plan_policies (plan_id, rolling_5h_limit_units, rolling_7d_limit_units) VALUES
    ('free',  100000,    400000),
    ('plus',  600000,    4000000),
    ('pro',   2500000,   15000000)
ON CONFLICT (plan_id) DO UPDATE
SET rolling_5h_limit_units = EXCLUDED.rolling_5h_limit_units,
    rolling_7d_limit_units = EXCLUDED.rolling_7d_limit_units;
```

> **设计取舍**：保持 pages / embedding_tokens / storage_bytes 数值**不变**（容量三档相同）；本次只调整 usage_limit_plan_policies 中的 token 滚动限额。llm_input_tokens / llm_output_tokens 的 quota_limits 保留作为**月度计费**维度（防止超量），但**主要限制**走 usage_limit 滚动窗口。

### 6.2 配置更新（`crates/billing/src/types.rs`）

```rust
impl BillingConfig {
    pub fn from_env() -> Self {
        Self {
            // ...existing fields...

            // Plus 价格（双币种）
            billing_price_label_plus: std::env::var("BILLING_PRICE_LABEL_PLUS")
                .unwrap_or_else(|_| "¥49 / 月 · $9 / 月".to_string()),

            // Pro 价格（双币种）
            billing_price_label_pro: std::env::var("BILLING_PRICE_LABEL_PRO")
                .unwrap_or_else(|_| "¥129 / 月 · $19 / 月".to_string()),

            // Alipay 价格同步
            alipay_price_plus: std::env::var("ALIPAY_PRICE_PLUS")
                .unwrap_or_else(|_| "49.00".to_string()),
            alipay_price_pro: std::env::var("ALIPAY_PRICE_PRO")
                .unwrap_or_else(|_| "129.00".to_string()),
            // ...
        }
    }
}
```

### 6.3 新增端点

| 端点 | 方法 | 用途 |
|------|------|------|
| `/api/billing/plans` | GET | 返回三档完整定义（含 quotas、price_label）|
| `/api/billing/usage/window` | GET | 返回当前用户 5h/7d 实时用量 + 限额 + **`reset_at` 时间戳** |
| `/api/billing/usage/history` | GET | 返回近 N 日每日 token 消耗（折线图数据）|
| `/api/billing/usage/forecast` | GET | 返回"按当前用量，本月是否需要升级"建议 |

**`/api/billing/usage/window` 返回结构契约**：

```typescript
type UsageWindow = {
  plan_id: "free" | "plus" | "pro";
  // 5h 滚动窗口
  rolling_5h: {
    used: number;          // 已用 token
    limit: number;         // 限额
    percentage: number;    // 0-100
    reset_at: string;      // ISO 8601：基于窗口内最旧消耗点计算的"重置时间"
    // 例："2026-06-07T18:23:00Z"
  };
  // 7d 滚动窗口
  rolling_7d: {
    used: number;
    limit: number;
    percentage: number;
    reset_at: string;
  };
  // 软/硬限位标识（用于 toast 触发判断）
  soft_limit_hit: { rolling_5h: boolean; rolling_7d: boolean };  // ≥80%
  hard_limit_hit: { rolling_5h: boolean; rolling_7d: boolean };  // =100%
};
```

> `reset_at` 由**后端**计算（基于窗口内最旧消耗事件的时间戳 + 窗口宽度），前端直接显示倒计时，避免在前端做时区/边界判断。前端只做 `Date(reset_at) - Date.now()` 的简单差值。

> 详细 contract 见后续 writing-plans 阶段输出。

## 7. 前端实现

### 7.1 新增路由

| 路径 | 文件 | 用途 |
|------|------|------|
| `/pricing` | `app/(marketing)/pricing/page.tsx` | 价格对比页 |
| `/settings/usage` | `app/settings/usage/page.tsx` | 用量仪表盘 |
| `/upgrade/paywall` | `app/upgrade/paywall/page.tsx` | 限流拦截落地页 |
| `/upgrade/success` | `app/upgrade/success/page.tsx` | 升级成功页 |

### 7.2 复用现有资源

- 字体：Space Grotesk（标题）+ IBM Plex Sans（正文）+ JetBrains Mono（数字）
- 设计令牌：`--accent`（青色）/ `--radius-card` / `--shadow-glow` / `--font-mono`
- 布局：复用 dashboard 顶部栏组件模式
- i18n：所有文案走 `next-intl`，新增 `pricing.*`、`usage.*`、`paywall.*` 词条

### 7.3 国际化文案（中文为默认）

```yaml
# messages/zh-CN/billing.json
{
  "pricing.title": "选择适合你的方案",
  "pricing.subtitle": "所有档位容量相同，按月用量限额划分",
  "pricing.monthly": "月付",
  "pricing.yearlySoon": "年付即将推出",
  "pricing.tier.free.name": "Free",
  "pricing.tier.plus.name": "Plus",
  "pricing.tier.pro.name": "Pro",
  "pricing.tier.plus.badge": "推荐",
  "pricing.tier.plus.tagline": "深度研究首选",
  "pricing.tier.pro.tagline": "重度无忧",
  "pricing.faq.token": "token 用量怎么算？",
  "pricing.faq.tokenAnswer": "输入 + 输出按 DeepSeek 公开计费标准累计",
  "pricing.faq.reset": "限额会重置吗？",
  "pricing.faq.resetAnswer": "5h 滚动窗口，7d 滚动窗口",

  "usage.title": "用量与套餐",
  "usage.window5h": "5 小时窗口",
  "usage.window7d": "7 天窗口",
  "usage.estimatedReset": "预计 {time} 后重置",
  "usage.softLimitWarning": "已超过软上限，建议控制节奏",
  "usage.forecastSave": "升级 Plus 后预计可省 {amount}",
  "usage.forecastStay": "按当前用量，本月无需升级",

  "paywall.title5h": "5h 用量已达上限",
  "paywall.title7d": "7d 用量已达上限",
  "paywall.subtitle": "Free → Plus，解锁 10× 用量",
  "paywall.continueFree": "继续 Free",
  "paywall.resetIn": "{time} 后限额自动重置"
}
```

## 8. 测试与验证

### 8.1 后端
1. **migration 回滚测试**：0037 up / down 双向可逆
2. **quota 计算测试**：模拟用户消耗，验证 5h/7d 窗口判断正确
3. **跨档升级测试**：Free→Plus 后立即生效新限额
4. **降级测试**：订阅过期 → Lazy Billing Downgrade → Free 限额生效
5. **价格 label 端点测试**：`/api/billing/plans` 返回双币种

### 8.2 前端
1. **可视化回归**：用 Playwright 截图 `/pricing`、`/settings/usage`、`/upgrade/paywall` 三页（桌面 + 移动）
2. **E2E 流程**：
   - Free 用户跑满 5h 限额 → 触发 toast → 跳 paywall → 选 Plus → 完成支付 → 限额更新
3. **状态机测试**：80% / 95% / 100% 阈值 toast 行为（用 mocked API 模拟）
4. **i18n 完整性**：所有新增词条在 zh-CN / en-US 均填充
5. **暗色模式**：用 `data-theme="dark"` 验证 4 个页面对比度
6. **响应式**：320px / 768px / 1280px 三档断点

### 8.3 经济模型验证
1. 用 mock LLM cost 跑 1000 个模拟用户 → 验证总毛利率 ≥ 70%
2. 极端场景：100% 用户用 V4 Pro → 验证毛利率仍 ≥ 70%（需要 V4 Pro 加价方案，本设计**不包含**，留 v2 设计跟进）
3. 重度用户识别：监控 7d 用量 > 12M 的用户比例（应 < 5%）

## 9. 范围外（明确排除）

以下内容**不在本设计范围内**，作为后续迭代项：

- **年付方案**（UI 留位但暂不开放）
- **V4 Pro 高级模型加价**（仅 Flash 在本设计内）
- **企业版 / 团队席位**（个人知识工作者 1 人 1 份原则）
- **Referral / 邀请返利**（不与 Moonshot/Kimi Code 直接对标，留作 growth 单独设计）
- **学生 / 教育优惠**
- **动态定价**（基于地理/收入/使用模式）
- **A/B 测试框架**

**冷却期约定**：本设计上线后 **4 周内不启动任何上述排除项的讨论与设计**。原因：本设计的核心 KPI（Free→Plus 转化率）需要至少 4 周数据才能跑出统计显著性，在此期间并行启动新功能设计会分散注意力、污染实验数据。4 周后基于实际数据再评估。

## 10. 开放问题（需后续对齐）

| 问题 | 备选 | 建议 |
|------|------|------|
| V4 Pro 是否在 Plus/Pro 档开放？ | A. 仅 Flash（推荐）/ B. 限额开放 Pro / C. 单独 V4 Pro add-on | 选 A，V4 Pro 留 v2 |
| Free 用户过 quota 后是否还能用"低质量"模式？ | A. 硬限（推荐）/ B. 降级到 V4 Flash-lite 仍可用 | 选 A，强化升级信号 |
| Paywall 跳 Creem vs Stripe vs Alipay 路由逻辑 | A. 按 IP 地理位置 / B. 按用户上次支付渠道 / C. 用户手动选 | 选 A + 提供切换器 |
| 现有 0035/0036 migration 是否需要合并重写？ | A. 追加 0037（推荐）/ B. 重写 0035 | 选 A，保持迁移单调 |
| 用量历史折线图后端存储 | A. 新建表（推荐）/ B. 复用 llm_usage_events 聚合 | **选 B（渐进策略）**：先按 `date_trunc('day', created_at)` 在 `llm_usage_events` 上聚合（有 `user_id + created_at` 索引时 100K 日活 < 10ms），上线后第一周监控 P99 latency；若 P99 > 200ms 再迁移到 A（新建预聚合表） |

## 11. 时间线（粗）

| 阶段 | 周期 | 交付物 |
|------|------|--------|
| 后端 migration + 端点 | 1 周 | 0037 up, /api/billing/* 4 端点, 单测 |
| 前端组件库 | 1 周 | 6 个组件（UsageMeter 等）+ Playwright 截图基线 |
| 4 个页面路由 | 1 周 | pricing / usage / paywall / success |
| E2E + i18n + 暗色 | 1 周 | Playwright + next-intl + 视觉回归 |
| 灰度 + 上线 | 1 周 | 10% → 50% → 100% |

总计 **5 周**。

## 12. 引用

- Kimi Code Coding Plan（参考）— 国内 token 订阅标杆
- Moonshot token plan（参考）— 5h/7d 滚动限额模型源头
- DeepSeek 公开定价 — https://api-docs.deepseek.com/zh-cn/quick_start/pricing/
- 现有 spec：`docs/superpowers/specs/2026-03-30-single-user-usage-limit-design.md`（usage_limit 滚动窗口机制）
- 现有 design：`frontend_next/docs/superpowers/plans/2026-04-27-visual-overhaul.md`（Precision Lab 视觉基线）
