# Rust 前端迁移可行性与分层切换设计（Next.js → Leptos/Axum）

> 日期：2026-09-03
>
> 状态：评审修订稿；建议只批准 Phase 0 可行性验证，尚未批准全量迁移或删除 `frontend_next`
>
> 候选栈：Leptos + Axum + cargo-leptos；版本由 Phase 0 的 SSR/WASM/Tauri 验证结果锁定
>
> **2026-09-04 目标态修订：** 桌面不再走 Tauri WebView / Leptos CSR。在线仍是 Leptos；Windows / macOS / Linux 桌面均为 GPUI。权威：[ADR-0011](../adr/0011-rust-web-gpui-desktop.md)。下文 §4 三目标矩阵、§10、Phase 5、完成定义中的「同一 `web-ui` + Tauri IPC」**不再执行**。
>
> **2026-09-04 19:40：** Gate 0 全部改为观察项，不再因未达 ≥20% 停开发。§3.3 的「停迁移」对开发计划失效；删除 Next / 部署仍须另批。

## 1. 决策摘要

本方案不是“按语言重写页面”，而是验证并逐层替换当前 Next.js 前端。迁移只有在同一产品语义、同一后端协议、同一测试夹具下，证明 Rust 方案在目标指标上有实质收益且没有不可接受的功能退化时才继续。

迁移目标：

1. Web 生产运行时不再依赖 Node.js；SSR、公网页面和静态资源由 Rust 服务提供。
2. 浏览器与桌面端复用 Rust 类型、聊天协议解析和状态约束。
3. 保持 Chat-first 产品模型：Conversation 可独立存在，Workspace 只提供可复用、可管理、可分享的知识上下文。
4. 保持现有路由、SEO、鉴权、流式聊天、文件、引用、BYOK、计费、管理后台和桌面能力的产品语义。
5. 切换完成后删除旧路径，不保留长期兼容层。

本次不做：

- 不把 `avrag-rs`、`contracts`、`frontend_rust`、`desktop/src-tauri` 强并为一个 Cargo workspace 或一份 `Cargo.lock`。它们有不同目标平台和发布边界。
- 不把“生产无 Node.js”扩大成“仓库完全无 Node.js”。Playwright 等开发/验收工具可继续使用 Node.js；是否替换由独立收益评估决定。
- 不在前端迁移中同时重做认证、计费、信息架构或后端 API。
- 不把现有能力裁掉来制造迁移完成度；任何明确下线都必须先作为产品决策写入 `docs/design/PRODUCT_IA.md`。
- 不复活已删除的旧 `frontend_rust` 实现。历史代码仅可用于考古，不作为当前产品模型或可直接复用的基础。

## 2. 权威输入与现状基线

迁移以以下文档和代码为准：

- 产品信息架构：`docs/design/PRODUCT_IA.md`
- Chat-first 领域与交互：`docs/plans/2026-09-02-chat-first-conversation-workspace-design.md`
- Rust API 契约：`contracts/src/**/*.rs`
- 当前前端可观察行为：`frontend_next/app`、`frontend_next/components`、`frontend_next/lib`
- 桌面边界：`desktop/src-tauri` 与 `frontend_next/lib/runtime/tauri-ipc.ts`
- 视觉基线：`docs/design/STYLE_BASELINE.md` 与 `packages/cos-tokens/tokens.css`
- 部署入口：`scripts/deploy-*.sh` 与 `deploy/nginx/app-contextlm.conf`

当前事实：

- `frontend_next/app` 当前有 69 个 `page.tsx`，另有 route、layout、metadata、图标和站点地图等非页面入口。迁移清单不能只覆盖聊天和 Workspace 主路径。
- 当前聊天 Web 入口是 `POST /api/v1/chat`，SSE 事件契约来自 `contracts/src/chat.rs::ChatEvent`。
- 生产 Nginx 已让 `/api/` 直达 `avrag-api:8081`，并关闭 SSE buffering；Rust Web 服务不承担生产 API 反向代理。
- Tauri 当前读取 Next 静态产物，并通过原生命令承载聊天流、取消、认证会话、上传、本地栈、发布、升级和深链等能力。桌面端不是简单的 Web URL 包装。
- `packages/cos-tokens/tokens.css` 是视觉 Token 唯一源，`packages/cos-tokens/sync.sh` 负责向各站点分发副本。

## 3. Phase 0：先证明迁移值得做

### 3.1 同条件对照

先实现一个隔离的 Rust 垂直 PoC，只包含：

- `/chat` 的 SSR 壳与浏览器 hydration；
- 会话列表最小加载；
- 消息历史；
- Composer；
- 使用真实 `ChatEvent` 夹具的流式回复、停止和错误恢复；
- Browser HTTP/SSE 与 Tauri IPC 两个 Transport adapter 的最小验证；
- 与现有 Token 一致的基础样式。

PoC 与优化后的 Next.js 基线使用相同设备、浏览器版本、网络条件、长会话数据和录制的 SSE 分块序列。不得拿空白 Rust 页面与完整 Next 产品比较。

### 3.2 必测指标

开始测量前记录机器、浏览器、构建模式、样本大小和判定阈值。至少比较：

| 维度 | 测量对象 |
|---|---|
| 首次加载 | 压缩传输体积、首屏内容出现、可交互、hydration 错误 |
| 流式交互 | 固定 token 速率下的输入到绘制延迟、批处理延迟、掉帧、长任务 |
| 长会话 | 固定消息数下的滚动稳定性、内存基线与增长斜率、定位到消息的耗时 |
| 操作正确性 | stop/cancel、retry、错误重连、重复终态、断流、切换会话 |
| 可访问性 | 键盘路径、焦点恢复、ARIA live、减少动画偏好 |
| 工程反馈 | 冷构建、增量构建、浏览器刷新、WASM 产物大小 |
| 跨目标 | Native SSR、Browser WASM、Tauri CSR 三种目标是否均能构建并运行 |

是否需要列表虚拟化由长会话测量决定；不能先假设虚拟化一定更快。若启用，必须覆盖动态高度、代码块、图片、引用展开、键盘浏览和滚动锚定。

### 3.3 Go / No-Go

Phase 0 启动时先冻结 benchmark charter；看到结果后不得改口径。默认判定线如下，若业务负责人另定阈值，应在首次跑数前写回本文：

| 判定项 | 默认通过线 |
|---|---|
| 协议正确性 | 全部合法/异常 SSE 与 IPC fixture 结果一致；零丢事件、零重复终态、零静默解析失败 |
| 平台可行性 | Native SSR、Browser hydrate、Tauri CSR 三个 release-like 产物均能构建和启动 |
| 产品正确性 | PoC 清单中的关键旅程全部通过，stop/retry/error/focus 无已知 P0/P1 缺陷 |
| 性能收益 | 预先指定的至少两个核心 p95 指标相对优化后 Next 基线改善不低于 20% |
| 性能护栏 | 首屏、交互、内存、压缩传输体积中任何核心指标不得回退超过 10% |
| 稳定性与无障碍 | 30 分钟压力样本无持续无界内存增长；自动/人工检查无新增 critical a11y 问题 |

每组性能结果至少包含 5 次冷启动和 20 次热路径样本，报告中给出中位数、p95、离散度和原始记录。低于门槛的结果不能用单次最佳值替代。

只有同时满足以下条件才进入 Phase 1：

1. Rust PoC 在预先选定的核心性能指标上有足以覆盖迁移成本的实质收益，而不是测量噪声。
2. Web SSR、Browser hydrate 和 Tauri CSR 都能使用同一核心 UI/状态模型，无目标平台阻塞。
3. `contracts` 在 native 与 `wasm32-unknown-unknown` 目标下可用；不可用部分已有更小、清晰的契约拆分方案。
4. 真实 SSE/IPC 夹具下，事件顺序、取消、终态、错误和引用行为与现网一致。
5. 没有要求用临时双协议、长期兼容层或产品功能降级换取通过。

任一条件失败即停止全量迁移，记录证据，转为针对现有 Next.js 的性能优化。Phase 0 不是既定结论的形式化步骤。

## 4. Workspace、目标平台与构建边界

`frontend_rust` 使用独立 workspace 和独立 lockfile：

```text
frontend_rust/
├── Cargo.toml
├── Cargo.lock
├── Leptos.toml
├── crates/
│   ├── web-sdk/       # 平台中立的契约客户端、Transport seam、SSE 解码
│   ├── web-ui/        # Leptos 组件、路由视图、纯状态转换
│   └── web-server/    # Axum SSR、公网页面、静态资源、健康检查
├── assets/
└── tests/
    └── fixtures/      # HTTP/SSE/IPC golden fixtures
```

`web-sdk` 通过 path dependency 复用 `../contracts` 中可跨目标编译的契约。若 `contracts` 的 server-only 依赖阻塞 WASM，应把共享 wire types 下沉为更小的契约 crate；不得在 `web-sdk` 复制第二套 `ChatEvent`。

构建矩阵：

| 产物 | 编译目标 | UI feature | 宿主 | 用途 |
|---|---|---|---|---|
| Web server | native | `ssr` | Axum | SSR、公网页面、静态资源 |
| Browser app | `wasm32-unknown-unknown` | `hydrate` | 浏览器 | Web 应用交互、HTTP/SSE |
| Desktop app | `wasm32-unknown-unknown` | `csr` | Tauri WebView | 桌面 UI、IPC |

规则：

- `web-ui` 不在无条件代码路径中访问 `window`、`document`、`localStorage` 或 Tauri API。
- 平台 API 只能存在于对应 adapter，并受 target/feature gate 约束。
- SSR 与 CSR 不能共享隐式全局状态；服务端请求状态按请求创建。
- `cargo-leptos` 的 server 和 frontend feature 必须显式配置并在验收中分别构建。
- 生产目标是 Rust Web 服务不依赖 Node runtime；Playwright、格式化或其他开发工具仍按实际价值保留。

文档通过后再把精确命令固化进实现计划。所有编译、测试和脚本运行仍需按仓库规则先给出耗时估计并获得同意。

## 5. 深模块与真实替换缝

### 5.1 `web-sdk`：平台中立的会话客户端

`web-sdk` 提供窄接口，隐藏认证头、HTTP 状态、SSE framing、IPC framing 和 wire event 解码：

```rust
pub trait ChatTransport {
    type Stream;

    async fn stream_chat(
        &self,
        request: ChatRequest,
        cancellation: Cancellation,
    ) -> Result<Self::Stream, TransportError>;
}

pub struct ChatClient<T: ChatTransport> {
    transport: T,
}
```

接口的具体形状由 PoC 验证，可使用稳定的 stream trait/库；这里表达的是模块责任，不预先锁死某个异步类型。

`web-sdk` 必须保持：

- 无 DOM、Storage、Leptos 组件或路由依赖；
- 不让 UI 手工拼 URL、Bearer header 或解析 SSE；
- HTTP 非 2xx、协议错误、未知事件、断流和取消有可区分的 typed error；
- 不用“解析失败即忽略”掩盖协议漂移。

Transport seam 至少有两个真实 adapter：

- `BrowserHttpTransport`：同源 `POST /api/v1/chat` + Fetch/SSE；
- `TauriIpcTransport`：调用既有 Tauri chat stream/cancel command；
- `FixtureTransport`：只用于确定性的 reducer、顺序、错误和取消测试。

凭据存储是另一条 seam，不与 Transport 混在一起：

- 浏览器 adapter 保持当前 token、auth hint 与 persisted session cookie 的可观察行为；
- Tauri adapter 使用原生 `cloud_session` 能力；
- auth hint 只用于导航提示，不能成为服务端授权依据；
- 认证模型的任何改变另立设计，不搭迁移便车。

### 5.2 `web-ui`：共享 UI 与纯状态转换

`web-ui` 拥有：

- ChatCanvas、Workspace shell、公共视图和共享组件；
- `ChatEvent + 当前状态 -> 新状态 + UI effect` 的可测试 reducer；
- 路由参数到页面模型的转换；
- SSR/hydrate/CSR 的目标适配点。

Reducer 是主要测试接口。组件不应同时承担协议解析、网络重试、领域状态和 DOM 渲染。

### 5.3 `web-server`：SSR 与静态资源

`web-server` 拥有：

- SSR、公网页面、静态资源和健康检查；
- cache/header/metadata 输出；
- 路由级 noindex、canonical 与 hreflang。

生产 `/api/`、`/v1/relay/` 和 signed upload 继续由 Nginx 直达 `avrag-api`。`web-server` 不再代理一次，以免引入双跳、buffering 和超时差异。开发环境若需要代理，应作为仅开发配置并单独测试。

## 6. 聊天 wire contract 与状态机

### 6.1 真实协议

Web 调用：

```http
POST /api/v1/chat
Accept: text/event-stream
Content-Type: application/json
Authorization: Bearer <token>

{ ...ChatRequest, "stream": true }
```

事件以 `contracts/src/chat.rs::ChatEvent` 为唯一真源：

| 事件 | UI 语义 |
|---|---|
| `start` | 建立 request/session 关联，进入运行态 |
| `operation_guide` | 内部/外部代理操作指南；不能误作用户答案 |
| `activity` | 进度、阶段、计数与来源预览 |
| `answer_start` | 建立 assistant message 与 agent 类型 |
| `trace` | 诊断/阶段轨迹；默认不进入用户主气泡 |
| `token` | 追加用户可见答案文本 |
| `reasoning_summary_delta` | 追加可展示的推理摘要区域，不混入答案正文 |
| `citations` | 合并引用元数据并解析正文 marker |
| `done` | 用服务端最终 payload 收束且只收束一次 |
| `error` | 进入可恢复失败态，保留已收到内容和重试上下文 |

`HostMarker`、`Observation`、`ContentChunk` 不是现行客户端事件名，不得写入实现或测试夹具。

### 6.2 SSE 与 IPC 的最低正确性

SSE 解码必须覆盖：

- UTF-8 字符跨 chunk；
- `\n` 与 `\r\n`；
- 一个事件的多行 `data:`；
- comment/keepalive；
- chunk 末尾无空行；
- HTTP 非 2xx、空 body、未知 event、坏 JSON 和中途断流；
- `done`/`error` 终态唯一性，终态后事件不可继续改写状态；
- Abort/cancel 竞态、快速切换会话和旧 request 迟到事件；
- 浏览器 SSE 与 Tauri IPC 对同一 `ChatEvent` 序列产生相同 reducer 结果。

夹具从真实协议样本脱敏固化。单测通过不代表网络链路通过，后续还需浏览器和 Tauri 端到端验收。

### 6.3 用户信道

- 用户主气泡只展示模型自然语言答案及其产品定义的引用/推理摘要，不拼接 harness observation、协议外壳或运行时诊断脚注。
- `activity`、`trace` 和工具状态进入独立进度/详情区域。
- 流式阶段可以用轻量纯文本保证性能；结束后切换到经 sanitizer 处理的 Markdown/引用渲染。
- 协议错误必须成为可观测错误与 telemetry，不能静默吞掉后继续显示貌似完整的答案。

## 7. Chat-first 产品不变量

迁移后的个人会话、Workspace 会话和分享只使用一个 ChatCanvas 与一条会话执行管线：

1. `/chat` 默认创建/恢复个人 Conversation，`workspace_id` 可空；不得暗建 Workspace。
2. `/dashboard/:workspace_id` 进入 Workspace shell，但聊天仍是 Conversation；Workspace 只是附加上下文。
3. 用户显式切换到 Workspace 或显式带入来源；上传文件不能隐式改变归属。
4. Session files 属于 Conversation 生命周期；Workspace files 属于 Workspace 生命周期，两者 UI、API 和清理语义分开。
5. 全局最近会话包含个人和 Workspace 会话，并清楚显示归属。
6. `model_role` 使用现行领域值 `quick_chat`、`agent`；每个 role 的模型选择独立。
7. quick-chat BYOK 独立配置，不得被 Workspace 绑定或由其他 provider 设置暗中继承。
8. Web 能力在个人 Conversation 中独立可用；它不是 Workspace 的附属开关。
9. 来源 scope 只做增量叠加：Session files、当前 Workspace、显式选择的其他 Workspace/Web 均保持可解释。
10. stop、retry、feedback、citation、progress、文件托盘、scope bar 和模型选择是 ChatCanvas 的共同能力，不能只迁移文本收发。
11. 分享页锁定其授权上下文；访问者不能借分享入口扩展到其他 Workspace 或个人资源。
12. Snapshot/Evidence 等后续层只能叠加在已经工作的 Conversation 上，不能为了未来结构牺牲当前端到端可用性。

## 8. 路由与表面迁移清单

每个 route family 建立一行可跟踪清单，记录现有 route、目标 render mode、auth/noindex、数据依赖、验收用例和切换状态。默认规则是保留；下线必须有单独产品决策。

| Route family | 必须保持的语义 |
|---|---|
| `/`、`/en` | SSR SEO 内容；浏览器根据真实会话状态进入 `/chat`；canonical/hreflang |
| `/login`、`/register`、`/reset-password/*` | 登录、注册、重置三阶段与回跳；桌面环境的认证边界不变 |
| `/chat`、`/chat/:sessionId` | Chat-first、恢复/新建、完整 ChatCanvas |
| `/dashboard`、`/dashboard/analytics`、`/dashboard/:workspace_id/*` | Workspace 列表、详情、全局分析、Workspace 分析、分享、访问日志 |
| `/settings`、`/settings/usage` | provider/BYOK、偏好、用量；canonical tab/deep link 不变 |
| `/pricing`、`/upgrade/paywall`、`/upgrade/success` | 会员、top-up anchor、paywall 解释、支付回跳 |
| `/desktop`、`/desktop/buy` | 公共下载与购买页 |
| `/setup`、`/activate` | 桌面专用初始化和激活，不暴露成普通 Web 主路径 |
| `/help`、`/help/write` | 应用 shell、弱入口、noindex |
| `/help/api-access*`、`/help/faq`、`/help/compare` | 公共 SSR、SEO、agent-readable 内容 |
| `/integrations/*` | 公共集成文档与 canonical |
| `/legal/*`、`/en/legal/*` | 法律文本结构、链接、语言与版本信息逐项一致 |
| `/shared/kb/:token`、`/shared/u/:userId` | 公共分享权限边界和 noindex/SEO 策略 |
| `/invite/:workspace_id/:member_id` | 邀请接受、登录衔接、失效和权限错误 |
| `/admin/*` | 13 个现有管理页、管理员鉴权、审计与降级信息 |
| metadata/static | `robots.txt`、sitemap、manifest、`llms.txt`、图标、OG/Twitter 图、百度验证 |

并行期不修改当前 `frontend_next/lib/navigation/nav-config.ts` 的权威地位；Rust 导航从迁移 route manifest 生成或接受自动 parity 检查。全量切换时再把唯一 nav config 移入 Rust，同时更新 `PRODUCT_IA.md` 和守卫测试，不长期手工维护两份导航定义。迁移不得发明第二个完成路径。

## 9. 内容、安全、编辑器与视觉

### 9.1 Markdown、引用与不可信内容

模型答案、外部网页和文档片段均按不可信内容处理：

- Markdown 到 HTML 后必须使用明确 allowlist 的 sanitizer；
- 禁止脚本、事件属性、危险 URL scheme 和未授权 iframe；
- 外链、代码复制、表格、引用 marker、来源卡片和流式/完成态切换保持现有语义；
- 用恶意 HTML、畸形 Markdown、引用增量更新和大代码块做 fixture 测试；
- 不把 `inner_html` 便利性当作安全边界。

### 9.2 笔记编辑器

现有 Tiptap 编辑器包含 Markdown round-trip、历史、选择、链接、粘贴清洗等行为。一个手写 `contenteditable` 不是等价替代。

Phase 0/1 必须先评估成熟、维护活跃、支持 WASM 的编辑器或可靠 DOM adapter。若没有满足需求的库，只有两种可接受结论：

1. 给出经过验证的窄编辑模型及完整行为测试；或
2. 暂不切换依赖编辑器的 route，并把该 route 作为迁移阻塞项。

不得以永久嵌入 Next 页面或长期双编辑器作为最终架构。

### 9.3 样式与资产

- `packages/cos-tokens/tokens.css` 保持唯一真源；为 `frontend_rust` 扩展现有同步脚本或构建复制目标，不复制出人工维护的第二份 Token。
- 保持字体权重、颜色、阴影、动效和 responsive 基线；测试应保留 Token 文件、品牌 metadata 和浮层等现有必要 allowlist，而不是机械禁止所有 hex/shadow。
- `packages/cos-tokens/mark.svg` 继续作为品牌 mark 来源。
- 中英文页面、canonical、hreflang、结构化数据和 noindex 必须在路由清单逐项验收。

## 10. Tauri 迁移边界

Tauri 使用专用 CSR 产物，例如 `frontend_rust/dist/tauri`。切换时修改实际文件 `desktop/src-tauri/tauri.conf.json` 的 `frontendDist` 与 build command；不能引用仓库根不存在的 `src-tauri`。

桌面验收至少覆盖：

- `chat_stream_start` / cancel 的事件、终态与快速重发；
- cloud session 登录/登出/恢复，以及桌面不跳普通 Web `/login`；
- REST、signed upload、文件选择与本地文件边界；
- local stack 状态和设置；
- publish、更新检查/下载/安装；
- deep link、single instance、窗口生命周期和 CSP；
- 离线、后端不可达、认证过期和升级失败。

浏览器与 Tauri 共用 reducer 和契约，但通过两个真实 adapter 访问宿主。Tauri WebView 不提供生产 SSR；桌面产物不得依赖 Axum 服务常驻。

## 11. 分层实施与验证门

任何阶段的 Gate 失败都停在当前层，不得用后续阶段掩盖失败。

### Phase 0：可行性与收益验证

- 建立独立 workspace、三目标最小构建；
- 验证 `contracts` native/WASM 兼容；
- 完成真实聊天协议、两个 Transport adapter 和 `/chat` PoC；
- 产出同条件性能与正确性报告；
- 做 Go/No-Go 决策。

**Gate 0：** 第 3 节五项条件全部满足。

### Phase 1：基础平台

- 固化 build matrix、feature 与 lockfile；
- 建立 `web-sdk` 深接口、typed errors、golden fixtures；
- 建立纯 reducer、browser credential adapter 和 SSR request state；
- 建立 route manifest、SEO/security/style 基线测试；
- 把共享 Token 同步到 Rust 产物。

**Gate 1：** 三目标构建、协议测试和最小 SSR/hydrate/CSR smoke 全绿。

### Phase 2：完整 Chat-first 垂直切片

- 完成个人 Conversation、历史、Session files、Web、模型角色和 quick-chat BYOK；
- 完成 stop/retry/feedback/citations/progress/scope；
- 完成 Workspace shell 内复用同一 ChatCanvas；
- 内部或受控 canary 暴露，不替换全部生产流量。

**Gate 2：** Chat-first 验收矩阵与性能回归均通过，不存在文本聊天之外的关键能力缺口。

### Phase 3：应用与交易表面

- Workspace 管理、分享/邀请、设置、用量；
- pricing、paywall、支付成功回跳；
- auth/reset；
- admin 全路由。

按 route family 独立验证、独立切流；尚未迁移的 route 仍由 Next 服务，不在 Rust 内实现永久 fallback。

**Gate 3：** 相应 route family 的 auth、错误态、响应式、可访问性和业务 E2E 全绿。

### Phase 4：公共 SSR、SEO 与双语

- 中文/英文首页、desktop、help、integrations、legal；
- metadata、robots、sitemap、`llms.txt`、验证文件和图片；
- 缓存、canonical、hreflang、noindex 与 structured data。

**Gate 4：** 页面清单无缺项，HTML/metadata crawl diff 与关键视觉 diff 在已批准阈值内。

### Phase 5：Tauri

- 接入 `TauriIpcTransport` 与原生 credential store；
- 生成专用 CSR 产物并修改真实 Tauri 配置；
- 完成第 10 节桌面验收。

**Gate 5：** Windows 安装包与本地开发两条路径通过；Web 与 IPC reducer fixture 一致。

### Phase 6：切流、观察与清理

- 通过部署脚本按 route family/canary 切换，不手工上传产品代码；
- 观察前端错误、SSE 完成率、取消延迟、登录回跳、支付回跳、分享访问和服务资源；
- 保留可一键回到上一已验证部署产物的发布级 rollback；
- 所有 route 和桌面 Gate 通过、观察窗口结束后，删除 `frontend_next` 与只为其存在的生成/构建路径；
- 更新部署脚本、服务文件、文档索引和 code-review graph。

**Gate 6：** 生产路由清单全部由 Rust 承担，Node 不再是 Web 生产运行时，rollback 已演练，旧前端才允许删除。

## 12. 切换、回滚与删除原则

迁移期同时运行两个前端是发布隔离手段，不是产品兼容承诺：

- 同一路由在任一时刻只有一个生产 owner；
- Nginx/部署清单明确 route owner 和产物版本；
- session/API 契约不因前端 owner 改变；
- 静态资产带内容 hash，避免新 HTML 引用旧 WASM 或反之；
- rollback 回到完整的上一版本产物，不在 Rust 中维护双实现分支；
- 最终删除旧前端、旧 service 和只服务旧前端的脚本/依赖，不保留 shim。

删除前逐项确认：

- route manifest 全绿；
- Web、桌面、公共 SEO 与 admin 验收全绿；
- 生产指标与错误预算稳定；
- 法律和双语内容完成结构 diff；
- Playwright 是否保留已有明确决定；
- `scripts/generate-contracts.sh` 等路径只有在不再有消费者时删除；
- 不删除并不存在的文件，也不顺手清理与迁移无关的代码。

## 13. 主要风险与控制

| 风险 | 后果 | 控制 |
|---|---|---|
| Rust 性能收益未经证明 | 花费巨大迁移成本但用户无感 | Phase 0 同条件 Go/No-Go |
| `contracts` 无法跨 native/WASM | 重复契约或复杂 feature 污染 | 先做 target 编译门；必要时抽小型 wire crate |
| SSR 与浏览器 API 混层 | native 编译失败、hydration 崩溃 | 平台 adapter + 三目标 build matrix |
| SSE/IPC 漂移 | 丢 token、重复完成、取消失效 | 唯一 Rust contract + golden fixture + reducer parity |
| Chat-first 被页面迁移稀释 | 隐式 Workspace、文件归属错误 | 第 7 节硬验收；单 ChatCanvas/单执行管线 |
| 路由漏迁 | 删除 Next 后 404、SEO/交易中断 | 全 route manifest；默认保留，显式下线 |
| Markdown/XSS 回归 | 安全事故 | allowlist sanitizer + 恶意 fixture + CSP |
| 编辑器被低估 | 数据丢失、撤销/粘贴退化 | 采用成熟库或把 route 留作阻塞项 |
| Tauri 被当普通浏览器 | 登录、流取消、上传、升级失效 | 独立 CSR 目标 + Tauri adapter + 安装包 E2E |
| Rust server 再代理 API | SSE buffering、双跳、超时差异 | 生产 Nginx 直达后端；server 只做 SSR/static |
| Token 分叉 | 多站点视觉漂移 | `packages/cos-tokens` 唯一源 + 同步校验 |
| 一次性切换 | 故障面过大、难定位 | route family 分层切流 + 版本化 rollback |

## 14. 完成定义

只有以下全部成立，项目才可称为“Next 前端迁移到 Rust 完成”：

- Rust Web 生产服务承担 route manifest 中全部 Web 路由和静态/SSR 职责；
- 生产 API/SSE 仍按既有 Nginx 边界直达后端；
- Chat-first、Session files、Workspace files、Web、模型角色、独立 quick-chat BYOK 和分享权限完整；
- Browser HTTP/SSE 与 Tauri IPC 共用契约/reducer，并通过各自端到端验证；
- auth、billing、admin、legal、双语、SEO 与 metadata 无缺项；
- 安全、可访问性、响应式和性能指标达到 Phase 0 约定门槛；
- `frontend_next` 及其生产 service 已删除，没有长期兼容层；
- Node.js 不再是 Web 生产运行时；保留的 Node 工具都有明确的开发/验收用途；
- 部署与 rollback 只走仓库脚本，相关权威文档和 code-review graph 已更新。

## 15. 实现参考

- Cargo workspace：<https://doc.rust-lang.org/cargo/reference/workspaces.html>
- cargo-leptos 的 SSR/hydrate 构建模型：<https://github.com/leptos-rs/cargo-leptos>
- Tauri 前端配置与静态 CSR 边界：<https://v2.tauri.app/start/frontend/>

上述外部文档用于实现机制；产品语义仍以本仓库权威文档与契约为准。
