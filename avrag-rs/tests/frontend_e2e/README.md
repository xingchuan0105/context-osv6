# Frontend E2E — Full-Stack, Real-LLM, Golden-Set Quality

## 架构

```
Playwright browser ←→ Next.js frontend (port 3001) ←→ Rust backend (port 8080)
                                ↑
                         LLM judge (DashScope)
```

- **Backend** is launched by `cargo test ... backend_launcher` (reuses `product_e2e` infra)
- **Frontend** is built (`pnpm build`) and started (`pnpm start`) pointing at the backend
- **Tests** run in Chromium, serially (`workers=1`) because real LLM APIs have rate limits

## 8 个用户旅程

| # | 场景 | 文件 |
|---|---|---|
| 1 | 文件上传 + ingestion | `specs/01-upload-ingestion.spec.ts` |
| 2 | RAG 问答 + citation | `specs/02-rag-qa.spec.ts` |
| 3 | Search 问答 + web citation | `specs/03-search-qa.spec.ts` |
| 4 | Chat 多轮会话 | `specs/04-chat-session.spec.ts` |
| 5 | Notebook CRUD | `specs/05-notebook-crud.spec.ts` |
| 6 | 格式化输出 (HTML) | `specs/06-format-output.spec.ts` |
| 7 | Session 历史持久化 | `specs/07-session-history.spec.ts` |
| 8 | 多租户隔离 | `specs/08-tenant-isolation.spec.ts` |

## 质量评估

- **Hard assertions** (product rules): citation count, keyword presence, HTTP status
- **Golden set**: `fixtures/golden_set.json` — 每题有预期标准
- **LLM judge**: DashScope `qwen-plus` 评分 (accuracy, completeness, citation_quality)
- **Score gate**: ≥ 7/10 才算通过

## 运行

```bash
cd avrag-rs/tests/frontend_e2e
pnpm install

# 运行全部（自动启动 Rust backend + Next.js frontend）
pnpm test

# 打开 UI 模式调试
pnpm test:ui

# 查看报告
pnpm report
```

## 配置

从 `.env` 读取（复用主仓库配置）：
- `AGENT_LLM_BASE_URL`, `AGENT_LLM_API_KEY`, `AGENT_LLM_MODEL`
- `EMBEDDING_BASE_URL`, `EMBEDDING_API_KEY`, `EMBEDDING_MODEL`
- `DASHSCOPE_API_KEY` (for judge)

## 产物

```
output/
├── report/                  # Playwright HTML report
├── artifacts/               # 失败截图、录屏、trace
└── {run_id}/
    ├── response.json        # 每个测试的 answer
    ├── metadata.json        # 模型、时间、citation_count
    └── judge-result.json    # LLM judge 评分
```

## 成本估算

- ~¥0.004 / test (DeepSeek flash + DashScope embedding)
- 8 场景 ≈ ¥0.032 / run
- ¥10 / month budget gate

## 已知限制

- 前端 selector 基于常见 `data-testid` 或文本匹配；若 UI 变动需要更新 POM
- 真实 LLM 有 rate limit；必须 `--workers=1` 串行
- ChatResponse 暂无 `usage` 字段；token 成本为近似值
