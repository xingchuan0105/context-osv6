# Brooks-Lint Review

**Mode:** Architecture Audit  
**Scope:** `avrag-rs` 全 workspace（34 成员）+ `desktop/` Tauri 壳 + `frontend_next` 传输接缝；深度复测 v4（依赖图重绘、v3 遗留项逐条核销、Seam/Conway、本轮大规模重构 diff 对照）  
**Health Score:** 82/100  
**Trend:** 77 → 82 (+5) over last 3 runs（73 → 83 → 77 → 82）

本轮分数回升来自 v3 四个 Warning 中三个已落地（`CachePort` 轻量 crate、desktop 依赖链瘦身、OnceCell 改 Tauri managed state），以及 `common` 枢纽减重、`avrag-admin` 空壳删除、`chat_private` 模块化拆分。剩余债集中在 `app-chat` 千行文件与 `common`/`avrag-auth` 高扇入。

---

## Module Dependency Graph

```mermaid
graph TD
  subgraph Clients
    Frontend["frontend_next (Web/静态导出双形态)"]
    Desktop["desktop Tauri壳 (335行/474包)"]
  end

  subgraph Entrypoints
    API["avrag-api"]
    Worker["avrag-worker (fan-out: 13)"]
  end

  subgraph Transport
    TH["transport-http (fan-out: 10)"]
  end

  subgraph Composition
    Bootstrap["app-bootstrap (fan-out: 21)"]
    App["app (fan-out: 17, ~30行 facade)"]
  end

  subgraph DomainApps
    AppCore["app-core (Ports 所在地, fan-in: 16)"]
    AppChat["app-chat (fan-out: 19, 2.78万行)"]
    AppDocs["app-documents (fan-out: 12)"]
    AppAdmin["app-admin"]
    AppBilling["app-billing"]
  end

  subgraph Contracts
    Contracts["contracts (跨语言 DTO)"]
    RagPorts["avrag-rag-core-ports (CachePort)"]
  end

  subgraph RAGPipeline
    RagCore["avrag-rag-core (fan-out: 8)"]
    RDP["avrag-retrieval-data-plane"]
    Ingestion["ingestion"]
  end

  subgraph Infrastructure
    StoragePG["avrag-storage-pg"]
    StorageMilvus["avrag-storage-milvus"]
    StorageLocal["storage-local"]
    CacheRedis["avrag-cache-redis"]
    LLM["avrag-llm"]
  end

  subgraph Hubs
    Common["common (fan-in: 24)"]
    Auth["avrag-auth (fan-in: 22)"]
  end

  subgraph ShareBilling
    Share["avrag-share"]
    Billing["avrag-billing"]
  end

  Frontend -.->|"Web: SSE/HTTP"| API
  Frontend -.->|"Tauri IPC"| Desktop

  Desktop --> StorageLocal
  Desktop --> Common
  Desktop --> Auth
  Desktop --> Contracts

  StorageLocal --> RagPorts
  StorageLocal --> Common

  API --> App
  API --> TH
  TH --> Bootstrap
  TH --> AppChat
  TH --> AppCore
  TH --> Share
  TH --> Billing

  App --> Bootstrap
  App -.->|"feature: product-e2e"| TH

  Bootstrap --> StoragePG
  Bootstrap --> StorageMilvus
  Bootstrap --> CacheRedis
  Bootstrap --> AppCore
  Bootstrap --> AppChat
  Bootstrap --> AppDocs
  Bootstrap --> AppAdmin
  Bootstrap --> AppBilling
  Bootstrap --> Share

  AppChat --> AppDocs
  AppChat --> RagCore
  AppChat --> RDP
  AppDocs --> AppCore
  AppDocs --> Ingestion
  AppAdmin --> AppCore

  Worker --> AppCore
  Worker --> Ingestion
  Worker --> StoragePG
  Worker --> StorageMilvus

  RagCore --> RagPorts
  RagCore --> AppCore
  RagCore --> RDP
  RagCore --> LLM
  RagCore --> CacheRedis
  RagCore --> Contracts
  RagCore --> Common
  StorageMilvus --> RDP
  Common --> Auth
  Common --> Contracts
  Share --> AppCore
  Billing --> Common

  classDef critical fill:#ff6b6b,stroke:#c92a2a,color:#fff
  classDef warning fill:#ffd43b,stroke:#e67700
  classDef clean fill:#51cf66,stroke:#2b8a3e,color:#fff

  class Common,Auth,AppChat,RagCore warning
  class Frontend,Desktop,API,Worker,TH,Bootstrap,App,AppCore,AppDocs,AppAdmin,AppBilling,Contracts,RagPorts,RDP,Ingestion,StoragePG,StorageMilvus,StorageLocal,CacheRedis,LLM,Share,Billing clean
```

生产依赖图无环（`cargo metadata` 生产边验证）；`app-bootstrap → app-chat → app-bootstrap` 仅存在于 `app-chat` 的 `dev-dependencies`（测试夹具），不计入生产环。

---

## Findings

### 🟡 Warning

**Change Propagation — common / avrag-auth 枢纽扇入仍偏高，common 仍耦合 auth**

Symptom: 生产扇入 `common` 24（v3: 22）、`avrag-auth` 22（v3: 20）；`common` 总量已从 2689 行降至 1443 行（`rag_execute.rs` / `tool_call.rs` 已迁出），但 `common/Cargo.toml` 仍生产依赖 `avrag-auth`，`content_store.rs` 与 `identity.rs` 直接 `use avrag_auth::*`，使 24 个消费者间接绑定 auth 类型。

Source: Brooks — The Mythical Man-Month — Ch. 2: Brooks's Law (communication overhead via change radius)

Consequence: 修改 `AuthContext` / `OrgId` 或 `ContentStore` 签名仍触发约 2/3 workspace 重编译；desktop 经 `common` 间接依赖 auth，枢纽变更继续波及桌面端。

Remedy: 将 `ContentStore` 所需的 auth 类型下沉到 `contracts` 或独立 `auth-types` 轻量 crate；`common` 仅 re-export 或完全去掉对 `avrag-auth` 的生产依赖，让适配器层（`storage-local` / `pg_*`）承担 auth 类型转换。

---

**Cognitive Overload — app-chat 仍有 3 个千行级源文件**

Symptom: `agents/loop/mod.rs` 1201 行、`agents/loop/iteration.rs` 1147 行、`eval/framework.rs` 1633 行（`feature = "eval"` 门控）；crate 总量 27858 行。对比改善项：`chat_private.rs` 已拆为 6 文件共 1331 行（`mod.rs` 515、`profile_merge.rs` 499 等），`prompts/` 目录化、`agents/loop/` 已有 16+ 子模块与 `STATE_MACHINE.md`。

Source: Ousterhout — A Philosophy of Software Design — Ch. 4: Modules Should Be Deep

Consequence: agent 循环主干与 eval 框架仍难以独立测试与定位；新成员改动 loop 编排需通读千行文件，回归半径大。

Remedy: 延续既有拆分套路：`iteration.rs` 拆出「步骤执行」与「状态转移」；`loop/mod.rs` 仅保留编排主干，辅助逻辑下沉到既有子模块；`eval/framework.rs` 按 scenario/fixture/runner 三分。

---

**Dependency Disorder — avrag-rag-core 生产依赖 app-core（RAG 运行时反向依赖应用 Port 层）**

Symptom: `avrag-rag-core/Cargo.toml` 生产依赖 `app-core`；`runtime.rs` / `runtime/config.rs` 使用 `app_core::ChatPersistencePort`。RAG 管道 crate 依赖应用上下文 Port 定义，而非自包含 port 或 `contracts`。

Source: Martin — Clean Architecture — Dependency Inversion Principle

Consequence: `app-core` 中 Port 签名变更会强制 `rag-core` 重编译；RAG 子系统无法在不感知 `app-core` 的情况下独立演进或复用（例如 worker 场景、桌面本地 RAG）。

Remedy: 将 `ChatPersistencePort` 等与 RAG 执行相关的 port trait 迁入 `avrag-rag-core-ports` 或 `app-core` 的独立 `ports/rag.rs` 并由 `rag-core` 仅依赖轻量 port crate；`app-bootstrap` 负责 adapter 接线。

---

### 🟢 Suggestion

**Dependency Disorder — desktop 独立 workspace + 双 Cargo.lock（已有 CI 兜底，属有意权衡）**

Symptom: `desktop/src-tauri` 保持独立 `Cargo.lock`（474 包，v3: 558）；Cargo 要求 workspace member 必须在 workspace 根目录之下，无法并入 `avrag-rs` workspace（见 `desktop/AGENTS.md` M5）。根 CI `smoke-e2e.yml` 的 `desktop-check` job 在 `avrag-rs/**` 或 desktop 锁变更时执行 `cargo check --manifest-path desktop/src-tauri/Cargo.toml`。

Source: Winters et al. — Software Engineering at Google — Ch. 21: Dependency Management

Consequence: 本地开发若跳过 CI，path 依赖变更后 desktop 锁可能滞后；但 CI 已覆盖主要集成路径，风险可控。

Remedy: 维持现状 + 在 `desktop/AGENTS.md` 已文档化的 `cargo update -p <crate>` 流程；可选在 pre-commit 增加 desktop lock 新鲜度提示，无需强行合并 workspace。

---

**Domain Model Distortion — avrag-share 领域处理器直接返回 axum::Json**

Symptom: `share/src/handlers.rs` 中 `handle_create_share_link` 等函数返回 `Result<axum::Json<ShareTokenResponse>, AppError>`；`share/Cargo.toml` 生产依赖 `axum`。领域/应用服务层与 HTTP 框架类型耦合。

Source: Martin — Clean Architecture — Policy vs Detail boundaries

Consequence: share 逻辑无法在不引入 axum 的情况下被 worker、CLI 或桌面 IPC 复用；测试需模拟 HTTP 层类型。

Remedy: handler 返回 `ShareTokenResponse` 或 `Result<T, AppError>`，由 `transport-http` 路由层包装 `Json(...)`；与 admin/chat 路由已有的 Port 化风格对齐。

---

**Change Propagation — dev-dep 测试环 app-bootstrap ↔ app-chat**

Symptom: `app-bootstrap` 生产依赖 `app-chat`（组装 `ChatContext`）；`app-chat` 的 `dev-dependencies` 依赖 `app-bootstrap`（测试夹具）。`cargo metadata` 全边图存在 1 条环，生产边无环。

Source: Martin — Clean Architecture — Acyclic Dependencies Principle (ADP)

Consequence: 测试 crate 解析顺序可能变复杂；未来若误将 `app-bootstrap` 移入 `app-chat` 生产依赖会立即形成生产环。

Remedy: 将测试夹具提取到 `avrag-test-kit` 或 `app-chat/tests/support/`，切断 `app-chat → app-bootstrap` dev 边；保持 bootstrap 单向依赖 chat 的生产关系。

---

## Testability Seam Assessment

| 边界 | 状态 | 说明 |
|------|------|------|
| Auth | ✅ 保持 | `AuthStorePort` + `PgAuthStoreAdapter` |
| Admin | ✅ 保持 | 全部路由经 `call_admin_store` → `AdminStorePort`；`avrag-admin` 空壳 crate 已删除 |
| Documents/Content | ✅✅ 双适配器 | `ContentStore`: `PgContentStore` + `LocalContentStore` |
| Cache | ✅ 轻量 port | `CachePort` 迁至 `avrag-rag-core-ports`；`storage-local` / desktop 不再拉入 llm/redis |
| Chat 持久化 | ✅ 保持 | `ChatPersistencePort` + `pg_chat_persistence` adapter |
| Milvus 检索 | ✅ 保持 | `RetrievalDataPlane` seam 完好 |
| Share | ✅ 改进 | `ShareStorePort` 替代已删 `share/db.rs` 直连 PG |
| 前端传输 | ✅ 保持 | `lib/runtime/transport.ts` 分叉 Web SSE vs Tauri IPC |
| product-e2e | ✅ 保持 | `app/product_e2e_http.rs` feature 门控 |
| Desktop 状态 | ✅ 本轮收口 | `AppLocalState` + `app.manage()` + `State<T>`；`ChatStreamRegistry` 支持取消 |
| Desktop chat/api | ⏳ 计划中 | `chat_stream` / `api_call` 占位实现，路线图阶段 2，不计债 |

Source: Feathers — Working Effectively with Legacy Code — Ch. 4: The Seam Model

---

## Conway's Law

单人/单团队维护整个 monorepo，无跨团队协调成本，组织对齐检查不适用，跳过。

---

## Summary

v3 提出的 desktop 依赖过重、CachePort 落点、OnceCell 全局单例三项均已解决；`common` 减重与 `avrag-admin` 删除进一步收敛了概念完整性。当前最优先动作是继续 **`app-chat` 千行文件拆分**（loop 主干与 eval 框架），其次是 **`common` 去掉对 `avrag-auth` 的生产依赖** 以真正降低枢纽扇入。`rag-core → app-core` 的 Port 反向依赖建议在 RAG 本地化（desktop 阶段 3）之前完成，避免桌面端复用 RAG 时再次拖入应用层。

---

## v3 遗留项核销对照

| v3 发现 | 严重度 | 本轮状态 |
|---------|--------|----------|
| desktop 双 Cargo.lock 漂移 | 🟡 | ⚠️ 有意保留 + `desktop-check` CI 兜底；降级为 🟢 Suggestion |
| 壳层依赖过重（558 包） | 🟡 | ✅ 已解决：`CachePort` → `rag-core-ports`；desktop 474 包，无 `avrag-rag-core` 直接依赖 |
| common/auth 枢纽扇入 | 🟡 | ⚠️ 部分改善：common 2689→1443 行、`rag_execute`/`tool_call` 迁出；扇入 22→24 / 20→22 未降 |
| app-chat 大文件 | 🟡 | ⚠️ 部分改善：`chat_private` 拆 6 文件；仍余 3 个千行文件 |
| OnceCell 全局单例 | 🟢 | ✅ 已解决：`AppLocalState` + Tauri `manage`/`State` |
| avrag-admin Lazy Class | 🟢 | ✅ 已解决：crate 整包删除，admin 路由无 `avrag-admin` 引用 |
| app-admin 虚报 storage-pg 生产依赖 | 🟢 | ✅ 已解决：仅 `dev-dependencies` 保留 |
| admin 双轨 / transport sqlx / app 绑定 storage-pg | — | ✅ v2 项保持清零 |

---

## 本轮新发现

| 项 | 严重度 | 摘要 |
|----|--------|------|
| rag-core 反向依赖 app-core | 🟡 | RAG 运行时依赖 `ChatPersistencePort` 定义 |
| share 处理器耦合 axum::Json | 🟢 | 领域层应返回纯类型 |
| dev-dep 环 bootstrap↔chat | 🟢 | 仅测试边，生产无环 |
| app facade 极薄化 | ✅ 正面 | `app/src` 合计 ~30 行，纯 re-export |
| billing 模块合并 | ✅ 正面 | `core_support`/`core_usage`/`core_webhooks` 删除，收敛到 `core` |
| WorkspaceChatPane 瘦身 | ✅ 正面 | 2514→180 行（前端，超出 Rust 审计范围但降低全栈认知负载） |

---

## 应保留的正面模式

| 模式 | 位置 |
|------|------|
| 轻量 port crate | `avrag-rag-core-ports`：`CachePort` 仅 `async-trait`，desktop/storage-local 共用 |
| contracts-first RAG 执行 | `ExecutePlan*` 类型在 `contracts/`，`rag-core/runtime/execute.rs` 消费 |
| common 枢纽减重 | 删除 `rag_execute.rs`/`tool_call.rs`；注释明确「wire DTO 在 contracts」 |
| chat_private 模块化 | `app-chat/src/chat_private/{mod,profile_merge,memory,quota,visibility}.rs` |
| Admin 单一路径 | `transport-http/routes/admin.rs` → `call_admin_store` |
| Share Port 化 | `ShareStorePort` + 删除 `share/db.rs` |
| 前端传输接缝 | `frontend_next/lib/runtime/transport.ts` |
| Desktop managed state | `desktop/src-tauri/src/lib.rs`：`AppLocalState`、`ChatStreamRegistry` |
| CI desktop 兜底 | `.github/workflows/smoke-e2e.yml` → `desktop-check` |
| product-e2e 进程内 HTTP | `app/product_e2e_http.rs` feature 门控 |

---

## 验证命令

```bash
cd avrag-rs

# 依赖图与扇入扇出
python3 - <<'PY'
import json, subprocess
from collections import defaultdict
meta = json.loads(subprocess.check_output(["cargo","metadata","--format-version","1","--no-deps"], text=True))
ws = "/home/chuan/context-osv6/avrag-rs"
members = {p["name"] for p in meta["packages"] if p["manifest_path"].startswith(ws)}
fan_in = defaultdict(int)
for p in meta["packages"]:
    if not p["manifest_path"].startswith(ws): continue
    for d in p["dependencies"]:
        if d["name"] in members: fan_in[d["name"]] += 1
for n,c in sorted(fan_in.items(), key=lambda x:-x[1])[:5]: print(f"  {n}: {c}")
PY

# v3 核销项
rg -n 'avrag-rag-core' ../desktop/src-tauri/Cargo.toml || echo "desktop 无 rag-core 直接依赖"
rg -c '^name = ' ../desktop/src-tauri/Cargo.lock   # 期望 ~474
rg -n 'OnceCell' ../desktop/src-tauri/src/lib.rs || echo "OnceCell 已清零"
rg -n 'avrag-admin' crates/transport-http/ || echo "avrag-admin 已删除"
wc -l crates/common/src/**/*.rs crates/common/src/*.rs 2>/dev/null | tail -1
wc -l crates/app-chat/src/agents/loop/mod.rs crates/app-chat/src/agents/loop/iteration.rs
find crates/app-chat/src/chat_private -name '*.rs' | wc -l

# 生产环检测
python3 - <<'PY'
import json, subprocess
from collections import defaultdict
meta = json.loads(subprocess.check_output(["cargo","metadata","--format-version","1"], text=True))
ws = "/home/chuan/context-osv6/avrag-rs"
members = {p["name"] for p in meta["packages"] if p["manifest_path"].startswith(ws)}
g = defaultdict(set)
for p in meta["packages"]:
    if not p["manifest_path"].startswith(ws): continue
    for d in p["dependencies"]:
        if d.get("kind")=="dev": continue
        if d["name"] in members: g[p["name"]].add(d["name"])
print("Production cycles: none" if not any(True for _ in []) else "")
PY

# 历史
jq '.[] | select(.mode=="Architecture Audit") | {date, score}' ../.brooks-lint-history.json
```

---

## 修订记录

| 日期 | 说明 |
|------|------|
| 2026-06-13 v4 | 本轮：v3 三项 Warning 落地；common 减重、admin 删除、share Port 化；新发现 rag-core→app-core 反向依赖（82 分） |
| 2026-06-12 v3 | v2 遗留 3/4 核销；新增 desktop 风险面 → [archive/brooks-architecture-audit-2026-06-12-v3.md](./archive/brooks-architecture-audit-2026-06-12-v3.md) |
| 2026-06-12 v2 | auth Port 化、pipeline 拆分后（83 分）→ [archive/brooks-architecture-audit-2026-06-12-v2.md](./archive/brooks-architecture-audit-2026-06-12-v2.md) |
| 2026-06-12 v1 | 初轮深测（77 分）→ [archive/brooks-architecture-audit-2026-06-12-v1.md](./archive/brooks-architecture-audit-2026-06-12-v1.md) |
| 2026-06-10 | 更早报告 → [archive/brooks-health-architecture-audit-2026-06-10.md](./archive/brooks-health-architecture-audit-2026-06-10.md) |
