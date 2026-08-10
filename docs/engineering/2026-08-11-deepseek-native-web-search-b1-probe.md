# B1 实测：DeepSeek 原生联网 vs 现网 Brave `client.web`

- **日期**: 2026-08-11  
- **状态**: 实测门禁（未接入产品；Brave 仍为现行 `client.web` 后端）  
- **关联**: 产品拍板 — search 极简题卡 + dense 仅 RAG；联网数据源待 B1 后再定  

## 1. 目标

确认 DeepSeek **官方 API** 是否能在产品 agent 路径上替代 Brave 作为「联网检索」数据源。

## 2. 现行路径（对照）

```text
client.web(query)
  → host web_search
  → SEARCH_PROVIDER=brave_llm_context
  → api.search.brave.com
```

## 3. 实测矩阵

| # | 协议 / 端点 | 请求形态 | 期望 |
|---|-------------|----------|------|
| A | OpenAI-compat `POST /v1/chat/completions` | 普通 chat，无 tools | 应成功；**无**联网结果字段 |
| B | OpenAI-compat + `tools: [{type: web_search}]` 或官方名 | tool calling | 看是否 400 / 是否返回 search tool |
| C | Anthropic-compat `POST /anthropic/v1/messages` | Claude tools / web_search | 文档称 Claude Code 侧原生 Web Search |
| D | 官方 docs 是否声明 Chat Completions 联网开关 | 文档阅读 | 记录结论 |

## 4. 通过标准

| 结论 | 条件 |
|------|------|
| **可用-B-native** | C 或 B 稳定返回可解析的搜索片段，可映射为 `web_search` observation |
| **可用-B-host** | 有独立 search HTTP API（非 chat）可宿主调用 |
| **不可用** | 仅网页 chat 有联网；API 无 tool / 无 endpoint → **暂留 Brave** |

## 4.1 实测结论（2026-08-11）

见 `docs/engineering/_reports/deepseek-web-b1-latest.md`。

| 路径 | 结果 |
|------|------|
| OpenAI `type: web_search` | 400 |
| OpenAI `function web_search` | 200 仅 model tool_calls（宿主仍要搜） |
| Anthropic + `web_search_20250305` | **200 server-side 搜索结果** |

→ **可用-B-native（Anthropic 路由）**；B2 再改产品接线。Brave 暂留。

## 5. 本机运行

```bash
# 读取 avrag-rs/.env 的 AGENT_LLM_* / E2E_LLM_*
bash scripts/probe-deepseek-web-search-b1.sh
# 报告：docs/engineering/_reports/deepseek-web-b1-latest.md
```

## 6. 产品决策（待 B1 报告）

- 若可用 → 设计 `deepseek_web` provider 与 Brave 切换/降级  
- 若不可用 → 保持 Brave；UI 文案不承诺「DeepSeek 原生联网」
