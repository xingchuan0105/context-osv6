# P2 agentd（含加深）

**日期：** 2026-08-11  
**状态：** pi RPC 宿主 + 出站闸 + **检索 harness 联调** + **HTTP/SSE** 已通。

## 架构

```
用户/HTTP
   │
agentd (Go) ── pi --mode rpc ── osv7-harness.ts ── retrieval-cli ── Service
   │                                 │                    │
   └─ outbound gate                  │                    ├ store/index
      billing usage stub             └ set_query_card/    └ rag_text_chunks
                                       lexical/dense/grep
```

- **主 agent 工具**：`.pi/extensions/osv7-harness.ts` 一等 tool → `bin/retrieval-cli`（与 MCP **同一** `retrieval.Service`）。
- **外接 agent**：仍走 `retrieval-mcp` stdio/HTTP。
- **全局 pi packages 默认关闭**（`--no-extensions` + 显式 `-e`），避免 `pi-mcp-adapter` 与 `@mariozechner` 版本 skew 崩 RPC。

## 组件

| 路径 | 说明 |
|------|------|
| `internal/agentd/*` | RPC、闸、Host、Event 回调 |
| `cmd/agentd-chat` | CLI（`-harness -workspace`） |
| `cmd/agentd-server` | `POST /v1/chat`、`POST /v1/chat/stream`（SSE）、`GET /healthz` |
| `cmd/retrieval-cli` | 题卡/检索 CLI + 文件 snapshot 多进程会话 |
| `.pi/extensions/osv7-harness.ts` | pi 工具桥 |
| `prompts/agentd-harness-append.md` | 第三人称检索环境观察 |

## 冒烟结果

### 1) 纯 chat（无 harness）

```text
answer=2  ~3s
```

### 2) harness 检索（滴灌通 DRC）

```text
tools: set_query_card, lexical, dense
answer: DRC = Daily Revenue Contract（每日收入分成合约）…
duration_ms: ~7.5s
```

### 3) HTTP

```bash
curl -sS -X POST http://127.0.0.1:8090/v1/chat \
  -H 'Content-Type: application/json' \
  -d '{"message":"只输出数字：3-1=？","harness":false}'
# → answer "2"
```

SSE：`POST /v1/chat/stream` 事件 `delta` / `tool_start` / `tool_end` / `done` / `result`。

## 命令

```bash
bash scripts/p2-agentd-smoke.sh          # 无 harness
bash scripts/p2-harness-smoke.sh         # CLI 检索 + agentd 检索 + HTTP
OSV7_AGENTD_ADDR=:8090 ./bin/agentd-server
```

## P2 收口（多轮 + PG 投影 + card-keeper 软信号）

| 能力 | 状态 |
|------|------|
| 多轮 | 第二轮带 `session_id` 恢复 pi transcript（暗号「蓝鸟」复测通过） |
| PG 投影 | `osv7_sessions` / `osv7_messages`；列表与气泡只存闸后正文 |
| card-keeper 软 | `card_missing` / `retrieval_invoked` / `card_observation`（第三人称；不硬拦交付） |
| API | `GET /v1/sessions`、`GET /v1/sessions/{id}/messages` |

```bash
bash scripts/p2-session-smoke.sh
```

## 仍未做

- 前端 `lib/api` 灰度  
- websearch 插件 + dual-web 事故复测  
- card-keeper 硬拦 deliver（可选；现软信号 + agentd 闸）  
- 余额真扣  
- SSE 与 v6 事件字段 100% 对齐