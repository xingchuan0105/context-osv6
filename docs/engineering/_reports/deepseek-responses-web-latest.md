# DeepSeek Responses vs Anthropic web_search — 实测

- **time_utc**: 2026-08-10 (session calendar 2026-08-11)
- **query**: `What is the capital of France and which river runs through it?`
- **key**: product `AGENT_LLM_API_KEY`
- **model**: `deepseek-v4-flash`

## 结果对照

| # | 路径 | HTTP | 墙钟 | 实际联网？ | 可解析 URL | 摘要/正文 |
|---|------|------|------|------------|------------|-----------|
| **A** | `POST /responses` + `tools:[{type:web_search}]` | 200 | **~0.8–2.3s** | **本次未触发搜**（仅 message） | 0 | 模型直接答 Paris/Seine |
| **B** | `POST /anthropic/v1/messages` + `web_search_20250305`（**现行产品**） | 200 | **~7s** | **是**，2× tool_result 各 10 条 | **20 title/url** | snippet 无；有 `encrypted_content` + 模型 text 总结 |
| **D** | `POST /responses` + `tool_choice:{type:web_search}` | 200 | **~22s** | **是**，多步 `web_search_call`（search / open_page / find_in_page） | action 内有 URL | 服务端会 open_page；本响应 output 以 tool 步为主 |

## 关键结论

1. **Responses + 仅声明 `tools: web_search` 不等于一定会搜**  
   简单事实题模型可能直接答（A 秒回），**没有** `web_search_call`，也**没有**链接 annotations。这解释了「秒回」——有时根本是参数知识，不是联网。

2. **现行 Anthropic 路径（B）是真的在服务端搜**  
   有 `web_search_tool_result` + title/url 列表；慢在多轮 tool + 大 body（~37KB）。  
   **不是空调**，但是 **Claude tool 形态**，摘要仍薄（encrypted）。

3. **Responses 强制 `tool_choice=web_search`（D）会真搜，且会 `open_page`**  
   比「只搜 URL」更像网页端的「搜+看页」；墙钟更长（~22s 本例），因多步 open_page/find_in_page。  
   这是更接近官方文档、且**可能减少我们 CRW** 的方向——需再解析完整 message/引用字段。

4. **网页端秒回** ≠ API 必走 B；也可能是 A 类「不搜直答」或专用产品链路。

## 对产品接线的含义

| 选项 | 含义 |
|------|------|
| 继续 B | 已通；要厚证据仍需 CRW/auto-scrape 或 Brave |
| 迁 A（不强制 tool） | 快但不保证联网，**不适合** `client.web` 证据闸 |
| 迁 D / Responses + 强制 web_search | 官方主路径；要解析 `web_search_call` + 最终 message；可能自带 open_page |
| 混合 | `client.web` 用 Responses 强制搜取 URL/摘录；仍可 auto-scrape 补正文 |

## 产物

- `docs/engineering/_reports/deepseek-responses-A.full.json`
- `docs/engineering/_reports/deepseek-anthropic-B.full.json`
- `docs/engineering/_reports/deepseek-responses-D.full.json`
- 探针脚本：`scripts/probe-deepseek-responses-web.sh`（shell 版 A/B 有小瑕疵；以本报告 Python 实测为准）
