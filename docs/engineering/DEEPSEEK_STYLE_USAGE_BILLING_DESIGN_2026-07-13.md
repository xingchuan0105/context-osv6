# DeepSeek 风格用量计费设计（输入 / 输出 / 缓存命中）

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-13 |
| 状态 | **Design frozen (v2)** — 决策已批；实现按 [DEV_PLAN](./DEEPSEEK_USAGE_BILLING_DEV_PLAN_2026-07-13.md) 推进中 |
| 约束 | Solo 本地 trunk；B2C `user_id`；`workspace` 范围；支付仅 Creem + Alipay；**价格不变** |
| 开发计划 | [DEEPSEEK_USAGE_BILLING_DEV_PLAN_2026-07-13.md](./DEEPSEEK_USAGE_BILLING_DEV_PLAN_2026-07-13.md)（**Ready**，Wave 0–5） |
| 相关 | [ADR 0001](../adr/0001-user-level-billing-b2c.md)、[exit metering](../../avrag-rs/docs/superpowers/specs/2026-07-05-llm-usage-exit-metering-design.md)、[Pricing tiers revamp](../superpowers/specs/2026-06-07-pricing-tiers-revamp-design.md)、[Chrome+Billing 计划](./PRODUCT_UI_CHROME_AND_BILLING_DEV_PLAN_2026-07-13.md)、[STRIPE 移除](./STRIPE_BILLING_REMOVAL_2026-07-13.md) |
| 官方参考 | [DeepSeek Models & Pricing](https://api-docs.deepseek.com/quick_start/pricing/) |

---

## 0. 一句话目标

把 5h / 7d 滚动配额做成 **DeepSeek 三桶成本同构** 的透明计量，再乘 **分档毛利乘数 M**（对用户可见）：

> **缓存命中输入 ≪ 缓存未命中输入 ≪ 输出（相对约 0.02 : 1 : 2）**  
> → `raw` → **`usage_units = ceil(raw × M_plan)`**  
> → 限额按 **「约 XX tokens」产品承诺 × M** 倒推进库。

**价格（Plus ¥49 / $9、Pro ¥129 / $19）不变**；**限额数字按本设计重算**。

---

## 1. 现状审计（代码真相）

### 1.1 主路径（产品滚动配额）

```text
LlmClient / EmbeddingClient
  → UsageObserver::record_chat / record_embedding
  → PgUsageObserver
  → insert_llm_usage_event → compute_usage_units_with_rates (两桶)
  → llm_usage_events (billable)
  → sum(usage_units) → 5h / 7d
```

| 组件 | 路径 | 角色 |
|------|------|------|
| 出口钩子 | `llm/.../client/mod.rs` | Prometheus 已拿 `cached_tokens` |
| `ChatUsageRecord` | `llm/src/usage_observer.rs` | **无** `cached_tokens` |
| 折算 | `app-core/billing_usage_units.rs` | 两桶：`ceil(in/1k·r_in + out/1k·r_out)` |
| 权重 | `llm_model_weights` | 仅 input/output rate |
| 窗口 | `billing_sql/core_usage.rs` | `llm_usage_events` + plan policy |
| Worker | `TaskTenantUsageObserver` | `billable=false`（保持） |

### 1.2 当前公式与历史口径错位

```text
units = ceil( prompt/1000 * input_rate + completion/1000 * output_rate )
// 默认 input_rate=1, output_rate=2
```

| 层 | 口径 | 问题 |
|----|------|------|
| 产品文档 (2026-06-07) | Free 5h = **100,000 tokens** | 对用户承诺「约 token」 |
| DB `usage_limit_plan_policies` | Free 5h = **100_000 units** | 数字拷了 token，但 units 是「每 1k token 一档」 |
| 扣费公式 | 1k miss input → **1 unit** | 同一数字下，实现额度 ≫ 文档体感 |

**本设计一并纠正：**  
- 产品侧只承诺 **「约 XX tokens」**（沿用 06-07 档位跳变）。  
- 库内 `rolling_*_limit_units` = **倒推后的 usage_units**（见 §5），不再把 token 数原样写入 units 列。

### 1.3 Cache 断层

| 层 | cached？ |
|----|----------|
| Provider / `LlmUsage` / Prometheus | ✅ |
| `ChatUsageRecord` → 账本 → 公式 | ❌ 静默丢掉 |

### 1.4 今日 DB 限额（migration 0037，**将被本设计替换**）

| plan | 5h units（旧） | 7d units（旧） |
|------|----------------|----------------|
| free | 100_000 | 400_000 |
| plus | 600_000 | 4_000_000 |
| pro | 2_500_000 | 15_000_000 |

### 1.5 已完成相关修复（勿回退）

| ID | 内容 | 状态 |
|----|------|------|
| B-U-1 | `llm_usage_events` RLS 读写 `set_current_user` | Done |
| B-U-2 | 账单页去掉 token/doc「未设置」双轨 | Done |
| B-U-3 | 用量单位副文案 | Done（B2 将按本设计改写） |

### 1.6 支付边界

Creem + Alipay only；**不改价格与 checkout**；只改计量与限额语义。

---

## 2. DeepSeek 成本模型（外部真相）

| 桶 | deepseek-v4-flash（USD / 1M） | 相对 miss=1 |
|----|------------------------------|-------------|
| Input cache hit | $0.0028 | **≈ 0.02** |
| Input cache miss | $0.14 | **1.0** |
| Output | $0.28 | **2.0** |

模型权重表默认 / fallback：`rate_miss=1.0`，`rate_cache=0.02`，`rate_out=2.0`。  
贵模型按上游比例写 `llm_model_weights`，业务代码不硬编码模型名。

---

## 3. 目标公式

### 3.1 三桶 raw + 分档 M

```text
cached = min(cached_tokens, prompt_tokens)
miss   = prompt_tokens - cached
out    = completion_tokens

raw = miss/1000   * rate_miss
    + cached/1000 * rate_cache
    + out/1000    * rate_out

M = M_plan(user)   // free | plus | pro，见 §4

usage_units =
  if prompt==0 && completion==0 then 0
  else max(1, ceil(raw * M))
```

**写入时**按用户当前 **active plan** 取 M（与 enforcement 查 plan 同源：`subscriptions` → plan_id，缺省 free）。

### 3.2 边界规则

| 规则 | 决策 |
|------|------|
| `cached > prompt` | clamp |
| Provider 无 cache 字段 | `cached=0`（退化为两桶） |
| Embedding | `completion=0, cached=0`；用 embedding 权重行 |
| Worker | `billable=false`，不进 5h/7d |
| 本地 CompletionCache / Redis embed hit | 不调 API → 不计费 |
| Provider prompt cache hit | **计费**，低 `rate_cache` |
| 历史行 | 不回算；部署日起新公式 + 新限额 |

### 3.3 权重表扩展

| 列 | 含义 | Flash 默认 |
|----|------|------------|
| `input_unit_rate` | miss 输入 | 1.0 |
| `cache_hit_unit_rate` | hit 输入 | 0.02 |
| `output_unit_rate` | 输出 | 2.0 |

---

## 4. 分档毛利乘数 M（透明、已定）

### 4.1 数值

| plan | M | 含义（用户可见话术） |
|------|---|----------------------|
| **free** | **2.0** | 免费档方案乘数 2.0（覆盖基础设施与获客；未在用户消息中单独点名时的默认补全） |
| **plus** | **1.5** | 用户明确指定 |
| **pro** | **1.3** | 用户明确指定 |

> **Free = 2.0** 与 Plus/Pro 形成递减阶梯：付费越贵，单位成本加成越低。若产品要改 Free 乘数，只改表配置 + 重算 §5 限额，公式不变。

### 4.2 存储与读取

- 列建议：`usage_limit_plan_policies.margin_multiplier DOUBLE PRECISION NOT NULL`  
  - free=2.0, plus=1.5, pro=1.3  
- **禁止** 仅 env 全局一个 M（与「分档不同」冲突）。  
- 扣费路径：`insert_llm_usage_event` 时 `get_user_plan` → `load_plan_policy.margin_multiplier`。  
- 用户覆盖（`usage_limit_user_overrides`）：可只覆盖 limit；M 仍跟 plan（除非未来加 override 列——**本期不做**）。

### 4.3 必须对用户透明

产品/设置/定价 **明示** M，不藏「黑箱加成」：

| 表面 | 要求 |
|------|------|
| `/pricing` | 每档卡片：限额 **约 X tokens / 5h**、**约 Y tokens / 7d**；脚注或次行：**用量按输入（含缓存命中优惠）与输出折算后 × 方案乘数 M=…** |
| Settings 账单 / 用量 | 当前方案 M；进度主文案用 **约 tokens**（见 §6） |
| 帮助 / 法律旁说明（可选短链） | 一页说明三桶 + M + 「约」的含义 |
| API `UsageWindowResponse` | 增加 `margin_multiplier: number`、`limit_tokens_approx_5h` / `7d`（或文档约定由前端 `limit_units/M*1000` 推） |

**示例文案（zh）：**

> 5 小时额度约 **600,000 tokens**（Plus）。  
> 实际扣减：将模型返回的输入（区分缓存命中 / 未命中）与输出折成成本单位后，再乘方案乘数 **1.5**。缓存命中消耗远低于新输入；输出按约 2 倍输入计价。  
> 「约 tokens」以「全为缓存未命中输入」为参照；真实对话因缓存与输出比例会有偏差。

---

## 5. 限额倒推（价格不变，数字重算）

### 5.1 产品承诺：约 tokens（沿用 2026-06-07 跳变，**不变**）

| 档位 | 5h 约 tokens \(T_5\) | 7d 约 tokens \(T_7\) | 价格 |
|------|----------------------|----------------------|------|
| Free | 100,000 | 400,000 | ¥0 |
| Plus | 600,000 | 4,000,000 | ¥49 / $9 |
| Pro | 2,500,000 | 15,000,000 | ¥129 / $19 |

跳变叙事保持：Free→Plus 5h **6×** / 7d **10×**；Plus→Pro 5h **≈4.2×** / 7d **3.75×**。

### 5.2 参照定义（「约 tokens」的唯一工程定义）

在 **Flash 默认 rate** 且 **全部为 cache-miss 输入、无输出** 时：

```text
raw = T / 1000          // rate_miss = 1
usage_units = ceil(raw * M) = ceil(T / 1000 * M)
```

故倒推：

```text
rolling_*_limit_units = ceil(T_approx / 1000 * M_plan)
```

等价：用户用尽额度时，**若用法恰好全是 miss 输入**，消耗的 token 数 **≈ T_approx**。  
有输出 / 有 cache 时，可支撑的「字面 token 数」会变（cache 让同等额度撑更久；输出让额度更快耗尽）——故文案永远带 **「约」**。

### 5.3 重算后的 `usage_limit_plan_policies`（目标值）

| plan | M | 5h 约 tokens | **5h limit_units** | 7d 约 tokens | **7d limit_units** |
|------|---|--------------|--------------------|--------------|--------------------|
| free | 2.0 | 100,000 | **ceil(100×2.0) = 200** | 400,000 | **ceil(400×2.0) = 800** |
| plus | 1.5 | 600,000 | **ceil(600×1.5) = 900** | 4,000,000 | **ceil(4000×1.5) = 6,000** |
| pro | 1.3 | 2,500,000 | **ceil(2500×1.3) = 3,250** | 15,000,000 | **ceil(15000×1.3) = 19,500** |

与旧 DB（10⁵～10⁷ 量级 units）相比：**数量级回到与公式一致**；旧值是「把 token 数误当 units」的历史债，实现 wave 用 migration **覆盖** policy 行。

### 5.4 反向展示（used → 约 tokens）

```text
tokens_approx_used  = used_units / M * 1000
tokens_approx_limit = limit_units / M * 1000   // 应等于产品 T_*（舍入内）
percentage          = used_units / limit_units  // 进度条仍按 units 比，与约 tokens 比一致
```

### 5.5 示意扣费（Plus，M=1.5）

对话：prompt 20k（hit 16k）、out 2k：

```text
raw = 4*1 + 16*0.02 + 2*2 = 8.32
units = ceil(8.32 * 1.5) = 13
≈ tokens 参照消耗 = 13/1.5*1000 ≈ 8,667（相对「纯 miss 输入」）
```

同对话若 **忽略 cache**（旧两桶当全 miss）：`raw=20+4=24` → `ceil(36)=36` units，多扣约 **2.8×**。

### 5.6 与 70% GM 文档的关系

[Pricing revamp §3](../superpowers/specs/2026-06-07-pricing-tiers-revamp-design.md) 用混合成本 ~0.9 元/M tokens 估月成本。  
引入 **透明 M** 后：用户侧额度按 **约 tokens** 不变；平台在 **单位扣费上** 已含 M，固定成本 / embed / CAC 由 M 覆盖。  
**不在本设计重算订阅价**；若实跑 GM 低于地板，优先调 **Free M 或约 tokens 承诺**，不动 Plus/Pro 标价（除非另开定价 wave）。

---

## 6. 产品展示契约（唯一 token 叙事）

限额在产品和文档里 **只讲一种用户语言：约 tokens**。  
内部账本仍是 **单一 `usage_units` 列**（不是第二套 token 配额表）。

| 位置 | 主文案 | 次文案 |
|------|--------|--------|
| 用量条 | `约 {used} / {limit} tokens`（5h 与 7d 各一条） | `含方案乘数 M={m}；缓存命中更省` |
| 定价页 | `5 小时约 {T5} tokens` · `7 天约 {T7} tokens` | `折算后 × M={m}` |
| 触顶 toast / paywall | 同上约 tokens | 升级后 M 更低 + 约 tokens 更高 |
| 工程 / Admin | 可显示 raw units | 非用户主路径 |

**禁止** 再主推「用量单位」而不给约 tokens；「用量单位」可留在高级说明一句。  
**禁止** 恢复 token/doc 双轨 +「未设置」容量条作为账单主 UI。

---

## 7. 差距矩阵

| ID | 差距 | 计划波次 | 优先级 |
|----|------|----------|--------|
| G1–G5 | cached 贯通 + 三桶公式 + weights 列 | Wave 1–2 | P0 |
| G12 | 分档 `margin_multiplier` + insert 时乘 M | Wave 1–2 | P0 |
| G13 | policy 限额改为 §5.3 倒推值 | Wave 1 | P0 |
| G14 | API/前端约 tokens + 展示 M | Wave 3–4 | P0 |
| G15 | 定价页 / 帮助文案同步 | Wave 4–5 | P0 |
| G6 | 历史 units 不回算 | 文档 | P1 |
| G8 | `usage_events` 双表不绑主 UI | Wave 6 可选 | P2 |
| G10 | DeepSeek 权重种子 | Wave 1 | P1 |
| G11 | 导出含 cached / M | Wave 5 | P2 |

---

## 8. 实现波次

**权威编排：** [DEEPSEEK_USAGE_BILLING_DEV_PLAN_2026-07-13.md](./DEEPSEEK_USAGE_BILLING_DEV_PLAN_2026-07-13.md)

```text
Wave 0  文档门禁
Wave 1  Schema（cached / M / 倒推 limits）
Wave 2  三桶公式 + 出口 cached + insert×M
Wave 3  usage/window API（M + 约 tokens）
Wave 4  前端透明展示
Wave 5  测试硬化 + pricing errata
Wave 6  （可选）usage_events 收敛
```

摘要：B1≈计划 Wave 1–2；B2≈Wave 3–4；B3≈Wave 6。细节、文件清单、验收命令以开发计划为准。

---

## 9. 非目标

- 预付人民币 token 钱包 / 实时挂牌扣费。  
- Stripe 回归。  
- Org 团队池。  
- Worker 入库 embedding 改 billable。  
- 回算历史 `usage_units`。  
- 改 Plus/Pro **标价**（本设计明确不变）。

---

## 10. 决策记录（v2 已批）

| # | 议题 | 决议 | 状态 |
|---|------|------|------|
| D1 | 三桶相对 Flash | hit:miss:out = **0.02 : 1 : 2** | **Accepted** |
| D2 | 毛利 | **分档全局 M，对用户透明**：free **2.0** / plus **1.5** / pro **1.3** | **Accepted** |
| D3 | 历史 | 部署日起新公式，**不回算** | **Accepted** |
| D4 | Plan 限额 | **按约 tokens × M 倒推重算**；约 tokens 档位保持 06-07；**价格不变** | **Accepted**（修正原「不动数字」） |
| D5 | Worker | non-billable | **Accepted** |
| D6 | UI 主轴 | 5h/7d；用户语言 **约 tokens** + 明示 M | **Accepted** |
| D7 | Free M | **2.0**（阶梯默认；可配置） | **Accepted（设计默认）** |

---

## 11. 与既有文档关系

| 文档 | 关系 |
|------|------|
| 2026-06-07 pricing revamp | **约 tokens 与价格** 以该文为准；**units 列与公式** 以本文 §5 替换其「units=token 数字」实现假设 |
| exit-metering 2026-07-05 | 出口钩子保留；本文补 cache + M |
| Chrome+Billing 计划 | B-U-4 → 本文；不阻塞 Wave 1 AccountMenu |
| STRIPE 移除 | 支付通道无关 |

**实现时**应回写 pricing revamp 或加 errata：「滚动限额入库为 usage_units=ceil(T/1000×M)，展示约 T tokens」。

---

## 12. 验收清单（实现后）

- [ ] 同 raw，Plus units = ceil(raw×1.5)，Pro = ceil(raw×1.3)，Free = ceil(raw×2.0)。  
- [ ] 二次相同 prompt 且 `cached_tokens>0` 时 units **明显低于** 全 miss。  
- [ ] DB policy：free 200/800，plus 900/6000，pro 3250/19500（± 实现取整一致）。  
- [ ] 定价页与设置页均出现 **约 tokens** 与 **M=…**。  
- [ ] `tokens_approx_limit` 与 §5.1 表一致（舍入内）。  
- [ ] 账单无 token/doc「未设置」双轨。  
- [ ] Worker billable=false 不增加 used。  
- [ ] 价格文案仍为 Plus ¥49/$9、Pro ¥129/$19。

---

## 13. 附录：代码锚点

| 主题 | 文件 |
|------|------|
| 公式 | `avrag-rs/crates/app-core/src/billing_usage_units.rs` |
| Record | `avrag-rs/crates/llm/src/usage_observer.rs` |
| 出口 | `avrag-rs/crates/llm/src/client/mod.rs` |
| Observer | `avrag-rs/crates/app-billing/src/usage_observer_impl.rs` |
| 落库 / rates | `avrag-rs/crates/app-bootstrap/src/adapters/pg_usage_limit_store.rs` |
| 窗口 | `.../billing_sql/core_usage.rs` |
| 旧限额 seed | `migrations/0037_pricing_revamp.up.sql` |
| 产品约 tokens 源 | `docs/superpowers/specs/2026-06-07-pricing-tiers-revamp-design.md` §2.1 |

---

*End of design v2. Implementation = [DEV_PLAN](./DEEPSEEK_USAGE_BILLING_DEV_PLAN_2026-07-13.md) after explicit「开工」.*
