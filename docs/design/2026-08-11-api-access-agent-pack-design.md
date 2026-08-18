# API Access「一次复制即可接入」调整设计

**日期**: 2026-08-11  
**状态**: Accepted — §12 决策：1 disabled 先建 key；2 创建后自动复制；3 规范路径 `/help/api-access/agents`  
**范围**: 工作区 API Access 面板（分享弹窗内 `WorkspaceApiAccessSurface`）+ agent 文档可达性  
**非目标**: 改 MCP 协议本身、改 API key 权限模型、OAuth MCP 授权流  

---

## 1. 问题（对照截图 + 现网）

截图场景：分享工作区 →「给 LLM Agents」卡片。

| 现象 | 证据 |
|------|------|
| **说明文档断链** | 现网 `GET https://app.contextlm.top/docs/api-access-for-agents.md` → **404**；VPS standalone 包内 `public/docs/` 未随前端正确落地 |
| **链接呈现像断链** | UI 用相对路径原文 `/help/api-access`、`/docs/...md` 作主链接，橙色裸路径像未解析 |
| **逻辑顺序反** | 文案写「先读说明页 → 再读 agent 文档」，但人类真实路径是：**建 key → 复制连接信息 → 贴进 agent**；文档应是补强，不是门槛 |
| **复制碎片化** | `workspace_id` / API base / HTTP MCP / MCP JSON / key / agent token 各点一次；外接 agent 需要人肉拼装 |
| **MCP 配置缺真值** | 当前 snippet 固定 `CONTEXT_OS_API_KEY: "<paste_workspace_api_key>"`，且**不含** `workspace_id`、文档 URL |

用户目标（产品原话）：

> **一次复制给外接 agent，就能连接上**（包括 KEY、MCP URL、说明链接等）。

---

## 2. 行业实践摘要（用于约束设计）

| 来源 | 可复用原则 |
|------|------------|
| [Firecrawl MCP Connect](https://docs.firecrawl.dev/mcp-server/connect) | **一个主服务器 URL**；按场景分「立刻试 / 账号 / API key」；强调 MCP URL **不是浏览器打开页**；密钥放 env，不进对话与 git |
| [Stripe API keys](https://docs.stripe.com/keys) / [best practices](https://docs.stripe.com/keys-best-practices) | 密钥 **只展示一次**；立刻复制/入库；Dashboard 里「连接单元」清晰，不散落 |
| [Microsoft Foundry MCP](https://learn.microsoft.com/en-us/azure/foundry/agents/how-to/tools/model-context-protocol) | 连接 = **endpoint + 凭据 + 引用配置** 一组，而不是分散字段 |
| MCP 生态常识 | stdio 用 `command`+`env`；远程用 **Bearer + MCP endpoint**；配置块必须可直接贴进 Cursor / Claude Code |

**结论**：外接 agent 的交付物应是 **一份完整的「接入包（Agent Pack）」**，而不是「多个复制按钮 + 两段先读文档」。

---

## 3. 设计原则

1. **一键优先**：主 CTA =「复制 Agent 接入包」；其余为次级。  
2. **真值优先**：包内写入当前环境的 **绝对 URL**、**workspace_id**、**刚创建的 key（若有）**；无 key 时明确「先创建密钥」而不是静默占位糊弄。  
3. **连接先于文档**：先能连，再链到文档；文档链接必须 **绝对 URL 且 200**。  
4. **人类 vs Agent 分流**：人类说明页 vs agent 可抓取文档；UI 标签写清「给人类 / 给 Agent」。  
5. **密钥不二次泄漏**：列表里永不回显完整 key；仅创建瞬间进入接入包与「仅显示一次」区。  
6. **与 PRODUCT_IA 一致**：API Access 仍挂在工作区（分享弹窗 / 工作区入口），不新增第三套 key 管理页。

---

## 4. 目标信息架构（面板内）

```text
┌─ ① 接入本工作区（主卡片）─────────────────────────┐
│  一句话：复制下方内容，粘贴给 Cursor / Claude / 自定义 Agent │
│  [ 复制完整接入包 ]  ← 唯一主按钮（大）              │
│  状态条：已含 key / 尚未创建 key / 桌面本地 vs 云端   │
│  可折叠预览：接入包 Markdown/文本（只读）            │
└──────────────────────────────────────────────────┘

┌─ ② 密钥（必要前置）───────────────────────────────┐
│  创建表单（默认 index+query，高级项折叠）             │
│  创建成功 → 自动刷新接入包（注入真 key）+ toast 提示  │
│  已有 key 列表：前缀 + 吊销（无完整 key）            │
└──────────────────────────────────────────────────┘

┌─ ③ 说明与链接（次级）─────────────────────────────┐
│  给人类：完整绝对链接 → /help/api-access           │
│  给 Agent：完整绝对链接 → 稳定 agent 文档（见 §6） │
│  可选：复制链接按钮（单条）                         │
└──────────────────────────────────────────────────┘

┌─ ④ 高级（默认折叠）───────────────────────────────┐
│  单独复制字段、agent user token（分享类工具）、     │
│  stdio vs HTTP 说明、探测命令                       │
└──────────────────────────────────────────────────┘
```

**删除/降权**：

- 去掉「推荐顺序：先读说明页 → 再读 agent 文档」作为主流程（与「一次复制」冲突）。  
- 去掉三行各自复制 `workspace_id` / base / MCP 作为**主路径**（保留在高级里）。  
- 裸路径 `/docs/...` 不再作为主 UI 文案；显示「Agent 说明文档」+ 绝对 URL。

---

## 5. Agent Pack 规范（一次复制的载荷）

### 5.1 格式

- **默认复制：纯文本 / Markdown**（外接 agent 系统提示或对话可直接吃）。  
- 内嵌 **JSON MCP 配置块**（Cursor / Claude Code 可原样粘贴）。  
- 编码：UTF-8；无 BOM；密钥行不换行。

### 5.2 字段（冻结契约）

| 字段 | 来源 | 必填 |
|------|------|------|
| `product` | 常量 `Context OS` | 是 |
| `workspace_id` | 当前工作区 | 是 |
| `api_base` | `resolveAgentApiBase()` 绝对 URL | 是 |
| `mcp_http` | `{api_base}/api/v1/mcp` | 是 |
| `api_key` | 本次创建的 plaintext；若无则为空 + 状态 `missing_key` | 条件 |
| `auth_header` | `Authorization: Bearer <api_key>` | 条件 |
| `docs_human` | `{origin}/help/api-access` | 是 |
| `docs_agent` | `{origin}<agent-doc-path>`（§6） | 是 |
| `permissions_hint` | 创建时勾选或默认 `index,query` | 建议 |
| `probe` | 一行 curl 或 `context-os-mcp --check` | 建议 |

### 5.3 模板示例（云端有 key）

```markdown
# Context OS — Workspace Agent Pack
# 把本块完整交给 Agent / 写入 MCP 客户端配置即可接入（勿提交到 git）

## Connection
- product: Context OS
- workspace_id: 9e3abf9d-cae9-43d2-882c-d27c05969c66
- api_base: https://app.contextlm.top
- mcp_http: https://app.contextlm.top/api/v1/mcp
- api_key: cos_ws_••••••••
- auth: Authorization: Bearer <api_key>

## MCP (Cursor / Claude Code) — stdio wrapper
```json
{
  "mcpServers": {
    "context-os": {
      "command": "context-os-mcp",
      "env": {
        "CONTEXT_OS_API_BASE": "https://app.contextlm.top",
        "CONTEXT_OS_API_KEY": "cos_ws_••••••••",
        "CONTEXT_OS_WORKSPACE_ID": "9e3abf9d-cae9-43d2-882c-d27c05969c66"
      }
    }
  }
}
```

## MCP tools note
- 每次 tools/call 的 arguments 须带 workspace_id（与上相同）。
- 工作区 key 仅用于 index/query 类工具；分享管理需用户 session / agent-token（高级）。

## Docs
- human: https://app.contextlm.top/help/api-access
- agent: https://app.contextlm.top/docs/api-access-for-agents.md

## Probe
curl -sS -X POST "$api_base/api/v1/mcp" \
  -H "Authorization: Bearer $api_key" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}'
```

### 5.4 无 key 时

- 主按钮文案改为：**「创建密钥后可复制完整包」**（disabled）或 **「复制接入包（缺 key）」**（包内 `api_key: <CREATE_KEY_FIRST>` + 醒目 `status: missing_key`）。  
- **推荐**：创建密钥成功后自动 `copy(pack)` 一次（可关），减少第二步。

### 5.5 桌面本地

- `api_base` 默认 `http://127.0.0.1:18080`（现有逻辑保留）。  
- Pack 增加一行：`runtime: desktop-local` 与「需先启动桌面客户端」提示。

---

## 6. 文档断链修复（必须与 UI 同 PR 或先于 UI）

### 6.1 根因

- 源文件在 `frontend_next/public/docs/api-access-for-agents.md`。  
- 现网 404；VPS standalone 目录下 **无** `public/docs/`（打包/挂载遗漏）。  
- Next App Router 对「仅 public 静态 md」在 standalone + 反向代理下脆弱。

### 6.2 方案（推荐组合）

| 层 | 做法 |
|----|------|
| **A. 稳定路由（主）** | 新增 App 路由 `GET /docs/api-access-for-agents`（或 `/help/api-access/agents`），**服务端读 md 并以 `text/markdown` 或渲染页返回**，保证 200 与缓存头 |
| **B. 兼容旧链** | `middleware` 或 `next.config` redirect：`/docs/api-access-for-agents.md` → A |
| **C. 部署校验** | `deploy-frontend.sh` 增加：`test -f` public docs **或** 路由 smoke `curl -f $origin/docs/...` |
| **D. Pack/UI 链接** | 一律 `new URL(path, window).href` 绝对地址；禁止只展示相对路径 |

文档内容精简方向（可选同 PR）：

- 文首 20 行 = **Connection 最小集**（与 Pack 对齐）。  
- 长表（工具列表、错误码）下沉，避免 agent 抓取噪声。  
- 去掉指向本机 repo 绝对路径的链接（现网不可达）。

---

## 7. 交互细节

| 行为 | 规格 |
|------|------|
| 主按钮 | 「复制完整接入包」；成功 toast「已复制，可粘贴给 Agent」 |
| 创建 key 后 | 注入 plaintext → 刷新 pack 预览 → 可选自动复制 |
| 密钥消失 | 刷新/关闭弹窗后 plaintext 清空；pack 回到 missing_key |
| 复制失败 | 展开 pack 预览 + 选中文本，提示手动 Ctrl+C |
| i18n | zh-CN / en 双语文案；**pack 正文建议固定英文键名**（agent 解析友好），说明句可双语 |
| a11y | 主按钮可键盘触发；预览 `pre` 可聚焦 |

---

## 8. 组件/文件改动草图（实现时）

| 文件 | 变更 |
|------|------|
| `components/api-access/workspace-api-access-surface.tsx` | 重构布局；`buildAgentPack(...)`；主复制；创建后注入 key |
| `components/api-access/workspace-api-access-surface.module.css` | 主 CTA、预览、折叠高级 |
| `lib/i18n/messages/api-access.ts` | 新文案；废弃「先读文档」主流程文案 |
| `app/.../docs/...` 或 `app/docs/.../route.ts` | agent 文档 200 |
| `public/docs/api-access-for-agents.md` | 精简 + 与 pack 字段对齐（可保留作源） |
| `scripts/deploy-frontend.sh` | smoke 文档 URL |
| `tests/api-access/*` | pack 含 workspace_id、绝对 docs、有 key 时无 placeholder |

**不改**：后端 MCP 路由、key 创建 API 契约（除非发现缺 `workspace_id` 字段返回）。

---

## 9. 验收标准

1. 现网（或预发）打开分享 → API Access：点 **一次**「复制完整接入包」。  
2. 粘贴到空白文件：含 **workspace_id、api_base、mcp_http、api_key（创建后）、docs 两条绝对 URL、MCP JSON**。  
3. 浏览器打开 pack 内 `docs_agent` 与 `docs_human` → **均 200**（不再 404）。  
4. 用 pack 内 MCP JSON + 真 key，Cursor/Claude 能 `initialize` / `tools/list`（手工冒烟）。  
5. 未创建 key 时：不能静默复制「像能用」的假完整包（须 missing_key 或 disabled）。  
6. 部署 frontend **不**触碰 DB / 支付热修（既有约束）。

---

## 10. PR 切分建议

| PR | 内容 | 风险 |
|----|------|------|
| **PR-1 文档可达** | 稳定路由 + redirect + deploy smoke | 低；先修断链 |
| **PR-2 Agent Pack UI** | 一键复制 + 布局重排 + 创建后注入 key | 中；纯前端 |
| **PR-3 文档瘦身** | md 与 pack 字段对齐、去死链 | 低 |

---

## 11. 明确不做什么

- 不在 URL query 里塞 api_key（防日志/Referer 泄漏）。  
- 不做「永久可回显历史 key」。  
- 不把 agent user token（分享工具）默认打进主 pack（仅高级；避免权限面扩大）。  
- 不在本设计落地 OAuth MCP（Firecrawl 式）；B2C workspace key 已够用。

---

## 12. 决策待你点头

1. **无 key 时主按钮**：disabled「先创建密钥」**(推荐)** vs 仍可复制缺 key 包？  
2. **创建 key 后是否自动复制**接入包？默认 **是**（可设本地 preference 关闭）。  
3. **Agent 文档路径**：保留 `/docs/api-access-for-agents.md`（redirect）还是规范到 `/help/api-access/agents`？

默认推荐：**1=disabled 先建 key，2=自动复制，3=新路由 + 旧路径 redirect**。

---

## 13. 附录：当前错误路径（实现对照）

```82:98:frontend_next/components/api-access/workspace-api-access-surface.tsx
function buildAgentMcpSnippet(apiBase: string): string {
  return JSON.stringify(
    {
      mcpServers: {
        "context-os": {
          command: "context-os-mcp",
          env: {
            CONTEXT_OS_API_BASE: apiBase,
            CONTEXT_OS_API_KEY: "<paste_workspace_api_key>",
          },
        },
      },
    },
    ...
  );
}
```

- 无 `CONTEXT_OS_WORKSPACE_ID`、无真 key、无 docs。  
- 文档区仅相对 `href`，且现网 md **404**。

以上即「一次复制即可接入」的调整设计；确认 §12 后可按 PR-1 → PR-2 落地。
