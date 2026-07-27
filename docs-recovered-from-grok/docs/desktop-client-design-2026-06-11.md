# 本地客户端设计文档：Web UI 复用 + Tauri 桌面壳 + 本地 Rust 核心

> 日期：2026-06-11　状态：阶段 2-3 部分完成（本地存储层可用，IPC 传输骨架已搭建，聊天/REST 核心链路待接入）
> 目标平台：Linux + Windows（macOS 需 macOS 环境构建，暂不支持）
> 适用范围：把现有 `frontend_next`（Next.js）+ `avrag-rs`（Rust 后端）演进为桌面本地客户端，而非把线上 VPS 部署原样搬到本机。

---

## 0. 结论先行（TL;DR）

- **采纳的方向**：界面层继续用 `frontend_next`，但桌面端只输出**静态资源**；桌面容器与系统权限交给 **Tauri 2**；本地能力由 **现有 `avrag-rs` Rust 核心**承载，而不是在客户端里再跑一个 Next server。
- **本项目的最大优势**：前端对后端的调用**已经收口在一个传输接缝**（`buildApiUrl` + `streamWorkspaceChat`/`request<T>`），后端 SSE 也是由一个内部 `ChatEvent` 流包装而来。所以桌面化主要是"换传输层"，不是"重写业务"。
- **对参考方案的一处纠偏**：参考资料建议新建 `apps/web + apps/desktop + crates/core` 的全新 monorepo。但本仓库已有成熟的 `frontend_next` 和 `avrag-rs` workspace，**不应推倒重排目录**。本文改为：复用现有两套工程 + 新增一个薄桌面壳 `desktop/`。
- **最大的坑**：把云端 PostgreSQL / Redis / Milvus / MinIO 那套分布式基础设施原样搬到本机，会让安装、升级、故障面急剧放大。桌面端要走**嵌入式/本地模式**。

---

## 1. 背景与目标

当前形态是 Web 全栈：

- `frontend_next`：Next.js 16，`output: "standalone"`，通过 `rewrites()` 把 `/api/*` 代理到本机 Rust API（默认 `http://127.0.0.1:8080`）。
- `avrag-rs`：Rust workspace，产出三个二进制——`bins/api`（HTTP/SSE 服务）、`bins/worker`（异步摄取/分析）、`bins/office-parser-jvm`。后端依赖 Postgres、Redis、Milvus、MinIO。

目标：交付一个**桌面优先**的本地客户端，启动更快、资源占用更低、对本地文件/系统集成更强，同时**尽量复用现有 Web UI 与 Rust 业务核心**，并保留未来继续维护 Web 版的可能性（协议统一、传输分离）。

---

## 2. 现状盘点（基于真实代码的事实）

这一节是后续所有设计决策的依据，全部来自当前代码。

### 2.1 前端的传输接缝（关键优势）

前端**没有**把后端调用散落各处，而是收口在两个文件：

- `frontend_next/lib/auth/client.ts`
  - `getApiBaseUrl()` 读取 `NEXT_PUBLIC_API_BASE_URL`；为空时回退到 `window.location.origin`（同源）。
  - `buildApiUrl(path)` 是**唯一**的 URL 拼装点。
  - `request<T>(path, init, token)` 是普通 REST 的统一封装（Bearer token、`cache: "no-store"`、错误解码）。
- `frontend_next/lib/workspace/stream.ts`
  - `streamWorkspaceChat(token, request, onEvent)`：发起 `POST /api/v1/chat`，`Accept: text/event-stream`，把响应体交给解析器。
  - `parseWorkspaceChatEventStream(stream, onEvent)`：把 SSE 帧解析为强类型 `WorkspaceChatStreamEvent`（`start / activity / answer_start / trace / token / reasoning_summary_delta / citations / done / error`）。

> 含义：**只要替换 `buildApiUrl` 的目标和 `streamWorkspaceChat` 的传输实现，整套 UI 就能在桌面端工作**，业务/渲染代码零改动。这是本项目最值钱的接缝。

### 2.2 鉴权模型（对桌面友好）

- Token 为 **Bearer token**，由后端 `/api/auth/login` 返回，前端写入 `avrag.auth.persisted` cookie（`server-session.ts`）。
- `middleware.ts` 只根据 `avrag.auth.session` cookie 是否存在做**跳转**（登录态 UX），不是安全边界。
- 含义：桌面端可以把 token 存进 Tauri 安全存储/本地 store，鉴权逻辑无需重写；middleware 的跳转可改为客户端守卫。

### 2.3 后端的流式真相源（关键优势）

`avrag-rs/crates/transport-http/src/handlers.rs`：

- HTTP 层用 Axum `Sse` 把一个内部 **`ChatEvent`** 流包装成命名 SSE 事件（`sse_event_name(&event)` + `sse_event(name, payload)`）。
- 也就是说，**SSE 只是 `ChatEvent` 的一种传输编码**。桌面端完全可以在 Rust 内直接消费同一个 `ChatEvent` 流，转发到 Tauri 事件，而不经过 HTTP。

### 2.4 静态导出（`output: 'export'`）的三个真实障碍

桌面端要求 Next.js 静态导出（官方明确：Tauri 不支持依赖服务器运行时的方案）。当前代码有三处依赖服务端运行时：

1. **动态路由**：`app/(app)/dashboard/[workspace_id]/...`、`app/shared/kb/[token]`、`app/admin/organizations/[org_id]`、`app/invite/[workspace_id]/[member_id]`。静态导出要求每个动态段提供 `generateStaticParams`，而这些 ID 是运行时用户数据，构建期未知。
2. **`next-intl` 服务端取 locale**：`i18n/request.ts` 用 `cookies()` + `getRequestConfig`，`app/layout.tsx` 用 `getLocale()/getMessages()`（服务端 API）。
3. **middleware 跳转**：静态导出不运行 middleware。

> 这三点不是阻断性的，但是桌面化的主要改造工作量所在（见 §5）。

### 2.5 后端基础设施依赖（最大的本地化挑战）

`bins/api` + `bins/worker` 依赖 Postgres、Redis、Milvus、MinIO。把这一整套搬到桌面是反模式（见 §6 的本地映射方案）。

---

## 3. 推荐形态：三层结构

```
┌──────────────────────────────────────────────┐
│  Next.js 静态资源 (frontend_next → out/)        │  只做展示与交互
│  - React UI / TipTap / react-query / zustand    │  不依赖 SSR、API Routes、middleware
└───────────────┬──────────────────────────────┘
                │  Tauri WebView 加载 frontendDist=out/
┌───────────────▼──────────────────────────────┐
│  Tauri 2 桌面壳 (desktop/)                      │  窗口 / 权限 / 系统集成 / 安全存储
│  - tauri::command（轻能力）                      │  - 把前端请求映射到本地核心
│  - 事件总线（流式转发）                           │  - sidecar 生命周期管理
└───────────────┬──────────────────────────────┘
                │  进程内调用 or sidecar IPC
┌───────────────▼──────────────────────────────┐
│  本地 Rust 核心 (复用 avrag-rs/crates/*)         │  配置 / 任务编排 / 检索 / 流式处理
│  - 轻能力：进 Tauri 主进程                        │
│  - 重能力：拆 sidecar（独立生命周期/崩溃隔离）      │
└──────────────────────────────────────────────┘
```

判断标准（贯穿全文）：

| 能力性质 | 归属 |
|---|---|
| 只负责展示与交互 | 留在 Next 前端 |
| 必须和窗口/文件/系统权限强绑定 | Tauri 主进程（command / plugin） |
| 长时间运行、资源重、需要隔离重启（向量检索、LLM 长连接、摄取流水线） | sidecar / 本地 daemon |

---

## 4. 进程模型与通信

### 4.1 两种传输，一套协议

核心原则：**协议统一，传输分离**。`ChatEvent` / `WorkspaceChatStreamEvent` 是单一真相源，浏览器和桌面只是不同的搬运方式。

- **Web 版**：浏览器 `fetch` SSE → `parseWorkspaceChatEventStream`（现状，保持不变）。
- **桌面版**：前端调用 Tauri command → Rust 核心产出 `ChatEvent` 流 → 通过 Tauri **事件**（`emit`）推回前端 → 前端监听事件、复用同一套 reducer。

### 4.2 前端侧的传输抽象（新增一层薄适配器）

在 `frontend_next/lib` 新增一个运行时探测 + 适配层（建议命名 `lib/runtime/transport.ts`）：

```ts
// 伪代码：根据运行环境选择传输实现
export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export async function streamChat(token, request, onEvent, options) {
  if (isTauri) {
    // 1. listen("chat://<request_id>", e => onEvent(e.payload))
    // 2. invoke("chat_stream", { token, request })
    // 3. 收到 done/error 后 unlisten
  } else {
    return streamWorkspaceChat(token, request, onEvent, options); // 现状 Web 路径
  }
}
```

- REST 同理：桌面端把 `request<T>` 的 fetch 换成 `invoke("api_call", { method, path, body, token })`，由 Tauri 主进程转调本地核心。
- **关键收益**：UI 组件、reducer、类型定义全部零改动，只在传输边界分叉。

> 备选传输：也可以让本地 Rust 核心继续在 `127.0.0.1` 上暴露 HTTP/SSE，前端仍走 `buildApiUrl` 指向 localhost。这条路改动最小（几乎只改 base URL），但放弃了 Tauri IPC 的权限边界与"无需开本地端口"的优势。**建议桌面优先走 IPC，localhost-HTTP 作为过渡期兜底**。

---

## 5. 关键改造点（静态导出落地）

按工作量从小到大：

### 5.1 传输接缝替换（小，价值最高）

- 新增 `lib/runtime/transport.ts`，在 `streamWorkspaceChat` 和 `request<T>` 之上加运行时分叉（§4.2）。
- 调用方从直接 import `streamWorkspaceChat` 改为 import 适配层。

### 5.2 `next.config.ts` 切换

- 桌面构建：`output: 'export'`，移除 `rewrites()`（静态导出不支持），把 `out/` 作为 Tauri `frontendDist`。
- Web 构建：保持 `output: 'standalone'` + `rewrites()`。
- 实现方式：用 `process.env.BUILD_TARGET=desktop` 区分两套配置，避免维护两份代码。

### 5.3 动态路由改造（中）

静态导出无法预渲染运行时 ID。两条可选路径：

- **方案 A（推荐，改动可控）**：对桌面端，把 `[workspace_id]` 等动态段配 `generateStaticParams` 返回占位/空集 + 客户端取参；实际 ID 走客户端路由（query/hash）或在客户端组件内读取。Workspace 页面本身已是 `<WorkspaceSurface workspaceId={...} />` 的薄壳，迁移成本低。
- **方案 B**：桌面端整体走 SPA 外壳（单一 `index.html` + 客户端路由），仅保留必要的静态页面。改动大，但与"桌面只做展示"的定位最契合。

> 建议先用方案 A 打通主链路（dashboard/workspace/chat），admin 与 marketing 页面桌面端可暂不打包。

### 5.4 `next-intl` 改为客户端模式（中）

- 去掉 `i18n/request.ts` 对 `cookies()` 的依赖；locale 改为客户端读取（本地 store / Tauri 存储）。
- `app/layout.tsx` 改用客户端 `NextIntlClientProvider` + 静态打包的 message catalog（`lib/i18n/messages.ts` 已有 catalog）。

### 5.5 middleware 跳转改为客户端守卫（小）

- 静态导出不跑 middleware。把 `resolveMiddlewareAction` 的登录态跳转逻辑下沉到客户端布局守卫（已有 `AuthProvider`，挂一个 redirect effect 即可）。

---

## 6. 本地数据栈映射（避免照搬云端）

桌面化的核心反模式就是把云端分布式套件搬到本机。映射建议：

| 云端组件 | 桌面本地做法 | 备注 |
|---|---|---|
| PostgreSQL | 嵌入式 SQLite（或本地单实例 PG） | `storage-pg` 是一个 crate seam；新增 `storage-sqlite` 适配同一 port 是干净做法。需评估 SQL 方言差异。 |
| Milvus（向量） | 嵌入式向量索引（如本地 HNSW/sqlite-vss 等） | `storage-milvus` 同样是 seam；按本地规模选轻量实现。 |
| Redis（限流/缓存） | 进程内缓存 + 本地任务调度 | 桌面单用户无需分布式限流；`cache-redis` 退化为 in-memory 实现。 |
| MinIO（对象存储） | 本地文件系统目录 | 文档/资源直接落盘到应用数据目录。 |
| `bins/worker`（异步摄取） | 主进程内任务队列 或 sidecar | 单用户量级可进程内；若摄取重/易崩，拆 sidecar 隔离。 |

> 注意：这些都是**适配新的 port 实现**，不是改业务。`avrag-rs` 已用 crate 分层（`storage-pg` / `storage-milvus` / `cache-redis`），是天然的替换点。是否值得做嵌入式实现，取决于桌面版要支持的数据规模——**先用 §9 阶段 0 验证瓶颈，再决定哪些 port 真的需要本地实现**，避免过度工程。

---

## 7. 工程落地（复用优先，不重排目录）

不新建 greenfield monorepo，而是在现有结构上**加一层**：

```
context-osv6/
├── frontend_next/          # 复用：新增桌面静态导出配置 + 传输适配层
├── avrag-rs/               # 复用：Rust 业务核心（crates/*）
│   └── crates/             # 新增本地 port 适配（storage-sqlite 等，按需）
└── desktop/                # 新增：Tauri 2 桌面壳（薄）
    ├── src-tauri/          #   - tauri::command / 事件转发 / sidecar 管理
    │   └── Cargo.toml      #   - 依赖 avrag-rs 的核心 crate（path 依赖）
    └── tauri.conf.json     #   - frontendDist = ../frontend_next/out
```

- `desktop/src-tauri` 通过 **path 依赖**引用 `avrag-rs/crates/*` 中的核心库，复用业务逻辑。
- 开发流程：`cargo tauri dev` 联动 `next dev`（dev 模式下仍可走 localhost HTTP，便于热重载）；发布走 `next build`（export）+ `cargo tauri build`。
- **第一天起的纪律**：不要把关键逻辑写进 Next 专属服务端层（API Routes / server actions / middleware 安全逻辑），否则桌面化会反复返工。当前代码这点做得不错——业务都在 Rust，前端只是消费者。

---

## 8. 统一流式协议（单一真相源）

把 `ChatEvent`（Rust）↔ `WorkspaceChatStreamEvent`（TS）这对类型确立为协议契约：

- **Web 传输**：`ChatEvent` → SSE 编码 → TS 解析（现状）。
- **桌面传输**：`ChatEvent` → Tauri 事件 payload（JSON）→ TS 直接消费（无需 SSE 解析）。
- 两侧共用同一组事件 `kind`，前端 reducer 不区分来源。
- 契约变更时，`contracts/` crate + TS 类型同步更新（项目已有 `contracts/` 与 `frontend_next/tests/workspace/stream.test.ts` 契约测试，可作为回归保障）。

---

## 9. 分阶段路线图

每个阶段都有可验证的成功标准（遵循"目标驱动"）。

- **阶段 0：可行性验证（spike）**
  - 用最小改动让现有 `frontend_next`（dev 模式）跑在 Tauri WebView 里，前端走 localhost 指向已有 `bins/api`。
  - 验证：聊天 SSE 在 Tauri WebView 内能正常流式渲染。
  - 产出：确认 WebView 兼容性与 SSE 行为，识别静态导出的真实阻碍清单。

- **阶段 1：静态导出打通**
  - 完成 §5.2–5.5（`output: export`、动态路由方案 A、next-intl 客户端化、middleware 客户端守卫）。
  - 验证：`next build` 产出 `out/`，Tauri 加载后主链路（登录 → dashboard → workspace → 聊天）可用。

- **阶段 2：IPC 传输接缝**
  - 完成 §4.2 传输适配层 + Tauri command/事件；本地核心以 path 依赖嵌入 `desktop/src-tauri`。
  - 验证：关闭 localhost HTTP，聊天与 REST 全走 IPC，UI 零改动通过。

- **阶段 3：本地数据栈**
  - 按 §6 评估并实现真正需要的本地 port（大概率从 MinIO→本地FS、Redis→内存 开始，PG/Milvus 视规模决定）。
  - 验证：桌面端可离线完成文档导入 → 摄取 → 检索 → 引用问答的闭环。

- **阶段 4：打包与系统集成**
  - sidecar 打包（如摄取/向量服务独立）、安全存储 token、自动更新、安装包。
  - 验证：干净机器安装即用，无需手动起 Postgres/Redis/Milvus/MinIO。

---

## 10. 风险与最容易踩的坑

1. **把 Next 当桌面 server 用**：禁止依赖 SSR / API Routes / middleware 安全逻辑。本项目业务在 Rust，风险低，但要守住 §7 的纪律。
2. **把桌面内部通信强行套成 HTTP-only**：桌面优先 IPC/事件；localhost-HTTP 仅作过渡兜底。
3. **把云端 PG/Redis/Milvus/MinIO 原样搬到本机**：安装/升级/故障面爆炸。走嵌入式/本地模式（§6）。
4. **动态路由与 i18n 的静态导出改造被低估**：这是阶段 1 的主要工作量，需提前排期。
5. **过度工程本地数据层**：不要一上来就写 SQLite/HNSW 适配。先用阶段 0/1 验证桌面真实数据规模，再决定哪些 port 值得本地化。

---

## 附录：与参考最佳实践的对应关系

| 参考建议 | 本项目落地 |
|---|---|
| Next.js 只做静态导出 | §5.2 `output: 'export'`，双构建目标 |
| Tauri 2 负责窗口/权限/系统集成 | §3 桌面壳层，§7 `desktop/src-tauri` |
| 轻能力进主进程，重能力拆 sidecar | §3 判断标准表，§6 worker 归属 |
| 桌面优先 IPC/events，Web 保留 SSE | §4 协议统一/传输分离，§8 单一真相源 |
| monorepo 隔离 web/desktop/core | §7 纠偏：复用现有 `frontend_next`/`avrag-rs` + 新增薄 `desktop/`，不重排目录 |
| 本地数据走嵌入式 | §6 云端→本地映射表 |

---

## 附录：实施记录

### 2026-06-11：阶段 0 完成（Linux 构建验证通过）

**已完成：**

1. **创建 Tauri 2 桌面壳项目结构** (`desktop/`)
   - `desktop/src-tauri/tauri.conf.json` - Tauri 配置，前端指向 `frontend_next/out`
   - `desktop/src-tauri/Cargo.toml` - Rust 依赖配置
   - `desktop/src-tauri/src/main.rs` - 主进程入口
   - `desktop/src-tauri/src/lib.rs` - Tauri command 定义（`get_app_data_dir`, `is_tauri_environment`, `get_app_version`）
   - `desktop/package.json` - Node.js 依赖（`@tauri-apps/cli`, `@tauri-apps/api`）

2. **修改前端配置支持静态导出** (`frontend_next/`)
   - `next.config.ts` - 支持双构建目标（`BUILD_TARGET=desktop` 时切换为 `output: 'export'`）
   - `package.json` - 添加 `build:desktop` 脚本

3. **创建传输适配层** (`frontend_next/lib/runtime/`)
   - `transport.ts` - 运行时环境检测 + 传输分叉（Web SSE vs Tauri IPC）
   - `client-guard.tsx` - 客户端认证守卫（替代 middleware.ts 的登录态跳转）
   - `client-i18n.tsx` - 客户端 i18n Provider（替代服务端 getRequestConfig）

4. **创建构建脚本** (`scripts/`)
   - `build-desktop.sh` - 桌面客户端构建脚本（支持指定平台）

5. **前端静态导出改造**（`frontend_next/`）
   - 动态路由全部改为客户端组件 + `generateStaticParams` 占位
   - 服务端 API（`cookies()`, `getLocale()`）改为客户端实现
   - `force-dynamic` 页面改为客户端组件
   - `ImageResponse` 图标路由改为静态重定向

6. **构建验证** ✅
   - `BUILD_TARGET=desktop pnpm build` 成功产出 `out/` 目录（41 个静态页面）
   - `pnpm tauri build` 成功构建 Linux 桌面应用
   - `pnpm tauri build --target x86_64-pc-windows-gnu` 成功构建 Windows 桌面应用
   - macOS 构建需要 macOS SDK（来自 Xcode），无法从 Linux 交叉编译
   - 产出文件：
     - Linux 二进制：`desktop/src-tauri/target/release/avrag-desktop`（13.6 MB）
     - Linux deb 包：`AVRag Desktop_0.1.0_amd64.deb`（5.0 MB）
     - Linux rpm 包：`AVRag Desktop-0.1.0-1.x86_64.rpm`（5.0 MB）
     - Windows 二进制：`desktop/src-tauri/target/x86_64-pc-windows-gnu/release/avrag-desktop.exe`（21.4 MB）

7. **macOS 构建指南** (`docs/macos-build-guide.md`)
   - 说明为什么不能从 Linux 交叉编译
   - 提供三种构建方式：本地 macOS、GitHub Actions CI/CD、osxcross

**阶段 0 验证结果：**

- [x] 生成图标文件（SVG → PNG via rsvg-convert，PNG → ICO via icotool）
- [x] 前端静态导出构建成功
- [x] Tauri 桌面应用构建成功（Linux）
- [x] Tauri 桌面应用构建成功（Windows 交叉编译）
- [x] macOS 构建指南完成（需要 macOS 环境，已提供详细指南和 CI/CD 配置）
- [ ] WebView 兼容性验证（需要桌面环境运行）
- [ ] SSE 流式渲染验证（需要桌面环境运行）

**下一步（阶段 1）：**

- WebView 兼容性测试
- SSE 流式渲染测试
- 客户端认证守卫集成测试

---

### 2026-06-12：阶段 2-3 本地后端功能实施

**已完成：**

1. **创建本地存储适配器** (`avrag-rs/crates/storage-local/`)
   - `local_content_store.rs` - 本地文件系统内容存储（替代 MinIO）
     - 实现 `ContentStore` trait
     - 文档和 chunk 存储在本地文件系统
   - `local_cache.rs` - 本地内存缓存（替代 Redis）
     - 实现 `CachePort` trait
     - 支持 TTL 过期机制

2. **更新 Tauri 后端命令** (`desktop/src-tauri/src/lib.rs`)
   - `init_local_backend` - 初始化本地存储
   - `get_backend_status` - 获取后端状态
   - `list_local_documents` - 列出本地文档
   - `get_cache_value` / `set_cache_value` - 缓存操作

3. **创建前端 IPC 传输层** (`frontend_next/lib/runtime/tauri-ipc.ts`)
   - `initLocalBackend()` - 初始化本地后端
   - `getBackendStatus()` - 获取后端状态
   - `listLocalDocuments()` - 列出文档
   - `streamChatViaIPC()` - 流式聊天（预留）
   - `requestViaIPC()` - REST 请求（预留）

4. **更新传输适配层** (`frontend_next/lib/runtime/transport.ts`)
   - 桌面端使用 Tauri IPC 替代 HTTP
   - Web 端保持不变

**架构变化：**

```
阶段 0-1（已完成）：
┌─────────────┐     HTTP/SSE     ┌─────────────┐
│  桌面客户端   │ ─────────────── → │  远程后端    │
│  (前端壳)    │                  │  (VPS)      │
└─────────────┘                  └─────────────┘

阶段 2-3（实施中）：
┌─────────────┐     Tauri IPC    ┌─────────────┐
│  桌面客户端   │ ─────────────── → │  本地后端    │
│  (前端)      │                  │  (嵌入式)    │
└─────────────┘                  └─────────────┘
```

**待完成：**

- [ ] 实现 Tauri command `chat_stream`（流式聊天）
- [ ] 实现 Tauri command `api_call`（REST 请求代理）
- [ ] 集成 LLM 客户端（本地或远程）
- [ ] 集成向量检索（本地 HNSW 或远程 Milvus）
- [ ] 端到端测试
- [ ] `chat_stream` command 当前为骨架实现（占位响应），需接入真正的 LLM 调用和 RAG 流水线
- [ ] `api_call` command 当前为占位响应，需接入本地 Rust 核心的 API 处理
- [ ] 传输适配层 (`lib/runtime/transport.ts`) 已创建但尚未被 UI 调用方接入

**下一步（阶段 4）：**

- 打包 sidecar（如摄取/向量服务独立）
- 安全存储 token
- 自动更新
- 安装包生成
