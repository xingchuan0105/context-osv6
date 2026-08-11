# P0 spike 实测：pi × MCP × 会话 × 进程

**日期：** 2026-08-11  
**环境：** pi `@mariozechner/pi-coding-agent@0.73.1`（npm 已弃用提示 → 后续用 `@earendil-works/pi-coding-agent`）、Go 1.25、本地 PG `avrag_rs`（`RETRIEVAL_BACKEND=pgvector`，`rag_text_chunks` ≈ 474 行）  
**范围：** 设计 §9.7 四问 + hello-retrieval；**不**进入 P1 产品 MCP 题卡/闸。

---

## 产出清单

| 产物 | 路径 | 状态 |
|------|------|------|
| monorepo 骨架 | `osv7/` | ✅ |
| hello-retrieval MCP（stdio + lexical） | `osv7/cmd/hello-retrieval-mcp` | ✅ 命中 v6 数据 |
| MCP client（Go） | `osv7/cmd/hello-retrieval-client` | ✅ |
| 与 pi-mcp-adapter 同 SDK 路径 | `osv7/scripts/p0-pi-mcp-path.mjs` | ✅ |
| pi-mcp-adapter 安装 | `pi install npm:pi-mcp-adapter` | ✅ |
| hard-block 探针扩展 | `osv7/p0/extensions/hard-block-probe.ts` | ✅ 代码；未烧 LLM 全链路 |
| RPC / 进程 smoke | `osv7/scripts/p0-pi-rpc-smoke.sh` | ✅ |

复现：

```bash
cd osv7
bash scripts/p0-hello-retrieval.sh          # → total_hits≥1 for 滴灌通
set -a; source ../avrag-rs/.env; set +a
node scripts/p0-pi-mcp-path.mjs             # adapter 自带 SDK → lexical
bash scripts/p0-pi-rpc-smoke.sh
```

---

## 四问答案

### 1. pi 如何连 MCP？

| 事实 | 说明 |
|------|------|
| **pi 核心不内置 MCP** | 需 extension；官方生态用 **pi-mcp-adapter**（`pi install npm:pi-mcp-adapter`） |
| **配置** | `.mcp.json` / `~/.pi/agent/mcp.json` / `~/.config/mcp/mcp.json` 等；stdio server = `command` + `args`，**继承进程 env**（勿把 `DATABASE_URL` 明文写进仓内 json） |
| **P0 验证的 transport** | **stdio**（client 拉起 `hello-retrieval-mcp` 子进程） |
| **prod 默认不变** | Streamable HTTP 多租户（设计 §9.1）；P0 未实现 HTTP MCP |
| **懒连接** | adapter 文档：默认首调再连、空闲断；工具发现可缓存 |
| **agentd 集成面** | **`pi --mode rpc`**（stdin/stdout JSONL）为主；Node 侧也可嵌 `AgentSession` SDK，但 Go 宿主应走 RPC 子进程 |

**hello-retrieval：**  
Go MCP `lexical` → `rag_text_chunks`（tsvector / ILIKE）→ 查询「滴灌通」返回 hits（workspace/doc/chunk/snippet）。  
同一路径用 adapter 内 `@modelcontextprotocol/sdk` 的 `StdioClientTransport` 再验一次 ✅。

**与完整「pi 会话内 tool 调用」的差距：** 未跑带 LLM 的 agent 轮（adapter 的 `mcp` 代理 tool 需模型发起）。路径与二进制已通；P2 接通时补一次 print/rpc + model 冒烟即可。

---

### 2. 插件 / card-keeper 硬度？

| 能力 | 结论 |
|------|------|
| **tool 前硬拦** | **支持。** `pi.on("tool_call", …)` 返回 `{ block: true, reason?, terminate? }` 即可阻止执行（官方 extensions 文档 + `hard-block-probe.ts`） |
| **tool 后改结果** | **支持。** `tool_result` 可改写 content / isError |
| **注入观察** | **支持。** `pi.sendMessage` / `sendUserMessage`（steer / followUp / nextTurn） |
| **终答「用户气泡」硬闸** | **不在 pi 产品语义内。** pi 无 TUI/RPC 事件流；**交付过滤放 `agentd` 出站薄闸**（设计已定） |
| **拦 deliver 的 pi 侧近似** | `message_end` 可改写 assistant 消息；`agent_settled` 后 `sendUserMessage` 可强制续轮 —— 适合 card-keeper「未履行则不许收束」的 soft/hard 组合 |

**card-keeper 落点（收敛）：**

1. **扩展内：** 订阅 `tool_call` / `tool_execution_*` / `tool_result`，对照题卡声明（websearch 调用、harness MCP 调用）。  
2. **未履行：** `sendMessage` 第三人称 observation（prompts 资产，P2 再挂 md）。  
3. **结构违规硬拦 tool：** `tool_call` → `{ block: true }`。  
4. **用户可见终答：** **agentd** 过滤协议残片 / tool transcript；可选在 RPC 侧丢弃未过闸的 `message_end` 文本。

---

### 3. 会话真源？

| 事实 | 说明 |
|------|------|
| **格式** | 树状 **JSONL**；header `type: "session"` + 消息/自定义 entry |
| **真源** | **pi transcript 文件**（与设计默认一致） |
| **重要行为** | **`SessionManager` 在出现首条 `assistant` 消息之前不落盘**（`_persist` 见 dist：`hasAssistant` 为 false 则只记内存、`flushed=false`）。仅 bash / 空会话时 `get_state.sessionFile` 有路径但**文件可不存在** |
| **RPC** | `get_state` → `sessionFile` / `sessionId`；`switch_session` / `new_session` / `fork` / `clone`；`set_session_name`；用量 `get_session_stats`（token/cost） |
| **默认目录** | `~/.pi/agent/sessions/<encoded-cwd>/`；可用 `--session-dir` |
| **PG 投影** | 仍由 agentd 从 transcript（或 RPC `get_messages` / 事件流）投影；**禁止 UI 反写真源** |

**对 agentd：**

- 会话 ID ↔ `sessionFile` 路径映射存 PG。  
- 可靠持久化 = **至少完成一轮含 assistant 的 turn**，或评估是否 fork 上游 / 包一层强制 flush（勿假定空会话文件已在盘上）。  
- resume = 新 pi 进程 + `switch_session` / `--session <path>`。

---

### 4. 进程模型？

| 事实 | 说明 |
|------|------|
| **RPC 宿主** | `pi --mode rpc` = **一个 Node 进程 / 一个活跃会话** |
| **实测 RSS** | 空闲 RPC 约 **110–120 MB RSS / 进程**（本机 0.73.1，两进程并行） |
| **并发** | N 在线活跃会话 ≈ N 个 pi 进程；**不要跨会话池化同一 pi 解释器** |
| **MCP 子进程** | stdio MCP = 再由 client 拉起（adapter 懒连接）；prod HTTP MCP 则共享服务进程 |
| **冷热** | 热：保持 RPC 管道；冷：落盘 transcript 后杀进程，下次 `--session` / `switch_session` 附着 |

**资源控：** 并发上限 + idle timeout（配置项，非架构分叉）。单机 50 会话量级粗估 ~5–6 GB 仅 pi RSS，需与产品预期对齐。

---

## 对设计文档的影响

| 项 | 动作 |
|----|------|
| 五默认 + monorepo `osv7/` | **不变** |
| transcript 真源 | **确认**；补注「首 assistant 前不落盘」 |
| tool 硬拦 | **确认可用** → card-keeper 可 hard-block **tool** |
| deliver 硬闸 | **确认在 agentd**（pi 无用户气泡） |
| hello-retrieval | **stdio 已通 v6 数据**；P1 换 HTTP + 题卡/闸 |
| 包名 | 已装 `@mariozechner/*@0.73.1`；后续升级 `@earendil-works/*` |

---

## 未做 / 下步

1. **带 LLM 的 pi 一轮：** adapter `mcp` tool → `lexical`（需 provider key 已在 pi auth；本机 env 有 ANTHROPIC/DEEPSEEK/GEMINI，未在 P0 烧 token）。  
2. **P1：** `retrieval-mcp` Streamable HTTP、题卡、资源/契约闸、证据句柄；`store`/`index` 吃同一 `rag_text_chunks`。  
3. **transcript 强制 flush 策略**（agentd）：是否接受「无 assistant 不落盘」或自定义扩展 `appendEntry` 旁路。  
4. **RSS 压测表**（10/50 会话）可在 P2 前补。

---

## 命令与依赖速查

```text
pi --version          # 0.73.1
pi install npm:pi-mcp-adapter
pi list
go build -o bin/hello-retrieval-mcp ./cmd/hello-retrieval-mcp
DATABASE_URL=... ./bin/hello-retrieval-client 滴灌通
```
