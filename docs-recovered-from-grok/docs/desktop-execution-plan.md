# 桌面端混合商业模式——总执行计划

> 日期：2026-07-08　状态：Proposed
> 决策依据：`docs/adr/0004-desktop-hybrid-business-model.md`

---

## 0. TL;DR

将桌面端从"骨架"推进为"可售卖产品"，通过软件许可（买断制）获取收入，与 SaaS 订阅形成正交双轨。底层 LLM 调用重构为四轴架构（移植 opencode 设计），覆盖 16+ provider。

**总工时：24-28 天**（含 LLM 四轴重构），分 7 个工作包。

---

## 1. 架构总览

```
┌─ SaaS (现有 VPS) ──────────────────────────────────────┐
│                                                         │
│  avrag-rs (Rust API) :8080     Keygen CE (Docker) :3001 │
│  ├ 用户/chat/RAG/billing       ├ License 发证/激活/心跳   │
│  ├ avrag-llm（重构后四轴架构）   ├ Postgres ←── 共享      │
│  ├ Postgres :5432 ←─────────── ├ Redis ←── 共享         │
│  └ Creem/支付宝 checkout        └ Ed25519 离线签名        │
│                                                         │
│  Next.js 前端 :3000                                     │
│  ├ (marketing)/desktop/buy   购买页                     │
│  ├ (account)/licenses        License 管理               │
│  └ (desktop)                 桌面端专属页面（静态导出）    │
│                                                         │
└─────────────────────────────────────────────────────────┘
          ↑ 系统浏览器跳转          ↑ Keygen API (心跳/验证)
          │                        │
┌─ Desktop (Tauri) ──────────────────────────────────────┐
│                                                         │
│  Tauri Shell (desktop/)                                 │
│  ├ (desktop)/activate    激活引导（独立页面）              │
│  ├ (desktop)/setup       LLM 配置引导（独立页面）          │
│  └ workspace             正常使用                         │
│                                                         │
│  Rust 核心 (desktop/src-tauri/)                         │
│  ├ commands/license.rs   keygen-rs + 本地 Ed25519 验签   │
│  ├ commands/llm_config.rs 读写配置 + 诊断                │
│  ├ commands/chat.rs      接 avrag-llm（四轴架构）         │
│  └ storage-local         本地存储                        │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

---

## 2. 工作包总览

| WP | 内容 | 工时 | 依赖 | 详细设计 |
|----|------|------|------|---------|
| WP1 | Keygen CE 部署 + 初始化 | 1d | 无 | `desktop-license-activation-design.md` §2 |
| WP2 | SaaS License 管理接口 | 1.5d | WP1 | `desktop-license-activation-design.md` §6 |
| WP3 | Desktop 激活 + 三层验证 + 深链 | 2d | WP1 | `desktop-license-activation-design.md` §3-5 |
| WP4 | LLM 四轴架构重构 | 12.5d | 无（独立） | `adr/0005` + `desktop-llm-provider-design.md` |
| WP5 | 前端独立页面 | 3d | WP2, WP3 | `desktop-frontend-pages-design.md` |
| WP6 | 收银（Creem/支付宝加 product） | 0.5d | WP2 | `desktop-license-activation-design.md` §6-7 |
| WP7 | 打包 + 深链注册 + 公钥嵌入 | 1d | WP3 | 本文档 §10 |
| **合计** | | **21.5d** | | |

> WP4（12.5d）是最大头且完全独立，可与 WP1-WP3 + WP5-WP7 并行推进。

---

## 3. WP1 — Keygen CE 部署（1d）

**目标**：在现有 VPS 上部署 Keygen CE，复用 Postgres + Redis。

**步骤**：
1. `CREATE DATABASE keygen;`（在现有 Postgres 实例上）
2. 编写 `docker-compose.keygen.yml`（web + worker，共用 host.docker.internal）
3. 配置 Nginx 反代 `/v1/* → :3001`
4. 运行 `keygen/api setup` 初始化
5. 创建 Product + 3 个 Policy（Pro / Standard / Trial）
6. 提取 `KEYGEN_PUBLIC_KEY`（`Account.sole.ed25519_public_key`）
7. 将所有 `KEYGEN_*` 环境变量写入 `avrag-rs/.env`

**验证**：
- `curl https://license.avrag.com/v1/ping` 返回 200
- 通过 console 创建测试 license，成功 validate

**交付物**：
- `docker-compose.keygen.yml`
- `avrag-rs/.env` 新增 `KEYGEN_*` 变量
- Keygen console 截图（Product / Policy 已创建）

---

## 4. WP2 — SaaS License 管理接口（1.5d）

**目标**：在 `avrag-rs` 新增 License 代理路由，代理到 Keygen CE API。

**改动文件**：
- 新增 `transport-http/src/routes/license.rs`
- 修改 `transport-http/src/routes/mod.rs`（注册路由）
- 修改 `transport-http/src/lib_impl/router_core.rs`（AppState 加 Keygen client）

**接口**：

| 路由 | 方法 | 说明 |
|------|------|------|
| `/api/v1/licenses/checkout` | POST | 购买（调 Creem/支付宝，成功后调 Keygen 创建 license） |
| `/api/v1/licenses/me` | GET | 当前用户的 license 列表（代理 Keygen） |
| `/api/v1/licenses/{id}/machines` | GET | 某 license 的已激活设备 |
| `/api/v1/licenses/{id}/machines/{mid}` | DELETE | 解绑设备 |
| `/api/v1/licenses/trial` | POST | 创建试用 license |

**验证**：
- 单元测试：mock Keygen client，测代理逻辑
- 集成测试：curl 测每个端点

---

## 5. WP3 — Desktop 激活 + 三层验证 + 深链（2d）

**目标**：桌面端实现激活、本地验签、心跳、深链回流。

**改动文件**：
- 新增 `desktop/src-tauri/src/commands/license.rs`
- 修改 `desktop/src-tauri/src/commands/mod.rs`
- 修改 `desktop/src-tauri/src/lib.rs`（注册 command + deep-link plugin）
- 修改 `desktop/src-tauri/Cargo.toml`（加 keygen-rs / machineid-rs / ed25519-dalek / tauri-plugin-deep-link）
- 修改 `desktop/src-tauri/tauri.conf.json`（加 `deepLinks`）

**Tauri Commands**：
- `get_device_id() → String`
- `start_trial() → TrialResult`
- `activate_license(license_key) → ActivationResult`
- `get_license_status() → LicenseStatus`
- `heartbeat_license() → HeartbeatResult`
- `revoke_this_device() → ()`
- `open_in_browser(url) → ()`

**验证**：
- 单元测试：`get_device_id` 稳定性（同机器多次调用结果一致）
- 集成测试：mock Keygen API，测激活→验签→心跳→吊销全流程
- 深链测试：浏览器点 `avrag-desktop://activate?key=...`，桌面端自动填入

---

## 6. WP4 — LLM 四轴架构重构（12.5d）⭐ 关键路径

**目标**：将 `avrag-llm` 从单层结构重构为 Protocol / Route / Provider 正交架构，支持 3 种原生协议 + 13 种 OpenAI 兼容 profile。

**详细设计**：`docs/adr/0005-llm-provider-protocol-architecture.md`

### 6.1 子任务分解

| 子任务 | 内容 | 工时 |
|--------|------|------|
| WP4-a | `schema/` 规范类型层（messages / events / errors / options） | 1.5d |
| WP4-b | `route/` 四轴路由层（auth / endpoint / framing / client / transport） | 2d |
| WP4-c | `protocols/openai_chat.rs`（从现有 request.rs + stream_parser.rs 迁移） | 1.5d |
| WP4-d | `protocols/anthropic_messages.rs`（原生 Anthropic，~300 行新增） | 2d |
| WP4-e | `protocols/gemini.rs`（原生 Gemini，~250 行新增） | 1.5d |
| WP4-f | `providers/` 配置层（3 原生 + 12 profile + profile 注册表） | 1d |
| WP4-g | `client/mod.rs` LlmClient 兼容 wrapper | 1d |
| WP4-h | 测试（单元测试 + 各协议集成测试） | 2d |

### 6.2 验证

- **回归测试**：`avrag-llm` 现有测试全部通过
- **SaaS 侧无改动**：`app-chat/` 的 15 个调用点编译通过，行为不变
- **新协议测试**：
  - OpenAI Chat：与现有行为一致（迁移后等价）
  - Anthropic Messages：用 Anthropic API key 跑真实请求（prompt caching / extended thinking）
  - Gemini：用 Google API key 跑真实请求
- **Profile 测试**：每个 profile 的 base_url + model 可达

### 6.3 并行策略

WP4 完全独立于 WP1-WP3 / WP5-WP7，可从 Day 1 开始并行。

---

## 7. WP5 — 前端独立页面（3d）

**目标**：新增 `(desktop)` 和 `(account)` route group，实现激活引导、LLM 配置引导、桌面端顶栏状态入口。

**详细设计**：`docs/desktop-frontend-pages-design.md`

**改动文件**：
- 新增 `app/(desktop)/layout.tsx` + `activate/page.tsx` + `setup/page.tsx`
- 新增 `app/(account)/licenses/page.tsx` + `[id]/page.tsx`
- 新增 `app/(marketing)/desktop/page.tsx` + `buy/page.tsx`
- 新增 `components/desktop/DesktopOnlyGate.tsx`
- 新增 `components/desktop/DesktopCenterLayout.tsx`
- 新增 `components/desktop/DesktopStatusBadge.tsx`
- 新增 `components/desktop/LLMDiagnosticPanel.tsx`
- 新增 `lib/desktop/llm-presets.ts`

**验证**：
- Vitest 组件测试
- Playwright E2E（桌面端模式）：激活引导流 → LLM 配置 → 测试连接 → 开始使用

---

## 8. WP6 — 收银（0.5d）

**目标**：在 Creem / 支付宝各加 2 个 product（Standard / Pro），购买后自动发 license。

**改动**：
- Creem console：创建 `desktop-standard` ($39) / `desktop-pro` ($99) product
- 支付宝配置：加 `ALIPAY_PRICE_DESKTOP_STANDARD=299` / `ALIPAY_PRICE_DESKTOP_PRO=699`
- `billing_domain.rs`：加 desktop checkout 分支
- Webhook handler：购买成功 → 调 Keygen CE 创建 license → 邮件发 key → 返回深链

**验证**：用 Creem sandbox 跑一次完整购买流程

---

## 9. WP7 — 打包 + 深链注册 + 公钥嵌入（1d）

**目标**：生成可发布的桌面安装包。

**改动**：
- `tauri.conf.json`：注册 `avrag-desktop` 深链 scheme
- `desktop/src-tauri/src/main.rs`：编译时嵌入 `KEYGEN_PUBLIC_KEY`
- `scripts/build-desktop.sh`：加 macOS / Windows / Linux 三平台构建
- 生成 installer（.dmg / .msi / .deb / .AppImage）

**验证**：
- 三平台安装包可正常安装
- 深链 `avrag-desktop://activate?key=...` 可拉起应用
- 离线验签：断网后 license file 仍可验证

---

## 10. 执行时间线

```
Week 1 (Day 1-5)
  ├─ WP1  Keygen CE 部署 ────────→ 1d  ✅
  ├─ WP2  SaaS License 接口 ─────→ 1.5d (依赖 WP1)
  ├─ WP3  Desktop 激活验证 ──────→ 2d  (依赖 WP1)
  └─ WP4-a/b/c  schema + route + OpenAI Chat 协议 → 5d (独立并行) ⭐

Week 2 (Day 6-10)
  ├─ WP4-d/e  Anthropic + Gemini 协议 → 3.5d ⭐
  ├─ WP4-f/g  Provider 配置层 + 兼容 wrapper → 2d ⭐
  └─ WP5  前端独立页面 ─────────→ 3d (依赖 WP2+WP3)

Week 3 (Day 11-15)
  ├─ WP4-h  测试 → 2d ⭐
  ├─ WP6  收银 → 0.5d (依赖 WP2)
  └─ WP7  打包 → 1d (依赖 WP3)

缓冲：2-3d
```

**关键路径**：WP4（12.5d）是整个计划的最大头。建议第一天就启动 WP4-a，与 WP1-WP3 完全并行。

---

## 11. 文档索引

| 文档 | 内容 |
|------|------|
| `docs/adr/0004-desktop-hybrid-business-model.md` | 混合商业模式决策 |
| `docs/adr/0005-llm-provider-protocol-architecture.md` | LLM 四轴架构决策 |
| `docs/desktop-license-activation-design.md` | 授权与激活详细设计 |
| `docs/desktop-llm-provider-design.md` | LLM 兼容性与诊断设计 |
| `docs/desktop-frontend-pages-design.md` | 前端独立页面设计 |
| `docs/desktop-execution-plan.md` | 本文档（总执行计划） |
| `docs/desktop-client-design-2026-06-11.md` | 现有桌面端架构设计 |

---

## 12. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| WP4 重构导致 SaaS 回归 | 高 | wrapper 保证旧 API 不变；分阶段迁移，每步跑全量测试 |
| Keygen CE 运维负担 | 中 | Docker 部署，半年一次升级；数据在自有 Postgres |
| Anthropic 原生协议实现复杂 | 中 | 先用 OpenAI 兼容端点发布，原生协议作为增量 |
| 深链在某些 OS 不生效 | 低 | 提供 fallback（手动复制 license key 输入） |
| 桌面端安装包体积大 | 低 | Tauri 产物本身较小（~15MB），avrag-llm 无重依赖 |