# ADR 0003: 桌面端混合商业模式——SaaS 订阅 + Desktop 软件许可

## Status

Proposed

## Context

当前产品形态是 VPS 部署的 SaaS：用户按月订阅（Free / Plus / Pro），服务器侧托管 LLM 调用、RAG 检索、文档摄取，按 token 计量配额。已有桌面客户端骨架（Tauri 2 + `frontend_next` 静态导出 + `storage-local`），但未接入 LLM，无收入路径。

SaaS 模式有三个结构性矛盾：

1. **高并发瓶颈**：单台 VPS 承载全部 LLM 调用，算力在服务器侧，扩展靠堆机器，成本线性增长。
2. **数据隐私**：用户的私有文档存在服务器上，部分行业/地区客户无法接受。
3. **Desktop BYOK 拆台**：桌面端让用户自带 LLM API key，本意是解决 1 和 2，但 BYOK 绕过了 token 计费——用户付钱给 LLM 厂商，项目零收入。

根本张力：**用 token 计费，但用户一旦自带 key，"管道"价值归零。** Desktop BYOK 不是 SaaS 的补充，而是绕过。

## Decision

**采纳混合商业模式（方案 A）：Desktop 卖软件许可，SaaS 卖托管便利，两者价值主张正交。**

### 双轨定位

| 维度 | SaaS（VPS） | Desktop（Tauri） |
|------|-------------|-----------------|
| 算力来源 | 服务器付 | 用户付（BYOK） |
| 数据归属 | 服务器存 | 用户本地 |
| 并发上限 | 受 VPS 限制 | 天然分散（算力在 LLM 厂商侧） |
| 配置门槛 | 低（开箱即用） | 高（需配 API key，通过引导流降低） |
| 收入来源 | 月度订阅（Plus ¥49/月、Pro ¥39/月） | 软件许可买断（Standard $39/¥299、Pro $99/¥699） |
| 目标用户 | 不想折腾的人 | 在乎隐私/成本/离线的进阶用户 |

两者卖的"价值"不同，不再互相拆台：
- Desktop 卖的是**软件本身**（私有化部署能力 + 隐私 + 无 token 焦虑）
- SaaS 卖的是**托管服务**（省心 + 多端同步 + 团队协作）

### 授权模式决策

| 决策点 | 选择 | 理由 |
|--------|------|------|
| 收费模式 | **买断 + 大版本免费升级** | `expires_at = NULL`；运维最轻，用户心理最友好；v1.x 终身免费，v2 再付费 |
| 试用 | **7 天全功能试用** | 降低购买门槛；`device_id` 防重复试用 |
| 设备数 | Standard 1 台 / Pro 3 台 | 浮动许可，可解绑再绑 |
| 防滥用 | **Keygen CE 自托管** | 复用现有 VPS + Postgres + Redis；Tauri 官方支持；Rust SDK 现成 |

### 技术现状盘点

| 能力 | 现状 | 复用度 |
|------|------|--------|
| JWT 签发/验证 | `transport-http/lib_impl/router_core.rs` `issue_jwt_for_auth_version` | 已就位 |
| `subscriptions` 表 | migration 0035，已带 `user_id`、`plan_id`、`status` | 已就位（SaaS 侧） |
| Creem/支付宝 checkout | `billing/creem_client.rs` / `alipay_client.rs` 已跑通 | 加 product 分支复用 |
| Desktop transport 接缝 | `lib/runtime/transport.ts` 自动分叉 Web/IPC | 已就位 |
| Desktop 本地存储 | `storage-local`（LocalContentStore + LocalCache） | 已就位 |
| Desktop chat | placeholder，未接 LLM | **待补（WP4 核心工程量）** |
| Desktop 激活/license | 无 | **待补（Keygen CE）** |
| Desktop LLM provider 配置 | 无 | **待补** |

## Consequences

### 正面

- **不拆台**：Desktop 用户即使 BYOK 也付了软件许可费；不会回流 SaaS 抢 token 收入。
- **高并发问题消失**：BYOK 用户的算力在 LLM 厂商侧，VPS 只服务 SaaS 订阅者。
- **隐私市场可达**：数据不出本机，覆盖 SaaS 无法触达的客户。
- **沉没成本变现**：Tauri 工程、`storage-local`、transport 接缝已投入，桌面许可将其变现。
- **收入多元化**：不再单一依赖 token 差价，软件许可收入更稳定。

### 负面

- **双轨维护**：SaaS 和 Desktop 两条产品线，需同时维护。
- **license 运维**：Keygen CE 自托管需维护（Docker 容器，半年一次升级）。
- **Desktop LLM 兼容性**：需覆盖足够多 provider 才有吸引力（见 ADR 0004）。

### 关联

- `docs/adr/0005-llm-provider-protocol-architecture.md` — LLM provider 架构重构
- `docs/desktop-license-activation-design.md` — 授权与激活详细设计
- `docs/desktop-llm-provider-design.md` — LLM 兼容性与诊断设计
- `docs/desktop-frontend-pages-design.md` — 前端独立页面设计
- `docs/desktop-execution-plan.md` — 总执行计划（WP1-WP7）
- `docs/desktop-client-design-2026-06-11.md` — 现有桌面端架构设计