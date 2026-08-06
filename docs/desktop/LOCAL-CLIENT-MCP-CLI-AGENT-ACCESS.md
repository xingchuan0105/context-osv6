# 本机客户端 × Coding Agent（MCP / CLI）能力与边界

**状态：** 现行参考（living）  
**日期：** 2026-08-06  
**范围：** 用户安装 **Context-OS 桌面客户端** 后，用 Claude Code / Codex / Cursor 等 coding agent，以 **MCP 或 CLI** 调用本机能力。

**相关：**

| 文档 | 关系 |
|------|------|
| `frontend_next/public/docs/api-access-for-agents.md` | **云端/通用** Agent HTTP·MCP 协议与工具表（权威 wire 契约） |
| `desktop/runtime/README.md` · `docs/desktop/2026-08-04-portable-runtime-design.md` | 本机 API/worker/PG·Redis 栈 |
| `docs/adr/0010-share-service-business-model.md` | 分享名额 / 档位 / Owner-pays |
| `docs/agent/product-apps.md` | 执行只走 Product Apps；MCP 为 thin transport |

---

## 1. 产品意图（目标）

用户下载并运行本地客户端后，coding agent 应能（理想态）：

1. **创建 Workspace**
2. **传入并解析/索引文档**（文件上传、URL 源）
3. **问答检索**（知识库 RAG；可选联网）
4. **分享**（开启分享、配置访客/限次等），且 **受会员档位额度** 约束

协议形态：

- **MCP**（优先，外部 agent 统一入口）
- **CLI**（可选，薄封装同一后端）

**非目标（刻意）：**

- 不让 Agent 走 Tauri `invoke` / 桌面私有 IPC 作为自动化主路径  
- 不在桌面壳内再实现一套第二套检索/对话协议  
- 不以 workspace API key 绕过「用户会话 only」的分享/密钥管理（除非另开产品决策）

---

## 2. 架构结论（一句话）

| 问题 | 结论 |
|------|------|
| 本机有没有可调的「客户端能力」？ | **有。** 桌面 runtime 起 **本机 `avrag-api`**（默认 `http://127.0.0.1:18080`）+ worker；与云端 **同一套** transport / Product Apps / MCP。 |
| Agent 今天能否调？ | **上传/问答：能**（workspace API key 或 user token）。**建库/分享：能**（`CONTEXT_OS_USER_TOKEN`；API key 仍关）。 |
| 装完是否「开箱被 Claude 发现」？ | **半开。** 提供 `context-os-mcp` + 配置片段 + UI 引导；**不会**自动写入 Claude 全局配置。 |

```text
  Claude Code / Codex / Cursor
           │
           │  ① 推荐：stdio → context-os-mcp
           │  ② 备选：HTTP MCP → 本机 API
           ▼
  http://127.0.0.1:18080/api/v1/mcp
  Authorization: Bearer <workspace_api_key 或 用户 JWT>
           │
           ▼
  桌面 runtime：avrag-api + avrag-worker + PG/Redis（本地数据）
           │
     ┌─────┴──────┬────────────┬────────────┐
     ▼            ▼            ▼            ▼
  建 Workspace  上传·解析    问答检索     分享 / 档位
  用户 JWT 开   已开         已开        用户 JWT 开 / API key 关
```

**原则：** 自动化面 = **HTTP / MCP**；桌面只负责 **起栈、登录/激活、UI、授权与额度真相**。

---

## 3. 本机运行时前提

| 项 | 现状（实现参考） |
|----|------------------|
| API 监听 | 默认 `AVRAG_API_ADDR=127.0.0.1:18080`（`desktop/src-tauri` native stack / `client.env`） |
| 公开基址 | `AVRAG_PUBLIC_BASE_URL=http://127.0.0.1:18080` |
| 进程 | sidecar：`avrag-api`、`avrag-worker`；状态目录见 portable runtime 设计 |
| MCP 入口 | `POST /api/v1/mcp`（JSON-RPC：`initialize` / `tools/list` / `tools/call`）；`GET /api/v1/mcp` SSE ready |
| 实现位置 | `avrag-rs/crates/transport-http/src/mcp/`（catalog / dispatch / gateway / tools） |

桌面 **不是** 另一套业务后端；是 **把同一 API 跑在本机**。

---

## 4. 能力矩阵（对照目标四条）

### 4.1 创建 Workspace

| 维度 | 状态 |
|------|------|
| MCP 工具 | `account.create_workspace`、`account.list_workspaces` **在目录与 dispatch 中存在** |
| 产品叙事 | `api-access-for-agents.md`：**个人用户在 UI 建库**；Agent 主路径用 **workspace API key**，不要依赖「账号级自动化」作为个人默认 |
| 鉴权 | 账号级工具要 **用户会话 / 账号向凭据**；**workspace key 不能**调 account 工具（`workspace_key_cannot_call_org_tools` 等） |
| 建 API key | `POST /api/v1/workspaces/{id}/api-keys` 需 **用户会话**；API key **不能**自管 key（`api_key_forbidden`） |

**今日合理路径：**

1. 人在客户端 UI（或已登录会话 REST）创建 Workspace  
2. 工作区 **API Access** 创建 key（默认权限常含 `index` + `query`）  
3. 将 `workspace_id` + key 交给 coding agent  

**缺口（P1）：** 桌面签发 **local user token**（或等价），使 MCP 在用户已登录前提下可安全 `create_workspace`，而无需长期暴露「裸账号密码」。

### 4.2 传入并解析文档 — **已开**

| MCP 工具 | 权限 | 作用 |
|----------|------|------|
| `workspace.create_upload` | `index` | 开始上传，返回 `upload_url` |
| （HTTP PUT） | — | 写对象字节（非 MCP body） |
| `workspace.complete_upload` | `index` | 完成上传，进入解析/索引 |
| `workspace.document_status` | `index` 或 `query` | 轮询至 completed / failed |
| `workspace.add_url_source` | `index` | URL 源 |
| `workspace.list_sources` | `query` | 列表 |

依赖本机 **worker** 跑摄取流水线；与云端同源能力。

### 4.3 问答检索 — **已开**

| MCP 工具 | 权限 | 作用 |
|----------|------|------|
| `workspace.rag_query` | `query` | 知识库 RAG |
| `workspace.chat` | `query` | 遗留别名，偏好 rag/search/chat |
| `workspace.search_query` | `query` | 联网检索（本机需配置对应能力） |

执行路径：`state.conversation().execute` / stream（Product Apps），**非** transport 旁路。

### 4.4 分享（含会员档位额度）— **用户 JWT 开；API key 关**

| 维度 | 状态 |
|------|------|
| 产品 UI / 用户 REST | 有分享中心、`share_enabled`、限次、档位可分享名额等（ADR-0010） |
| MCP 工具 | **有**（用户 JWT）：`workspace.share_create_link` / `share_get_settings` / `share_update_settings` / `share_revoke_link`；`account.share_quota` |
| API key | share / members / notes / notifications / **API key 管理** → 用户会话 only → **`403 api_key_forbidden`** |
| 额度 | 配额真相在 **Owner 用户 + 订阅档位**；`ShareService::ensure_share_enabled`；不能让 workspace key 绕过 |

**结论：** coding agent 持 **`CONTEXT_OS_USER_TOKEN`**（短时 agent token）可开分享并受档位约束；**禁止**用 workspace API key。

---

## 5. 鉴权模型（Agent 必读）

| 调用方 | 凭据 | 典型能力 |
|--------|------|----------|
| 人 · 产品 UI | 用户 JWT（会话） | 全量：建库、key 管理、分享、设置… |
| Coding agent · 索引/查询 | **Workspace API key** | `workspace.*` 上传/RAG；**不能** share / 管 key / account.* |
| Coding agent · 建库/分享 | **User JWT / agent token**（`CONTEXT_OS_USER_TOKEN`） | `account.*`、`workspace.share_*`；受 ADR-0010 名额约束 |
| 平台级自动化（非个人默认） | 账号向凭据（若启用） | account 类工具；文档不鼓励个人依赖 |

Wire 与错误码细节见：`frontend_next/public/docs/api-access-for-agents.md`。

---

## 6. 今日可用操作规程

适合：高级用户 / 内测 / 工程师验证。

1. 启动 **Context-OS Client**，确认本机栈就绪（API `127.0.0.1:18080`）。  
2. UI：登录/激活（若产品要求）→ **新建 Workspace**。  
3. 该工作区 **API Access** → 创建 key（建议 `index` + `query`）；复制 **workspace_id** 与配置片段。  
4. **优先：stdio MCP**（`context-os-mcp`）——见 §6.1。  
   或直接打 **HTTP MCP**：

```http
POST http://127.0.0.1:18080/api/v1/mcp
Authorization: Bearer <workspace_api_key>
Content-Type: application/json
```

5. 工具顺序建议：`create_upload` → PUT → `complete_upload` → 轮询 `document_status` → `rag_query`。  
6. **分享** 仍在客户端 UI 完成（受档位限制）。

### 6.1 stdio MCP 包装（`context-os-mcp`）

| 项 | 值 |
|----|-----|
| 源码 | `avrag-rs/bins/client`（package `context-os`，bins：`context-os-mcp` + `context-os`） |
| 构建 | `cargo build -p context-os --release`（在 `avrag-rs/`） |
| Stage | `bash scripts/stage-desktop-sidecars.sh` → `desktop/runtime/bin/context-os-mcp`（+ `context-os` CLI） |
| 默认 base | `http://127.0.0.1:18080` |
| Env | `CONTEXT_OS_API_KEY`（或 `CONTEXT_OS_WORKSPACE_API_KEY`）；可选 `CONTEXT_OS_API_BASE` / `AVRAG_PUBLIC_BASE_URL` |
| 探活 | `context-os-mcp --check`（exit 0 就绪；1 不可达/鉴权失败；3 缺 key） |

**Claude Code**（`~/.claude.json` 或项目 `.mcp.json` 形态，按本机 Claude 版本调整键名）：

```json
{
  "mcpServers": {
    "context-os": {
      "command": "/path/to/context-os-mcp",
      "env": {
        "CONTEXT_OS_API_BASE": "http://127.0.0.1:18080",
        "CONTEXT_OS_API_KEY": "<workspace_api_key>"
      }
    }
  }
}
```

**Cursor**（Settings → MCP，或 `.cursor/mcp.json`）：

```json
{
  "mcpServers": {
    "context-os": {
      "command": "/path/to/context-os-mcp",
      "env": {
        "CONTEXT_OS_API_BASE": "http://127.0.0.1:18080",
        "CONTEXT_OS_API_KEY": "<workspace_api_key>"
      }
    }
  }
}
```

**Codex / 其它支持 stdio MCP 的客户端：** 同一 `command` + `env`；工具调用时在 `arguments` 里带上 **`workspace_id`**（与签发 key 的工作区一致）。

**可读错误（设计）：**

| 情况 | 表现 |
|------|------|
| 客户端/API 未起 | stderr + JSON-RPC `-32000`：提示启动 Context-OS Client / 检查 `CONTEXT_OS_API_BASE` |
| 未设 key | 启动 warning；`--check` exit 3 |
| key 无效 / 401 | 提示到 UI **API Access** 重建 key |

---

## 7. 交付缺口与分档（P0 / P1）

### P0 — 「本机知识库 Agent」（基建已齐，补包装）

**目标：** Claude Code / Codex 稳定调用 **上传 + 解析 + 问答**。

| 工作项 | 状态 | 说明 |
|--------|------|------|
| stdio MCP 包装 | **已落地** | `context-os-mcp`（`avrag-rs/bins/mcp`）：stdio ↔ `POST {base}/api/v1/mcp`；stage 到 `desktop/runtime/bin/` |
| 配置片段 | **已落地** | 见下文 §6.1（Claude Code / Codex / Cursor） |
| 本机发现与错误 | **已落地** | `context-os-mcp --check`；连接失败 / 缺 key / 401 可读文案（stderr + JSON-RPC） |
| 文档与 UI 引导 | **已落地** | Workspace **API Access**「给 Agent 用」卡片 + 本页 / wire 文档 |

**范围外：** 分享 MCP、纯 workspace key 全自动建库。

### P1 — 「全能力 Agent」（需产品决策）

**目标：** 建库 + 分享也走 Agent，且额度不可绕过。

| 工作项 | 状态 | 说明 |
|--------|------|------|
| Local user token | **已落地** | `POST /api/auth/agent-token` 签发短时用户 JWT；`CONTEXT_OS_USER_TOKEN`；UI「签发 2h token」；吊销靠短 TTL + `auth_version`（改密） |
| `create_workspace` 个人路径 | **已落地（CLI/MCP）** | `context-os workspace create` / MCP `account.create_workspace` + 用户 JWT（非 workspace key） |
| `workspace.share_*`（用户态 MCP） | **已落地** | `share_create_link` / `get_settings` / `update_settings` / `revoke_link` + `account.share_quota`；API key → `api_key_forbidden`；配额走 ShareService（ADR-0010） |
| CLI（workspace key 路径） | **已落地** | `status|ingest|ask|sources`；share 子命令需 `CONTEXT_OS_USER_TOKEN` |

**禁止：** 用 workspace API key 实现分享以绕过用户会话与档位。

---

## 8. CLI 定位

| 项 | 说明 |
|----|------|
| 现状 | **`context-os`** 薄 CLI（`avrag-rs/bins/client`）+ 服务进程 `avrag-api` / `avrag-worker` |
| 原则 | CLI 调本机（或云端）HTTP/MCP；**不** 嵌入 Tauri |
| 与 MCP | 同一后端契约与 workspace API key；CLI 服务 shell/脚本，MCP 服务 agent 工具循环 |

### 8.1 `context-os` 子命令

```bash
# optional: CONTEXT_OS_API_BASE=http://127.0.0.1:18080

# 用户态（建库 / 分享）— 推荐 mint/from-desktop --save（默认不打印完整 JWT）
context-os auth from-desktop --save     # 读桌面 local_session.json → ~/.config/context-os/user.token
# 或: context-os auth login --email … --password … --save
# 或: context-os auth mint --ttl 120 --save
# 桌面会话自动加载默认关闭；需要时: CONTEXT_OS_LOAD_DESKTOP_SESSION=1
context-os auth path                    # 查看 token 文件与桌面候选路径
context-os workspace create --name Research
context-os workspace list

# 工作区自动化（API key 或 user token 均可）
export CONTEXT_OS_API_KEY=...          # 或继续用 USER_TOKEN
export CONTEXT_OS_WORKSPACE_ID=...
context-os status
context-os ingest ./doc.pdf            # create_upload → PUT → complete → poll
context-os ask "Summarize the indexed docs"
context-os sources

# 分享（用户 JWT；消耗档位 share 名额）
context-os share quota
context-os share enable --workspace $WS --role viewer
context-os share configure --workspace $WS --access-level link --anon-limit 10
context-os share status --workspace $WS
context-os share invite --workspace $WS --email peer@example.com --role viewer
context-os share revoke --workspace $WS --token <share_token>
```

**鉴权优先级：**  
- **stdio MCP / ingest / ask：** least-privilege `bearer_token()` — 显式 user env ＞ API key ＞ 仅 auto user JWT  
  - 若 API key + **自动发现**的 `user.token` → 优先 API key（避免静默提权）  
- **CLI share / workspace create|list：** 固定 `tools_call_as_user`（始终带 user JWT），双凭据下不误用 API key  
- 桌面会话自动加载默认 **关**（`CONTEXT_OS_LOAD_DESKTOP_SESSION=1` 开启）  

**Agent mint：** JWT `token_kind=agent`（与响应字段一致），TTL ≤ 父会话 `exp`，不可 remint。  
**Token 文件：** `~/.config/context-os/user.token`（文件 0600；仅自有 `context-os` 目录 0700）。

构建：`cargo build -p context-os --release` → `target/release/context-os` 与 `context-os-mcp`。  
Stage：`bash scripts/stage-desktop-sidecars.sh` → `desktop/runtime/bin/context-os`。

### 8.2 Agent user token（API）

| 项 | 值 |
|----|-----|
| 路径 | `POST /api/auth/agent-token`（需用户 JWT；API key → `403 api_key_forbidden`） |
| Body | `{ "ttl_minutes": 120 }`（5–1440，默认 120） |
| 返回 | `{ success, data: { token, expires_at, ttl_minutes, token_kind: "agent" } }` |
| 吊销 | 短 TTL；改密等抬高 `auth_version` 使旧 JWT 失效 |

---

## 9. 明确不支持 / 勿依赖

1. **API key 调分享、成员、笔记、通知、API key CRUD** → `api_key_forbidden`。  
2. **Workspace key 调 `account.*`** → 失败；先 UI 建库。  
3. **Agent 经 Tauri invoke 作为正式自动化面** → 非设计路径。  
4. **stdio MCP 零配置自动发现** → 无；需用户配置 command/env（二进制可由 stage/安装目录提供）。  
5. **「本机 MCP」与「云端 MCP」两套工具语义** → 应保持 **同一 catalog 与权限模型**；仅 base URL 不同。

---

## 10. 实现索引（代码）

| 区域 | 路径 |
|------|------|
| MCP 路由 | `avrag-rs/crates/transport-http/src/routes/chat.rs`（`/api/v1/mcp`） |
| MCP 实现 | `avrag-rs/crates/transport-http/src/mcp/` |
| 工具 catalog | `…/mcp/catalog.rs` |
| 鉴权 | `…/auth_guard.rs`（workspace tool / account tool / api_key_forbidden） |
| stdio 包装 + CLI | `avrag-rs/bins/client` → `context-os-mcp` / `context-os` |
| Stage 脚本 | `scripts/stage-desktop-sidecars.sh`（runtime/bin） |
| Agent 文档（wire） | `frontend_next/public/docs/api-access-for-agents.md` |
| 本机 API 地址 | `desktop/src-tauri/src/commands/native_stack.rs`（`client.env` / `18080`） |
| 桌面壳 | `desktop/`（Tauri 2 + `frontend_next` out） |
| UI 引导 | `frontend_next/components/api-access/workspace-api-access-surface.tsx` |

---

## 11. 变更记录

| 日期 | 说明 |
|------|------|
| 2026-08-06 | 初版：对齐「本机客户端被 coding agent 以 MCP/CLI 调用」意图；矩阵 + P0/P1 + 非目标 |
| 2026-08-06 | P0：`context-os-mcp` stdio 包装、`--check`、配置片段、API Access UI 引导 |
| 2026-08-06 | P1 切片：`context-os` CLI（status/ingest/ask/sources；share 拒绝）；client 包合并 mcp+cli |
| 2026-08-06 | P1：`POST /api/auth/agent-token` + `CONTEXT_OS_USER_TOKEN`；CLI auth/workspace create；API Access 签发 UI |
| 2026-08-06 | P1：MCP `workspace.share_*` + `account.share_quota`；CLI `share enable|status|configure|revoke|quota` |
| 2026-08-06 | UX：token 文件 / `auth from-desktop` / mint|login `--save`；`share_invite_member` |
