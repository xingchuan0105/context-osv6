# Context-OS 客户端：用户旅程与本地化能力差距审计

**日期**: 2026-07-14  
**状态**: Audit（检查结论；未改产品逻辑）  
**触发**: 纠正「双击后 Login」与「本地全栈」认知偏差，对齐真实用户旅程  

---

## 1. 目标旅程（你描述的正确逻辑）

```text
双击打开客户端
    │
    ├─ 未授权 → 欢迎页（非 Login）
    │              ├─ 试用 21 天
    │              └─ 购买授权
    │
    └─ 已授权 / 试用中 → 直接进入工作区
                           │
                           ├─ 本地 PG / Milvus / Redis / 文档解析
                           └─ 设置页：LLM + Embedding（BYOK）
```

**与网页端的本质区别**：

| 维度 | 网页 SaaS | 客户端（目标） |
|------|-----------|----------------|
| 入口 | 账号登录 | **设备许可**，无强制 Login |
| 数据 | 云端 PG/Milvus/对象存储 | **本机**数据面 |
| 算力 | 服务端代理 LLM | **用户自带 Key（BYOK）** |
| 身份 | user_id / JWT | 本机 workspace + license |

---

## 2. 现状旅程（代码实际行为）

### 2.1 启动路由

| 步骤 | 实际 | 目标 |
|------|------|------|
| 首页 `app/page.tsx` | cookie 有会话 → `/dashboard`，**否则 → `/login`** | 有许可 → 工作区；无许可 → 欢迎/激活 |
| `(app)/layout` | **`ProtectedRouteGate` 一律要求登录**，否则踢 `/login` | 客户端应 **LicenseGate**，不要求云账号 |
| 激活页 `/activate` | 有试用/输码 UI | 可作欢迎页基底 |
| 配置页 `/setup` | LLM 预设引导 → 然后 `/dashboard` | 可作设置/onboarding 基底 |

**结论**：只要用户打开「工作区类」路由，就会被 **Login 墙**挡住。双击后看到 Login **不是偶然**，是 Web 路由默认逻辑。

### 2.2 授权 / 试用

| 能力 | 现状 | 目标 |
|------|------|------|
| 试用 | 有 `start_trial`；设计/ADR 写 **7 天**，非 21 天 | **21 天** |
| 试用依赖 | `POST {云端}/api/v1/licenses/trial`（需联网 + 服务端） | 可本地签发试用（可选联网校验） |
| 买断激活 | Keygen 路径 + deep link `avrag-desktop://activate` | 保留；欢迎页入口清晰 |
| 聊天门禁 | 无有效 license → 拒绝 chat | OK |
| 欢迎页 | `/activate` 接近，但 **未设为冷启动默认首页** | 冷启动默认欢迎 |

### 2.3 LLM / Embedding 设置

| 能力 | 现状 | 目标 |
|------|------|------|
| LLM 配置持久化 | ✅ `llm-config.json`（app data 目录） | 有 |
| 多厂商预设 | ✅ `/setup` + `LLM_PRESETS` | 有 |
| 连通性测试 / 诊断 | ✅ `test_llm_connection` / `diagnose_llm` | 有 |
| Embedding 配置结构 | ✅ `LocalEmbeddingConfig` 字段已在配置模型中 | 有 |
| **常驻设置页** | ⚠️ 仅 `DesktopSettingsDrawer`（侧栏抽屉）+ `/setup` onboarding | 需要明确 **设置页** 入口（工作区内稳定可达） |
| Embedding 真正参与索引 | ⚠️ 配置可存；**本地完整 ingest→embed→向量库链路未闭环** | 需本地向量管道 |

**结论**：LLM 配置「有壳、有页、有测通」；**不是**「本地 RAG 全链路已齐」。

### 2.4 本地数据面（PG / Milvus / Redis / 解析）

| 组件 | 目标（你说的） | 现状 | 差距 |
|------|----------------|------|------|
| **PostgreSQL** | 本机部署/内嵌 | ❌ 未内嵌、未随客户端起 | 大 |
| **Milvus** | 本机向量库 | ❌ 未内嵌 | 大 |
| **Redis** | 本机缓存/队列 | ❌ 未内嵌 | 大 |
| **文档解析** | 本机解析 PDF 等 | ❌ 无完整本地 ingest worker | 大 |
| 文档/chunk 文件 | 本地落盘 | ✅ `storage-local`（JSON 文件目录） | 仅「文件级内容存储」，非 PG schema |
| 进程内缓存 | 有 | ✅ `LocalCache` | 非 Redis |
| 对象存储 | 本地目录 | ✅ 本地 `content/` 路径 | 非 MinIO |

当前桌面「本地后端」实质是：

```text
init_local_backend
  → LocalContentStore（文件系统 docs/chunks JSON）
  → LocalCache（内存）
  → 无 PG / 无 Milvus / 无 Redis / 无独立 ingest worker
```

**没有**把云端栈（Postgres + Milvus + Redis + worker）打包成 sidecar 或嵌入式替代。

### 2.5 聊天 / RAG 深度

| 能力 | 现状 |
|------|------|
| `chat_stream` IPC | 有；受 license 门禁 |
| 桌面 chat 实现 | 有本地 LLM 调用路径（BYOK） |
| 与云端同等级 RAG 工具链 | **未**等价：无完整 workspace 产品管道 + 本地向量检索闭环 |

---

## 3. 能力对照总表

| # | 能力 | 做过？ | 完成度 | 说明 |
|---|------|--------|--------|------|
| J1 | 冷启动不进 Login | ❌ | 0% | 根路由 + ProtectedRouteGate 强制云登录 |
| J2 | 欢迎页：试用/购买 | ⚠️ | ~40% | `/activate` 有试用与买链；非默认首页；试用 7 天 |
| J3 | 试用 21 天 | ❌ | 0% | ADR/实现为 **7 天** |
| J4 | 已授权直进工作区 | ⚠️ | ~20% | 有 license 仍可能被 **Login 墙**挡在 dashboard 外 |
| J5 | 本机 PostgreSQL | ❌ | 0% | 未部署 |
| J6 | 本机 Milvus | ❌ | 0% | 未部署 |
| J7 | 本机 Redis | ❌ | 0% | 未部署 |
| J8 | 本机文档解析/ingest | ❌ | ~5% | 无 worker 管道；storage-local 只存文件 |
| J9 | LLM 设置 | ✅ | ~80% | setup + drawer + 测试/诊断 |
| J10 | Embedding 设置 | ⚠️ | ~40% | 配置模型有字段；UI/管道不完整 |
| J11 | 设置页产品化 | ⚠️ | ~50% | 有抽屉与 setup，缺「工作区设置」一等公民页 |
| J12 | 安装包/下载/许可购买网页 | ✅ | ~70% | 下载/NSIS/授权购买页已有（云侧） |

---

## 4. 根因（为何会做成「像网页」）

1. **UI 复用 `frontend_next` 整站静态导出**，默认带着 SaaS 的 Login / ProtectedRoute。  
2. **阶段设计**（`desktop/AGENTS.md`）早期允许 localhost HTTP / 渐进 IPC，本地全栈未列为 M0。  
3. **ADR-0004** 定位是「BYOK + 软件许可 + storage-local」，**不是**「嵌入 PG+Milvus 私有化小集群」。  
4. **storage-local** 是刻意轻量替代对象存储，**不是**云端数据库的桌面版。

你现在的纠正 = **产品定义升级**：从「薄壳 + BYOK + 文件存储」→「本地全功能运行时 + 许可门禁 + 无云账号」。

---

## 5. 目标架构（修正后，供后续实现）

```text
┌─────────────────────────────────────────────────────────┐
│  WebView UI (frontend_next desktop build)                 │
│  冷启动路由：License 状态机，而非 Auth cookie               │
└───────────────────────────┬─────────────────────────────┘
                            │ Tauri IPC only
┌───────────────────────────▼─────────────────────────────┐
│  Desktop Runtime (Rust)                                   │
│  · License store (trial 21d / paid)                       │
│  · LLM + Embedding config (settings)                      │
│  · Orchestration: ingest / chat / search                  │
└───────┬─────────────────┬─────────────────┬─────────────┘
        │                 │                 │
   ┌────▼────┐      ┌─────▼─────┐     ┌─────▼─────┐
   │ 本地元数据 │      │ 本地向量   │     │ 本地队列  │
   │ SQLite 或  │      │ 嵌入式向量 │     │ 内存/文件 │
   │ 嵌入式 PG  │      │ (非必 Milvus│     │ 不必 Redis│
   └──────────┘      │  单机)     │     └───────────┘
                     └───────────┘
   文档解析：本机解析库 / sidecar（非云 worker）
```

### 技术建议（避免在笔记本上硬扛完整 Docker 栈）

| 云端组件 | 桌面推荐替代 | 原因 |
|----------|--------------|------|
| PostgreSQL | **SQLite**（或可选 embedded PG） | 安装零运维、单用户足够 |
| Milvus | **本地向量索引**（如 sqlite-vec / 专用 crate / lance） | Milvus 体积与运维过重 |
| Redis | **进程内队列 + 文件锁** | 单机无必要 Redis |
| Ingest worker | **同进程 async worker** 或 轻量 sidecar | 不必独立容器 |

若坚持「真 PG + 真 Milvus + 真 Redis」，需接受：**安装包巨大、后台服务多、Win 权限/端口冲突**，适合「企业私有化安装包」，不适合默认消费级客户端。

---

## 6. 建议实施路线（与现网解耦）

### Wave D0 — 旅程闸门（优先，体感立刻变）

1. 桌面 build：`/` 与 `(app)` **绕过 ProtectedRouteGate**  
2. 冷启动状态机：`Unactivated → /activate（欢迎）`；`Active/Trial → /dashboard`  
3. 试用天数配置改为 **21**（并统一文案/服务端 trial 策略）  
4. 欢迎页：试用 / 购买 / 已有授权码（现 `/activate` 增强）  

**不依赖** PG/Milvus 即可交付。

### Wave D1 — 设置与 BYOK 产品化

1. 工作区内稳定 **设置** 入口（LLM + Embedding）  
2. Embedding 配置 UI 完整 + 测通  
3. 未配置 LLM 时引导 `/setup`，而非 Login  

### Wave D2 — 本地数据面 MVP

1. 选定嵌入式存储方案（推荐 SQLite + 本地向量）  
2. 本机 ingest：解析 → chunk → embed → 索引  
3. 本机 search + chat 引用本地索引  
4. 数据目录：`app_data/workspace/` 可备份  

### Wave D3 — （可选）重量级本地栈

仅当明确要「与云端同构」时：sidecar Docker Compose 或嵌入式二进制捆绑。**默认不做。**

---

## 7. 直接回答你的问题

> 双击打开后不应该是 LOGIN，而是工作区 / 欢迎页？

**对。现状是 Login，不符合目标。**

> 欢迎页可买授权或试用 21 天？

**欢迎/激活 UI 有一部分；默认路由未接好；试用是 7 天不是 21。**

> 应集成 PG、Milvus、Redis、文档解析并本地化？

**目标若成立，当前几乎都没做。** 只有轻量文件存储 + 内存缓存 + LLM 配置骨架。

> 设置页配置 LLM、embedding，做了吗？

**LLM：大部分做了（setup + 抽屉 + 测试）。Embedding：配置结构有，产品闭环不够。设置作为一等公民页面仍弱。**

---

## 8. 建议你拍板的决策

| # | 决策 | 建议默认 |
|---|------|----------|
| D1 | 客户端是否仍强制云账号 Login | **否** |
| D2 | 试用天数 | **21 天** |
| D3 | 本地数据栈 | **SQLite + 本地向量**（不默认捆绑 PG/Milvus/Redis） |
| D4 | 是否允许可选「连接已有云端/自建服务端」 | 二期高级模式 |
| D5 | 先做 D0 旅程 还是 先做 D2 本地 RAG | **先 D0**（用户第一印象） |

---

**下一步**：你确认 D1–D5 后，可按 **D0 → D1 → D2** 开工。本文件只做检查与编排，未改客户端路由逻辑。
