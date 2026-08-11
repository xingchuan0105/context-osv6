# B2：DeepSeek web_search 接入 `client.web`

- **日期**: 2026-08-11  
- **状态**: **Responses API**（强制 `tool_choice=web_search`）；CRW 仅补薄 snippet；**VPS 待通知**  
- **前置**: B1 Anthropic 可用；Responses 探针见 `_reports/deepseek-responses-web-latest.md`  

## 1. 产品路径

```text
client.web(query)
  → host web_search skill
  → SearchExecutor (SEARCH_PROVIDER)
       ├ deepseek_web_brave (默认)
       │    1) DeepSeek POST {base}/responses + tools web_search + tool_choice web_search
       │    2) 解析 web_search_call (open_page URL；若有 markdown/text 写入 snippet)
       │    3) snippet 仍薄 → CRW auto-scrape top-K（有正文则跳过）
       │    4) 空结果 / 错误 → Brave llm/context
       ├ deepseek_web
       └ brave_llm_context
```

- **News vertical**（`vertical=news`）：DeepSeek 无独立 news 端点 → 直接走 Brave news。  
- **Agent 主对话 LLM** 仍用 `AGENT_LLM_*`（OpenAI-compat）；联网数据源与 chat 协议解耦。  

## 2. 配置

| 变量 | 用途 |
|------|------|
| `SEARCH_PROVIDER` | `deepseek_web_brave`（默认）/ `deepseek_web` / `brave_llm_context` |
| `SEARCH_API_KEY` / `SEARCH_BASE_URL` | Brave |
| `SEARCH_DEEPSEEK_BASE_URL` / `_API_KEY` / `_MODEL` | 可选；未设则复用 `AGENT_LLM_*` |

## 3. 代码位置

| 组件 | 路径 |
|------|------|
| `SearchConfig`（crate） | `avrag-rs/crates/search/src/config.rs` |
| 路由 + fallback | `…/search/src/executor.rs` |
| Anthropic parse/execute | `…/search/src/provider.rs` |
| 产品 env → config | `avrag-rs/crates/app-core/src/config.rs` |
| bootstrap map | `avrag-rs/crates/app-bootstrap/src/lib.rs` (`map_avrag_search_config`) |

## 4. 验证

```bash
# 单元（含 Anthropic 解析 fixture）
cd avrag-rs && cargo test -p avrag-search --lib

# 可选 live（需 AGENT_LLM_API_KEY 或 SEARCH_DEEPSEEK_API_KEY）
cargo test -p avrag-search --lib deepseek_web_live_smoke -- --ignored --nocapture
```

## 5. 非目标（本切片）

- 不改 verify / three-loop 策略  
- 不把 DeepSeek 联网结果当终答（仍是 observation 源）  
- **不自动 VPS 部署**  
