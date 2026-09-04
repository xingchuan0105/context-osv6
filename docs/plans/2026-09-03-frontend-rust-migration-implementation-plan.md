# 前端全栈 Rust 迁移实施编排计划（Next.js → Leptos/Axum）

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-03 |
| 状态 | 编排草案。**2026-09-04 19:40：Gate 0 改为观察，不再是绝对前置门禁**；目标态见 [ADR-0011](../adr/0011-rust-web-gpui-desktop.md) |
| 设计真相 | [`2026-09-03-frontend-rust-migration-design.md`](2026-09-03-frontend-rust-migration-design.md)（权威设计规范） |
| 产品 IA | [`docs/design/PRODUCT_IA.md`](../design/PRODUCT_IA.md)（Canonical 路由、Shell 规则与单一完成页） |
| 视觉基线 | [`docs/design/STYLE_BASELINE.md`](../design/STYLE_BASELINE.md) 与 `packages/cos-tokens/tokens.css` |
| 桌面真相 | `desktop/src-tauri` 与 `frontend_next/lib/runtime/tauri-ipc.ts` |
| 规则硬约束 | 编译/测试前须先做耗时估算并获得批准；严禁未另批就删除 `frontend_next` 或部署。Gate 0 数字只观察 |

---

## 0. 一句话目标

**以 Phase 0 真实会话同条件 Benchmark 为刚性决策依据（Go/No-Go），在确保核心性能指标改善 $\ge 20\%$、无关键功能与可访问性退化的前提下，逐层替换 69 个业务页面与桌面端，将 Web 生产运行时完全去 Node.js 化，实现前后端统一基于 Rust 契约与单向数据流收敛。**

---

## 1. 总体实施切片与门禁总览

```text
[Phase 0: PoC 收益与可行性] ── Gate 0 ──► [Phase 1: 基础平台与协议层] ── Gate 1 ──► [Phase 2: 完整 Chat-first 垂直切片] ── Gate 2
                               (Go/No-Go)
                                                                                                  │
┌─────────────────────────────────────────────────────────────────────────────────────────────────┘
▼
[Phase 3: 应用与交易表面] ──── Gate 3 ──► [Phase 4: 公共 SSR 与 SEO] ── Gate 4 ──► [Phase 5: Tauri 桌面端接入] ── Gate 5
                                                                                                  │
┌─────────────────────────────────────────────────────────────────────────────────────────────────┘
▼
[Phase 6: 灰度切流、观察与清理] ── Gate 6 ──► 【终态完成】
```

---

## 2. 现场勘察与实施前提（2026-09-03 真实证据）

| 事实项 | 现状与勘察依据 | 对本计划的影响与执行策略 |
|---|---|---|
| **Rust 工具链** | `cargo 1.94.0`，`wasm32-unknown-unknown` target 已安装 | 本地环境具备 WASM 交叉编译能力，无需额外安装底层 target |
| **`contracts` 依赖** | `contracts/Cargo.toml` 包含 `serde`, `chrono`, `uuid`, `utoipa`, `thiserror`, `typeshare`, `ts-rs` | 属于纯数据结构 crate，无系统级 libc/网络阻塞，但在 wasm32 下需验证 `uuid/js` 与 `chrono` 的时间依赖 |
| **后端 API 拓扑** | `deploy/nginx/app-contextlm.conf` 已将 `/api/` 反代直达 `avrag-api:8081` 并禁用 buffering | `frontend_rust` 的 `web-server` 只承担首屏 HTML、SSR 和静态资源，**严禁在 Rust 服务内再做二次 API 反代** |
| **桌面端路径** | 真实路径为 `desktop/src-tauri`（配置 `tauri.conf.json` 指向静态前端产物） | 桌面产物为独立 CSR WASM 包，不依赖 Axum 后台进程 |
| **视觉 Token 唯一源** | `packages/cos-tokens/tokens.css` 为全仓设计基线唯一真源 | `frontend_rust` 严禁分叉维护第二份 Token，通过构建脚本直接同步使用 |

---

## 3. Phase 0 实施细则：可行性与收益验证（Go/No-Go 闭环）

### 3.1 步骤清单

#### Step 0.1：`contracts` WASM 兼容性嗅探
- **动作**：创建最小化 `crates/web-sdk` 骨架，添加对 `../../contracts` 的 path 依赖。
- **验证**：执行 `cargo check -p web-sdk --target wasm32-unknown-unknown`。
- **异常处理**：若 `uuid` 或 `chrono` 阻碍 wasm32 编译，按照设计文档 §4 将 shared wire types 拆出或添加 feature gate，严禁在客户端复制代码。

#### Step 0.2：录制标准测试夹具（Golden Fixtures）
- **动作**：从当前 `frontend_next` 中录制 3 组真实生产会话样本，存放在 `frontend_rust/tests/fixtures/`：
  1. `stream-normal-long.json`：包含 3000+ 字 Markdown、嵌套代码块、表格及 5 个引用节点的真实 `ChatEvent` 序列。
  2. `stream-tool-observation.json`：包含 `activity`、`operation_guide`、`trace`、多阶段检索工具调用的事件流。
  3. `stream-cancel-abort.json`：高频打字中途发出 `cancel` 动作及网络中断场景。

#### Step 0.3：构建纯 Rust 垂直 PoC
- **动作**：
  1. 在 `frontend_rust` 建立独立的 `Cargo.toml` 与 `Cargo.lock`。
  2. 搭建 3 个最小子 crate：`web-sdk`（含 `BrowserHttpTransport` 与 `TauriIpcTransport` 抽象）、`web-ui`（Leptos 组件与纯 Reducer）、`web-server`（Axum 宿主）。
  3. 仅实现 `/chat` 单一路由：包含会话列表恢复、Composer 输入框、使用真实 `ChatEvent` 驱动的流式消息气泡渲染、停止/重试按钮。
  4. 样式直接加载 `packages/cos-tokens/tokens.css`。

#### Step 0.4：同条件 Benchmark 测量与数据采集
- **环境隔离**：在相同的 Chrome 实例、无插件、固定视口宽度（1440x900）、固定网络带宽限制（Fast 3G / Localhost）下。
- **测量对照组**：`frontend_next` 优化基线 vs `frontend_rust` PoC。
- **必测指标与取样**：
  - 每项指标执行 5 次冷启动 + 20 次热路径，记录中位数与 p95：
    1. 首屏传输大小（Compressed bundle size）与 LCP。
    2. 流式高频输入到屏幕绘制延迟（Input-to-render latency，固定 50 token/s 吞吐）。
    3. 30 分钟连续会话内存占用与 GC 泄漏斜率（Heap snapshot 采样）。
    4. 长会话快速滚动帧率（FPS 及掉帧率）。
    5. 协议正确性：零丢事件、零重复终态、取消响应时间。

#### Step 0.5：Gate 0 评审报告与决策
- **产出**：编写 `docs/plans/2026-09-03-phase-0-benchmark-report.md`，汇总数据表格与结论。
- **判定线（Hard Pass Line）**：
  - [ ] 预先指定的至少两个核心 p95 指标相对 Next.js 提升 $\ge 20\%$。
  - [ ] 首屏、交互、内存、体积无任何一项退化超过 $10\%$。
  - [ ] 30 分钟长会话无持续无界内存膨胀。
  - [ ] 真实 `ChatEvent` 夹具单测 100% 对齐，零协议漂移。
  - [ ] Native SSR、Browser Hydrate、Tauri CSR 三目标均能成功构建运行。
- **决策分支**：
  - **全部满足**：正式批准进入 Phase 1。
  - **任一失败**：立刻停止迁移，转向对 `frontend_next` 的定向优化，归档 PoC，绝不强行推进。

---

## 4. 后续全量迁移编排路线（Phase 1 ~ Phase 6 承接）

### Phase 1：平台基础设施与核心协议层（Gate 1）
- **范围**：
  1. 固化 `frontend_rust` 的 `Leptos.toml`、feature 体系与编译矩阵。
  2. 完善 `web-sdk` 的 Typed Error 系统（可区分网络中断、4xx/5xx、JSON 损坏、心跳超时）。
  3. 编写 `style_baseline_guard.rs` 静态测试，守卫无裸露 Hex 与无 `font-weight >= 500`。
  4. 接入 `packages/cos-tokens/sync.sh` 自动化样式同步。
- **Gate 1 验收**：三目标（SSR、Hydrate、CSR）构建全绿，底层 Reducer 单元测试 100% 通过，样式守卫测试无违规。

### Phase 2：完整 Chat-first 垂直切片（Gate 2）
- **范围**：
  1. 落地完整 ChatCanvas：多轮历史回放、文件托盘（Session files）、引用节点折叠/悬浮卡片、模型角色切换（现行领域值 `quick_chat`、`agent`）。
  2. 实现独立的 quick-chat BYOK 配置通道，严禁暗建 Workspace。
  3. 在 `/dashboard/:workspace_id` 内复用同一套 ChatCanvas 与执行管线，仅注入 Workspace 上下文。
  4. 部署内部 Canary 环境，由团队进行日常狗粮测试（Dogfooding）。
- **Gate 2 验收**：个人对话、带文件对话、工作区对话端到端可用；Stop、Retry、Feedback、Citation 交互零缺陷。

### Phase 3：应用与交易表面（Gate 3）
- **范围**：
  1. 按照 Route Family 逐组迁移并挂载：
     - `/dashboard`（工作区卡片、列表、删除确认）。
     - `/dashboard/:id/share`（唯一分享中心：链接、权限、API 密钥、访问日志）。
     - `/dashboard/analytics`（跨工作区汇总分析）。
     - `/settings`（Profile、Providers BYOK、Billing、Security）。
     - `/pricing` 与 `/upgrade/*`（Canonical 钱与商业化，会员档位与 top-up 锚点）。
     - `/login`、`/register`、`/reset-password/*`。
     - `/admin/*`（13 个管理后台页面）。
  2. 富文本编辑策略落地：基于成熟 WASM 编辑器或轻量 Markdown 增强输入，完成笔记编辑能力迁移与回归。
- **Gate 3 验收**：所有已迁移页面的权限守卫、表单交互、深链参数（`?tab=`, `?session=`）完全对齐。

### Phase 4：公共 SSR、SEO 与多语言（Gate 4）
- **范围**：
  1. 在 `web-server` 中实现 Axum SSR 直出：`/pricing`、`/desktop`、`/help/*`、`/legal/*`。
  2. 落地中英文双语对照（`/en/*`），精确对齐 hreflang 与 canonical URL。
  3. 挂载 `robots.txt`、`sitemap.xml`、`llms.txt`、Web App Manifest 及 JSON-LD 结构化数据（Organization, SoftwareApplication, FAQPage）。
- **Gate 4 验收**：通过 `curl` 抓取 HTML 与搜索引擎爬虫模拟器测试，文本内容与元数据 100% 完整，无 JavaScript 亦可阅读。

### Phase 5：Tauri 桌面端接入（Gate 5）
- **范围**：
  1. 生成纯静态 CSR 产物至 `frontend_rust/dist/tauri`。
  2. 修改 `desktop/src-tauri/tauri.conf.json`，将构建命令与资源目录指向 Rust 前端产物。
  3. 打通 `TauriIpcTransport`，验证原生命令调用（流式收发、取消、本地文件系统读写、更新器与深度链接）。
- **Gate 5 验收**：Windows 桌面客户端打包成功；本地离线/在线状态切换、云端登录下发凭据全流程跑通。

### Phase 6：灰度切流、观察与清理（Gate 6）
- **范围**：
  1. 修改部署脚本 `scripts/deploy-*.sh`，Nginx 将 Web 流量逐步切至 Rust `web-server`。
  2. 进入至少 7 天生产观察窗口，监控 SSE 断流率、用户首屏性能、鉴权回跳与服务器资源。
  3. 保持一键回滚能力（回滚至上一版本 `frontend_next` 部署产物）。
  4. 观察期无异常后，正式执行清理：
     - 删除 `frontend_next/` 目录。
     - 删除 `scripts/generate-contracts.sh` 与 `typeshare.toml`。
     - CI 移除 Node.js/pnpm 构建步。
     - 更新 `docs/README.md` 与 code-review-graph。
- **Gate 6 验收**：单仓库仅由 Cargo 统一构建，生产无 Node.js 运行时，全站业务指标平稳。

---

## 5. 纪律约束与禁止清单

1. **禁止越过 Phase 0 提前写全量业务组件**：未获得 Benchmark 实测收益证据前，代码仅限于 Phase 0 PoC 范围。
2. **禁止在 Rust Web 服务中二次反代 `/api/`**：生产环境必须遵守现有架构，Nginx 直接打向 `avrag-api`。
3. **禁止发明第二完成页**：严格遵守 `docs/design/PRODUCT_IA.md`，废弃路径（如 `/analyze`）仅在路由层 301 重定向，绝不在 Rust 前端实现第二套分析页。
4. **禁止混淆 Conversation 与 Workspace 归属**：不得为了实现方便在用户提问时隐式创建 Workspace。
5. **禁止在未获时间批准的情况下跑全量构建**：执行任何耗时较长的编译或测试前，先向用户报告预计用时并征得同意。
