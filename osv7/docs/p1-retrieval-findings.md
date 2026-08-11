# P1 检索腿：落地记录

**日期：** 2026-08-11  
**状态：** 薄切片已通；**full-149 检索子集（Layer A）已跑**（见 §P1 收口）。

## 已实现

| 组件 | 路径 | 说明 |
|------|------|------|
| `store` | `internal/store` | 唯一 SQL 连接池；workspace 存在性 / chunk 计数 |
| `index` | `internal/index` | lexical（CJK LIKE / FTS）、grep、dense（embed + `<=>`） |
| `billing` | `internal/billing` | 能力表 stub + usage 日志（未扣余额） |
| `retrieval` | `internal/retrieval` | 题卡、会话别名、SELECTED/KEEP、verify_draft、资源/契约闸 |
| MCP server | `cmd/retrieval-mcp` | 工具注册；stdio 默认；`OSV7_MCP_HTTP_ADDR` 可开 Streamable HTTP |
| client smoke | `cmd/retrieval-client` + `scripts/p1-retrieval-smoke.sh` | 无卡拦截 → set card → lexical/dense → select/keep → verify |

### MCP 工具

`set_query_card` · `lexical` · `dense` · `grep` · `select_evidence` · `keep_evidence` · `verify_draft` · `retrieval_status`

### 闸行为（实测）

1. **无卡调 lexical** → `query_card_missing`（IsError）  
2. **set_query_card** → 校验 workspace 存在；`chunk_count` 返回  
3. **required_actions** 跟踪 Ok；`verify_draft` 在缺失 Ok 时 `contract_gate`  
4. **句柄** `#1…` 跨工具递增；reseen 同 chunk 复用 alias  

### 验证命令

```bash
cd osv7
# source avrag-rs/.env for DATABASE_URL + EMBEDDING_*
bash scripts/p1-retrieval-smoke.sh
go test ./internal/retrieval/ -count=1
```

## 刻意未做（后续）

| 项 | 阶段 |
|----|------|
| `struct_catalog` / `struct_query` / `doc_summary` | P1.1 或随数据表 |
| 身份/RLS 注入（token → user，不信模型 arg） | P1+ / 与 HTTP 鉴权一起 |
| 余额真扣 + preflight 地板 | P4 |
| Streamable HTTP 多会话 map | P2 agentd 前 |
| full-149 全量语料 ingest + Layer A 再跑 | 本地仅 ~13 题有 needle；需导入 full-149 语料 |
| full-149 经 pi+MCP 端到端（含 agent） | P2 |
| 证据句柄线协议与 v6 SaC 打印格式 100% 字节兼容 | 对照后微调 |

## 包依赖（纪律）

```
cmd/retrieval-mcp → retrieval, index, store, billing, config
retrieval → index, store, billing
index → store
store → pgx only
```

## P1 收口：full-149 检索子集（Layer A）

**命令：**

```bash
bash scripts/p1-full149-subset.sh available   # 默认：仅本地有 gold needle 的题
# bash scripts/p1-full149-subset.sh all      # 全部检索资格题（缺语料会大量 miss）
```

**实现：** `cmd/retrieval-eval` — 读 `golden_set_realistic.json`，筛检索资格（有 `source_chunks`、非纯 chat），`available` 模式用 needle∈`rag_text_chunks` 过滤；每题 `lexical`∪`dense` top-k=15 合并后对 gold substring 算 hit/recall。

**本机一次结果（2026-08-11）：**

| 指标 | 值 |
|------|-----|
| eligible（检索资格） | 123 / 149 |
| available ran | **13** |
| skipped（无语料） | 110 |
| **hit_rate** | **0.769**（10/13） |
| mean_recall | 0.705 |
| 门槛 | `fail-below=0.5` **通过** |

报告：`docs/_reports/p1-full149-available-latest.json`

**结论：** 检索腿在**现库可见语料**上可工作；**不是** full-149 产品闸门（109/149 agent 终答），那要全量语料 + P2 agent。缺语料 110 题属数据缺口，非 harness 逻辑 bug。