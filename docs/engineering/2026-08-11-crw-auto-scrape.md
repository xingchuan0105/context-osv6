# CRW auto-scrape：DeepSeek web 后宿主读页

- **日期**: 2026-08-11  
- **状态**: 已接线本机；Docker CRW 默认 **:3100**（避开 Next :3000）  

## 路径

```text
client.web(query)
  → DeepSeek / Brave → results[]
  → host auto_scrape (CRW POST /v1/scrape) top-K unique thin URLs
  → results[].snippet = markdown (truncated)
  → observation（模型一般不必再 fetch 同 URL）
```

## 配置

| 变量 | 默认 |
|------|------|
| `WEB_AUTO_SCRAPE` | `1` |
| `CRW_BASE_URL` | `http://127.0.0.1:3100` |
| `CRW_API_KEY` | 空（本机 docker） |
| `WEB_AUTO_SCRAPE_TOP_K` | `4` |
| `WEB_AUTO_SCRAPE_MAX_CHARS` | `4000` |
| `WEB_AUTO_SCRAPE_MIN_SNIPPET` | `80`（Brave 厚摘要跳过） |
| `WEB_AUTO_SCRAPE_TIMEOUT_MS` | `12000` |
| `WEB_AUTO_SCRAPE_CONCURRENCY` | `4` |

```bash
bash scripts/dev-crw-up.sh
# CRW_BASE_URL=http://127.0.0.1:3100
```

## 代码

| 位置 | 职责 |
|------|------|
| `avrag-search/src/crw.rs` | scrape client + enrich |
| `SearchExecutor::dispatch_search` | web 后 enrich |
| `WebFetchSkill` | 有 `CRW_BASE_URL` 时走 CRW，否则旧 regex HTML |

## 许可

CRW 引擎 AGPL；产品仅 **HTTP 调用**，不链接引擎 crate。
