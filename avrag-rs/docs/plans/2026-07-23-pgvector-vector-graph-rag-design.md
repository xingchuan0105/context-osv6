# pgvector Vector Graph RAG 方案（移植自 Milvus 数据面）

**状态**: P0 implemented（2026-07-23）— `storage-pgvector` + migration `0060` + `RETRIEVAL_BACKEND`  
**日期**: 2026-07-23  
**目标读者**: 实现 `storage-pgvector` 适配器、本地/桌面私有化路径、运维  
**相关**:

- 现实现：`crates/storage-milvus`、`crates/retrieval-data-plane`
- 产品架构：`docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md`
- **图增强触发 / 证据得分落差（canonical）**：`docs/plans/2026-07-23-lexical-graph-augment-scoring-design.md`
- 图通道历史方案：`docs/plans/2026-07-04-vector-graph-rag-upgrade.md`（§2 dense 挂钩已废止；**云端仍保持 Milvus**）
- 图分析：`docs/plans/2026-07-04-graph-channel-analysis.md`

---

## 0. 结论与边界

### 0.1 要解决什么

| 痛点 | 本方案 |
|------|--------|
| 本机 Milvus 镜像大、拉起难 | 复用已有 **Postgres**，加 **pgvector** 扩展 |
| 桌面/私有化「数据不出机」 | 本地 PG 即控制面 + 检索面，无需第二重向量服务 |
| 又要向量 RAG + 图关系检索 | **完整移植** 当前 5 collection + 四通道 + graph BFS 语义 |

### 0.2 不做什么

- **不** 默认替换云端 SaaS 的 Milvus（大规模 / hybrid BM25 sparse 仍是 Milvus 强项）
- **不** 引入 Neo4j / Apache AGE（与 2026-07-04 选型一致：图 = 实体/关系向量 + 按名 BFS）
- **不** 改 `RagRuntime` 多通道编排契约；只换 `RetrievalDataPlane` 实现
- **不** 把 `chunks` 控制面表与检索表混成一张（避免 notebook/org 历史字段污染）

### 0.3 架构位

```text
                    ┌─────────────────────────────┐
                    │   RagRuntime / worker 索引   │
                    │  (DocumentIndexBatch 不变)   │
                    └──────────────┬──────────────┘
                                   │ RetrievalDataPlane
                 ┌─────────────────┴─────────────────┐
                 ▼                                   ▼
        storage-milvus                      storage-pgvector  (NEW)
        (SaaS / 默认)                       (local / desktop / 私有化)
                 │                                   │
              Milvus 5 coll                     PG 5 tables + HNSW
                                                 + FTS (lexical)
```

**部署选择**（env）：

```text
RETRIEVAL_BACKEND=milvus|pgvector   # 默认 milvus
# pgvector 路径复用 DATABASE_URL；维度与 metric 对齐现有 MILVUS_* 配置
```

---

## 1. 移植原则：尽量 1:1

| Milvus 概念 | pgvector 对应 | 说明 |
|-------------|---------------|------|
| collection | table `rag_*` | 表名 = 去前缀的逻辑名 + 可选 `collection_prefix` schema/表前缀 |
| FloatVector field | `vector(N)` + HNSW | 维度 = `text_vector_dim` / `multimodal_vector_dim` |
| COSINE metric | `vector_cosine_ops` + `<=>` 距离转相似度 | `score = 1 - (a <=> b)`（cosine distance） |
| BM25 sparse + analyzer | `tsvector` + `ts_rank`（**已有 PG FTS 习惯**） | 中文可用 `simple` 或后续 `zhparser`/`pg_jieba`；**不装** 第二套 sparse 引擎 |
| filter 表达式 | `WHERE` + 绑定参数 | `owner_user_id` / `doc_id = ANY($n)` |
| entities/query | `SELECT … WHERE` | 子图扩展用 SQL，可比 Milvus `in` 更清晰 |
| entities/search | `ORDER BY embedding <=> $q LIMIT k` | ANN |
| insert / delete | `INSERT` / `DELETE` | **事务** 包裹 replace（Milvus 是 best-effort 分 collection） |
| `DocumentIndexBatch` | 同构 JSON 行映射 | worker 零改或只改 data plane 注入 |

**上层契约零变更**：

- `RetrievalReadPort` / `RetrievalDataPlane`
- `DocumentIndexBatch` / `EntityIndexRecord` / `RelationIndexRecord` / …
- `GraphSearchRequest` / `GraphSearchOutput`（存储层可暂留 `GRAPH_CHUNK_SCORE = 0.85` 作 **channel_proxy telemetry**；**禁止**作图证据相关度/排序——见 lexical-graph-augment-scoring-design）
- channel 名在实现内改为 `pgvector_*`（trace 可区分后端）
- 图查询语义对齐 canonical：**1 hop 强制增强**、terms 种子、`graph_context` 分层；与后端 milvus|pgvector 无关

---

## 2. 逻辑模型：5 表 = 5 collection

字段名与 `storage-milvus/src/schema.rs` **对齐**，类型迁到 SQL。

### 2.1 公共列（所有表）

| 列 | 类型 | 来源 |
|----|------|------|
| `id` | `TEXT PRIMARY KEY` | Milvus primary `id`（chunk/entity/relation/passage 业务 id 字符串） |
| `owner_user_id` | `UUID NOT NULL` | 租户强制过滤 |
| `workspace_id` | `UUID NULL` | 可选 workspace 作用域 |
| `doc_id` | `UUID NOT NULL` | 文档作用域 |
| `parse_run_id` | `UUID NOT NULL` | 替换/清理 |
| `doc_version` | `INT NOT NULL` | 版本 |

索引（每表）：

```sql
CREATE INDEX … ON … (owner_user_id, doc_id);
CREATE INDEX … ON … (parse_run_id);
```

### 2.2 `rag_text_chunks` ← `*_rag_text_chunks`

| 列 | 类型 | Milvus field |
|----|------|----------------|
| `chunk_id` | `UUID NOT NULL` | chunk_id |
| `page` | `BIGINT NULL` | page |
| `text` | `TEXT NOT NULL` | text |
| `text_dense` | `vector(D_text) NOT NULL` | text_dense |
| `chunk_type` | `TEXT NOT NULL` | chunk_type |
| `parser_backend` | `TEXT NULL` | parser_backend |
| `source_locator` | `JSONB NULL` | source_locator |
| `search_vector` | `tsvector GENERATED …` | 替代 text_sparse + BM25 function |

```sql
-- 生成列示例（simple 与现有 migrations 一致；中文可后续换配置）
search_vector tsvector
  GENERATED ALWAYS AS (to_tsvector('simple', coalesce(text, ''))) STORED;

CREATE INDEX rag_text_chunks_dense_hnsw
  ON rag_text_chunks
  USING hnsw (text_dense vector_cosine_ops)
  WITH (m = 16, ef_construction = 64);

CREATE INDEX rag_text_chunks_fts_gin
  ON rag_text_chunks USING gin (search_vector);
```

### 2.3 `rag_multimodal_chunks` ← `*_rag_multimodal_chunks`

| 列 | 类型 |
|----|------|
| `chunk_id` | UUID |
| `asset_id` | UUID |
| `page` | BIGINT NULL |
| `context_text` | TEXT |
| `caption` | TEXT NULL |
| `image_path` | TEXT NULL |
| `multimodal_dense` | `vector(D_mm)` |
| `chunk_type` | TEXT |
| `parser_backend` | TEXT NULL |
| `retrieval_weight` | REAL NULL |
| `source_locator` | JSONB NULL |

HNSW on `multimodal_dense`。检索后 **乘** `retrieval_weight`（默认 1.0），对齐 `FALLBACK_RETRIEVAL_WEIGHT` 行为。

### 2.4 `rag_kg_entities` ← `*_rag_kg_entities`

| 列 | 类型 |
|----|------|
| `entity_id` | UUID |
| `name` | TEXT |
| `normalized_name` | TEXT |
| `entity_type` | TEXT NULL |
| `entity_dense` | `vector(D_text)` |
| `supporting_chunk_ids` | `UUID[]` 或 JSONB | **优先 UUID[]**（比 JSON 好 join） |
| `metadata` | JSONB NULL |

索引：

```sql
CREATE INDEX rag_kg_entities_dense_hnsw
  ON rag_kg_entities USING hnsw (entity_dense vector_cosine_ops);

CREATE INDEX rag_kg_entities_norm_name
  ON rag_kg_entities (owner_user_id, lower(normalized_name));
```

### 2.5 `rag_kg_relations` ← `*_rag_kg_relations`（图边 + 关系向量）

| 列 | 类型 | 作用 |
|----|------|------|
| `relation_id` | UUID | |
| `subject` | TEXT | BFS 按名扩展（**与现实现一致：按字符串实体名**） |
| `predicate` | TEXT | |
| `object` | TEXT | |
| `relation_text` | TEXT | 展示 / 作为 graph supporting content |
| `relation_dense` | `vector(D_text)` | 可选：关系语义 ANN（扩展用） |
| `supporting_chunk_ids` | UUID[] | |
| `metadata` | JSONB NULL | |

```sql
CREATE INDEX rag_kg_relations_dense_hnsw
  ON rag_kg_relations USING hnsw (relation_dense vector_cosine_ops);

-- 子图扩展关键路径
CREATE INDEX rag_kg_relations_subject
  ON rag_kg_relations (owner_user_id, subject);
CREATE INDEX rag_kg_relations_object
  ON rag_kg_relations (owner_user_id, object);
```

> **保留「按名 join」的增量友好性**（2026-07-04 文档核心论点）：新文档灌入后，只要实体字符串对齐即可连通，**不**强制全局 entity_id 外键图。  
> 可选增强（P2）：冗余列 `subject_entity_id` / `object_entity_id` 便于精确 join，**不**作为 BFS 唯一路径。

### 2.6 `rag_graph_passages` ← `*_rag_graph_passages`

| 列 | 类型 |
|----|------|
| `passage_id` | UUID |
| `chunk_id` | UUID NULL |
| `text` | TEXT |
| `passage_dense` | `vector(D_text)` |
| `relation_ids` | UUID[] |
| `metadata` | JSONB NULL |

当前 `search_graph` **未** 用 passage 向量做主路径（关系 query + 固定分）；表仍写入以保持 **replace_document_index 对称**，便于后续 passage ANN 或证据补全（与 Milvus 一致预留）。

### 2.7 表前缀 / 多环境

对齐 `MILVUS_COLLECTION_PREFIX`：

```text
prefix = avrag → 默认表名 rag_text_chunks …
或 schema: avrag_e2e.rag_text_chunks（E2E 隔离推荐 search_path / 独立 schema）
```

E2E：每个 context 用 `search_path` 或 `table_prefix`（`e2e_xxx_rag_text_chunks`），等价于今天的 collection prefix + drop。

---

## 3. 写入路径（移植 `replace_document_index`）

### 3.1 语义（与 Milvus 对齐，事务更强）

```text
BEGIN;
  -- Phase 0: purge 本 doc 在 5 张表中的全部行
  DELETE FROM rag_* WHERE owner_user_id = $owner AND doc_id = $doc;

  -- Phase 1: bulk insert 5 类记录（空则跳过）
  INSERT … text / multimodal / entities / relations / passages;

COMMIT;  -- 失败整单回滚（优于 Milvus 分 collection 半成功 + cleanup）
```

Milvus 的 `cleanup_current_parse_run` 在 PG 事务下 **可省略**；若要兼容观测，仍可打同样 metrics。

### 3.2 行映射（与 `ops/index.rs` 同构）

`text_chunks` 示例：

```text
id            = chunk_id
owner_user_id = batch.owner_user_id
workspace_id  = batch.workspace_id
doc_id        = batch.document_id
chunk_id      = chunk.chunk_id
parse_run_id  = batch.parse_run_id
doc_version   = batch.doc_version
page, text, text_dense, chunk_type, parser_backend, source_locator
-- text_sparse 不写：由 GENERATED search_vector 产生
```

`entities` / `relations` / `graph_passages` 同理，字段名保持 `entity_dense` / `relation_dense` / `passage_dense` 或在适配层映射为 SQL 列名（推荐 **SQL 列名与 Milvus field 同名**，降低心智切换）。

### 3.3 `delete_document_index`

```sql
DELETE FROM rag_text_chunks WHERE owner_user_id = $1 AND doc_id = $2;
-- ×5 表，单事务
```

### 3.4 `ensure_schema`

- 迁移：`avrag-rs/migrations/00xx_rag_pgvector.up.sql`（`CREATE EXTENSION IF NOT EXISTS vector`）
- 运行时：可选 `CREATE INDEX IF NOT EXISTS`（HNSW 大表创建成本高 → **只走 migration**）
- 维度：migration 用配置默认 1024；若要可配置 dim，采用：
  - **方案 A（推荐 v1）**：固定 1024，与 `.env` 默认一致；改 dim 需新 migration  
  - **方案 B**：`vector` 不设 dim 约束（pgvector 允许）+ 运行时校验 dim（与 `validate_document_batch_vector_dims` 同）

---

## 4. 读路径（四通道 + graph）

### 4.1 过滤（移植 `doc_filter` + `owner_user_id`）

```sql
WHERE owner_user_id = $auth_user   -- 与 milvus doc_filter 一致用 auth.user_id
  AND ($doc_ids IS NULL OR doc_id = ANY($doc_ids))
```

空 `doc_ids` 向量：与现实现一样 **直接返回空**（短路径）。  
Graph：额外 `owner_user_id`（`GraphSearchRequest.owner_user_id`）——**保持双字段现状**，不在本方案合并语义。

### 4.2 `search_text_dense`

```sql
SELECT chunk_id, doc_id, text AS content, page, chunk_type, …
     , 1 - (text_dense <=> $query::vector) AS score
FROM rag_text_chunks
WHERE …
ORDER BY text_dense <=> $query::vector
LIMIT $k;
```

`source = "pgvector_text_dense"`。

### 4.3 `search_bm25`（lexical）

```sql
SELECT …,
       ts_rank(search_vector, plainto_tsquery('simple', $q)) AS score
FROM rag_text_chunks
WHERE … AND search_vector @@ plainto_tsquery('simple', $q)
ORDER BY score DESC
LIMIT $k;
```

`Bm25SearchTrace.backend = "pgvector_fts"`。  
**差异说明（可接受）**：不是 Milvus 内置 BM25 sparse；中文分词质量取决于 text search config。后续可：

1. `pg_trgm` 兜底  
2. 接入 `zhparser` / `pg_jieba`  
3. 应用层稀疏向量（不优先）

### 4.4 `search_multimodal`

同 dense，表 `rag_multimodal_chunks`，字段 `multimodal_dense`；  
score 乘 `coalesce(retrieval_weight, 1.0)`。

### 4.5 `count_text_chunks` / `list_text_chunks`

```sql
SELECT count(*) …;
SELECT … FROM rag_text_chunks WHERE … LIMIT 16384;  -- 对齐 Milvus query cap 语义
```

### 4.6 `search_graph`（核心：移植 `ops/graph.rs`）

算法步骤 **原样保留**，存储换成 SQL：

```text
1. Seed entities
   - request.entity_names ∪ lower(query_entities)
   - 对每个 query_entity_vector:
       SELECT name FROM rag_kg_entities
       WHERE filter
       ORDER BY entity_dense <=> $v LIMIT 10

2. Multi-hop BFS (hop_limit, fan_out_limit, relation_limit)
   for hop in 0..hop_limit:
     SELECT relation_id, subject, predicate, object, relation_text,
            supporting_chunk_ids, doc_id, parse_run_id, …
     FROM rag_kg_relations
     WHERE owner filter AND doc filter
       AND (subject = ANY($boundary) OR object = ANY($boundary))
     LIMIT fan_out_limit

     - 去重 relation_id
     - 填 RelationPathCandidate(score=0.85 channel_proxy；**非**证据相关度)
     - 填 ScoredChunk(..., score=0.85 同上；证据排序见 lexical-graph-augment-scoring)
     - 下一跳实体 = 邻接 subject/object \ visited

3. return GraphSearchOutput { relation_paths, supporting_chunks }
```

**PG 可做的等价增强（行为兼容，性能更好）**——实现时默认 **算法对齐，SQL 可批量化**：

| 增强 | 说明 |
|------|------|
| 单次 hop 用 `= ANY($1::text[])` | 替代字符串拼 `in [...]` |
| 可选 recursive CTE | 多跳一次查出（v2；v1 保持循环以便与 Milvus 分步日志一致） |
| `normalized_name` 匹配 | seed 同时 match `name` 与 `lower(normalized_name)`（与大小写修复一致） |

**仍不做**：把图做成 AGE property graph；关系主路径仍是 **名字邻接**，不是 FK 强制图。

### 4.7 可选：`relation_dense` / `passage_dense` ANN

当前 graph 主路径不用；保留 upsert + 索引，便于后续：

- 用 `relation_hints` 做 relation 向量预召回再 BFS  
- passage 补证据  

不进 v1 验收。

---

## 5. 代码布局（crate 边界）

```text
crates/storage-pgvector/          # NEW
  Cargo.toml                      # sqlx, pgvector crate, retrieval-data-plane
  src/
    lib.rs                        # PgvectorDataPlane
    config.rs                     # dim, metric, table_prefix, database_url
    schema.rs                     # DDL helpers / table names（对照 milvus schema）
    ops/
      index.rs                    # replace / delete（事务）
      search.rs                   # dense / fts / multimodal / count / list
      graph.rs                    # 移植 graph.rs
    row_map.rs                    # ScoredChunk 映射
    tests.rs

crates/app-bootstrap/             # 按 RETRIEVAL_BACKEND 装配 Arc<dyn RetrievalDataPlane>
bins/worker/                      # 同上注入
migrations/00xx_rag_pgvector_*.sql
```

**禁止**：在 `storage-milvus` 里 if-else 两后端；保持 T 层单一实现 + bootstrap 选择。

测试：

- 单元：过滤 SQL 构造、score 换算、BFS 纯逻辑（可用 stub pool）  
- 集成：`#[ignore]` 或 feature `pgvector-e2e`，需本机 `CREATE EXTENSION vector`  
- 契约：复用 `retrieval-data-plane/tests/behavioral_contract.rs` 模式（Stub → 真 PG）

---

## 6. 配置与运维

### 6.1 环境变量

| 变量 | 默认 | 含义 |
|------|------|------|
| `RETRIEVAL_BACKEND` | `milvus` | `milvus` \| `pgvector` |
| `DATABASE_URL` | 已有 | pgvector 与控制面可同库 **或** 分库 |
| `PGVECTOR_TABLE_PREFIX` | `rag_` | 表前缀 |
| `PGVECTOR_TEXT_DIM` | 同 `MILVUS_TEXT_VECTOR_DIM` | 1024 |
| `PGVECTOR_MM_DIM` | 同 multimodal | 1024 |
| `PGVECTOR_METRIC` | `cosine` | 仅 cosine v1 |
| `PGVECTOR_HNSW_EF_SEARCH` | `40` | 会话级 `SET hnsw.ef_search` |

### 6.2 依赖安装（本地）

```bash
# Debian/Ubuntu 示例
sudo apt install postgresql-16-pgvector   # 版本对齐本机 PG
# 或
CREATE EXTENSION vector;                 # 超级用户一次
```

Docker：官方 `pgvector/pgvector:pg16` 镜像可替换开发用 PG（**不必** 再拉 milvus/etcd/minio）。

### 6.3 体积对比（量级）

| 栈 | 本机额外体积 |
|----|----------------|
| Milvus standalone 三件套 | ~4GB 镜像 + volumes |
| pgvector | 扩展库数十 MB 级 + 数据随语料增长 |

### 6.4 备份与私有化

- 桌面/私有化：整库 `pg_dump` = 用户 + 会话 + **检索索引** 一体  
- SaaS：继续 Milvus + PG 分治；pgvector 路径可不部署

---

## 7. 与现有 PG 表关系

| 表 | 角色 | 本方案 |
|----|------|--------|
| `chunks` / `document_multimodal_chunks` | 产品/解析控制面 | **不写向量**；worker 继续写控制面 + 另写 `rag_*` |
| `rag_*` | 检索数据面 | **仅** pgvector 后端使用 |
| 未来 | 可从 `chunks` 回填 | 不在 v1 |

避免与历史 `org_id` / `notebook_id` 列纠缠（T8：`user_id` / `workspace_id`）。

---

## 8. 行为差异清单（验收时显式接受）

| 项 | Milvus | pgvector v1 | 影响 |
|----|--------|-------------|------|
| Lexical | BM25 sparse + chinese analyzer | `simple` tsvector | 中文分词弱；可后续增强 |
| 事务 | 分 collection | 单事务 5 表 | **更好** |
| ANN 算法 | AUTOINDEX | HNSW | 小中规模足够 |
| 空库冷启动 | 服务进程 | 仅需 PG | **更好** |
| 十亿级 | 擅长 | 不作为目标 | SaaS 仍用 Milvus |
| channel 字符串 | `milvus_*` | `pgvector_*` | trace 区分 |
| Graph | query API | SQL ANY | 语义同、实现更干净 |

---

## 9. 实施分期

### P0 — 可运行适配器（约 2–3 天）

1. migration：`vector` 扩展 + 5 表 + HNSW + GIN  
2. `storage-pgvector`：`ensure_schema`（校验）、`replace` / `delete`  
3. dense + multimodal + FTS + graph BFS  
4. bootstrap：`RETRIEVAL_BACKEND=pgvector`  
5. 单测 + 本地手工：灌 1 文档 → 四通道有结果  

**验收**：

- `ensure_schema` + replace 后 5 表行数对齐 batch  
- `search_text_dense` / `search_bm25` / `search_graph` 在 fixture 上非空  
- 与 Milvus **同一 `DocumentIndexBatch`** 结构无改  

### P1 — 产品接线与开发体验（约 1 天）

1. `product-dev-up` / 文档：本地默认可切 pgvector，**可不启 Milvus**  
2. E2E：`product_e2e` 可选 `RETRIEVAL_BACKEND=pgvector` 路径  
3. worker / api 配置示例写入 `.env.example`  

### P2 — 质量与桌面（按需）

1. 中文 FTS 配置  
2. `hnsw.ef_search` / 索引参数调优  
3. 桌面安装包捆绑 pgvector 扩展或内嵌 PG  
4. relation/passage ANN 实验通道  

### 明确不做（本方案）

- 双写 Milvus+PG  
- 在线从 Milvus 迁云数据（可另开迁移工具）  
- AGE / Neo4j  

---

## 10. 关键算法伪代码（graph，便于对照 `graph.rs`）

```rust
// seed
let mut seeds = entity_names ∪ lower(query_entities);
for v in query_entity_vectors {
    seeds.extend(ann_entity_names(v, k=10, filter));
}
if seeds.is_empty() { return empty; }

let mut visited = seeds.clone();
let mut boundary = seeds;
let mut relations = vec![];
let mut chunks = vec![];
let mut seen_rid = HashSet::new();

for _ in 0..hop_limit {
    if boundary.is_empty() || relations.len() >= relation_limit { break; }
    let rows = sql_relations_touching(boundary, fan_out_limit, filter);
    let mut next = HashSet::new();
    for row in rows {
        if !seen_rid.insert(row.relation_id) { continue; }
        if relations.len() < relation_limit {
            relations.push(path_candidate(row, score=0.85));
            if chunks.len() < supporting_chunk_limit {
                chunks.push(scored_relation_chunk(row, "pgvector_graph_relation"));
            }
        }
        for n in [row.subject, row.object] {
            if !visited.contains(&n) { next.insert(n); }
        }
    }
    visited.extend(next.iter().cloned());
    boundary = next;
}
```

---

## 11. 风险与缓解

| 风险 | 缓解 |
|------|------|
| HNSW 构建在大批量 insert 后变慢 | replace 按文档删除再插；大库考虑 `SET maintenance_work_mem` |
| 中文 FTS 弱 | P2 分词；图/ dense 通道仍为主干 |
| 与控制面同库争锁 | 检索表无 FK 到业务表；大负载可分 `DATABASE_URL` |
| 维度变更 | v1 固定 1024；变更走 migration |
| 双后端漂移 | behavioral contract 测试 + 字段映射表（本文 §2）作为单一事实来源 |

---

## 12. 决策记录（建议 ADR 一句话）

> **检索数据面允许双后端**：SaaS/默认 = Milvus；本地/私有化/开发可选 = Postgres+pgvector。  
> 图检索继续采用 **实体/关系向量化 + 按实体名 BFS**（vector graph RAG），不引入专用图数据库。  
> 对外契约固定为 `RetrievalDataPlane`；worker 索引批次结构不变。

---

## 13. 实现状态

### P0 已落地

| 项 | 位置 |
|----|------|
| migration | `migrations/0060_rag_pgvector.{up,down}.sql` |
| crate | `crates/storage-pgvector`（官方 **`pgvector` crate** + sqlx） |
| 开关 | `RETRIEVAL_BACKEND=milvus\|pgvector`（`app-core` / bootstrap / worker） |
| 示例 env | `.env.example` |
| smoke | `cargo test -p avrag-storage-pgvector --test smoke_pgvector -- --ignored`（需 `DATABASE_URL`） |

本地系统包：`postgresql-16-pgvector`；扩展 `CREATE EXTENSION vector`。

### 启用方式

```bash
# .env
RETRIEVAL_BACKEND=pgvector
DATABASE_URL=postgres://...
AVRAG_RUN_MIGRATIONS=true
# 可不启 Milvus
```

重启 `avrag-api` + `avrag-worker`。云端默认仍为 `milvus`。

### 后续（P1+）

1. `product-dev-up` 可选跳过 Milvus 当 backend=pgvector  
2. E2E 路径可选 `RETRIEVAL_BACKEND=pgvector`  
3. 中文 FTS / 桌面捆绑  

**不需要** 等组件都齐：两条路径并行，开发机可默认 pgvector，云端继续 milvus。
