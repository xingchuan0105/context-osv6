# P3 摄入腿

**日期：** 2026-08-11  
**状态：** 双生产者薄切片已通（agent 制备包 + server 文本解析）→ embed → `rag_text_chunks` → lexical 可见。

## 契约

| 项 | 说明 |
|----|------|
| Schema | `osv7-document-ir-p3.0` |
| 工具 | `ingest_preflight` / `ingest_begin` / `ingest_blocks` / `ingest_summary` / `ingest_package` / `ingest_commit` |
| 硬校验 | title、blocks 非空、summary 长度带、KG 形状、覆盖度启发 |
| preflight | embedding hosted\|byok\|missing；缺 embedding 拒 begin/commit |
| 索引 | 每 block 一次 embed；INSERT `rag_text_chunks`（同步 commit，P3 无 Redis 队列） |

## 双生产者

1. **agent_package**：JSON DocumentIr → begin → package → commit  
2. **server_parse**：`.md`/文本切段 → 同一 commit 路径  

解析器 anydoc/markitdown/liteparse 适配器 **未移植**（P3 后可挂 `Command`）；当前 server 路径是确定性文本切分。

## 冒烟

```bash
bash scripts/p3-ingest-smoke.sh
```

结果摘要：

| 标记 | 结果 |
|------|------|
| ALPHA-P3-AGENT-PACKAGE-9917 | lexical hits=1 |
| BETA-P3-SERVER-PARSE-8826 | lexical hits=1 |

## 组件

| 路径 | 角色 |
|------|------|
| `internal/ingest` | IR、校验、Session、Commit |
| `cmd/ingest-mcp` | MCP stdio |
| `cmd/ingest-cli` | agent-package / server-parse CLI |

## 未做

- Redis `queue` 异步 + 有界终态  
- anydoc / markitdown / liteparse / OCR 适配器  
- documents 表元数据行 / MinIO blob  
- KG / summary 索引与检索原语  
- 余额真扣 preflight 地板