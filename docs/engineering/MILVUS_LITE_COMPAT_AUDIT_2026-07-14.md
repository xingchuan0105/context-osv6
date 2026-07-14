# Milvus Lite 兼容性审计（向量知识图谱）

**Date:** 2026-07-14  
**Scope:** `avrag-rs/crates/storage-milvus` + 检索读路径（RAG text / BM25 / multimodal / graph）  
**Baseline product:** Docker **Milvus Standalone**（`MILVUS_URL` HTTP + REST v2）  
**Candidate:** **Milvus Lite**（嵌入式 `.db` 或 `milvus-lite server` gRPC）  
**Goal:** 是否可在「不丢向量知识图谱」前提下用 Lite 替换 Docker 全家桶  

---

## 1. Executive verdict

| 问题 | 结论 |
|------|------|
| Lite 能否 **无改代码** 替换当前 Standalone？ | **否** |
| 稠密向量 + 图谱多跳检索是否 **有机会** 跑在 Lite 上？ | **有条件可能**（功能面偏可；传输面要改） |
| 当前 **全文 BM25（中文 analyzer + BM25 function）** 在 Lite 上？ | **高风险 / 可能不可用**（官方与生态均提示 Lite 全文能力不全） |
| 是否建议客户端默认改 Lite？ | **现阶段不建议默认切换**；可做 **spike** 验证 gRPC/REST 与 BM25 |

**一句话：**  
产品「向量知识图谱」的 **图扩展核心**（实体/关系稠密向量 + filter query）与 Lite 的「小规模稠密检索」方向较接近；但 **实现栈绑了 HTTP REST v2**，且 **text 通道依赖 Standalone 级 BM25 全文**，这两点使 Lite **不能**被当作无损、即插即用的轻量 Standalone。

---

## 2. 我们实际依赖了什么（代码事实）

### 2.1 传输与接入

| 项 | 本仓库事实 | Lite 典型形态 | 风险 |
|----|------------|---------------|------|
| 客户端 | `reqwest` **HTTP POST** `MILVUS_URL` + 路径 `/v2/vectordb/...`（见 `storage-milvus/src/lib_impl.rs`） | 嵌入式 Python / **gRPC :19530** | **P0**：Lite **不保证**暴露与 Standalone 相同的 **REST v2** 面；现有 Rust 客户端无法假定 `http://127.0.0.1:19530/v2/vectordb/entities/search` 可用 |
| URL 配置 | `MILVUS_URL=http://127.0.0.1:19530` | gRPC 同端口或不同 | 协议不匹配则全链路失败 |
| Token | 可选 Bearer | Lite 无 RBAC | 低（我们可空 token） |
| `dbName` | `with_database` 写入 body | Lite 多为单 namespace | 中：需忽略或固定 default |

**结论：** 兼容的第一道硬门是 **传输**，不是「有没有 FloatVector」。

### 2.2 五个集合（`MilvusConfig::collection_names`）

| 集合后缀 | Schema 要点 | 索引 | 用途 |
|----------|-------------|------|------|
| `{prefix}_rag_text_chunks` | `FloatVector text_dense` + `SparseFloatVector text_sparse` + **BM25 function**（`text` → sparse）+ 中文 analyzer | dense `AUTOINDEX` + sparse **`SPARSE_INVERTED_INDEX` / metric BM25** | 稠密召回 + **BM25 全文** |
| `{prefix}_rag_multimodal_chunks` | 仅 `FloatVector multimodal_dense` + 标量/JSON | dense `AUTOINDEX` | 多模态稠密 |
| `{prefix}_rag_kg_entities` | `FloatVector entity_dense` + JSON 支撑 chunk | dense `AUTOINDEX` | 图谱实体向量 |
| `{prefix}_rag_kg_relations` | `FloatVector relation_dense` + subject/predicate/object | dense `AUTOINDEX` | 图谱关系向量 + **标量 filter 扩展** |
| `{prefix}_rag_graph_passages` | `FloatVector passage_dense` | dense `AUTOINDEX` | 图相关 passage |

前缀默认 `avrag`；客户端可用 `avrag_client` 等隔离。

### 2.3 读路径（产品功能映射）

| 产品能力 | 代码入口 | Milvus 操作 | 图谱相关？ |
|----------|----------|-------------|-----------|
| 文本稠密检索 | `search_text_dense` | `entities/search` on `text_dense` | 间接（RAG） |
| **BM25 全文** | `search_bm25` | `entities/search` on **`text_sparse`**，query 为**原始中文 query 字符串** | 间接 |
| 多模态 | `search_multimodal` | dense on `multimodal_dense` | 间接 |
| **向量知识图谱** | `search_graph` | ① 实体 dense 搜 seed ② **query** 关系表 `subject in … \|\| object in …` 多跳 BFS ③ 支撑 chunk | **是（核心）** |
| 扫描/计数 | `count_text_chunks` / `list_text_chunks` | `entities/query` + filter，limit≤16384 | 工具链 |

图谱 **不依赖 partition**；扩展靠 **filter + query**，稠密 seed 靠 **FloatVector + AUTOINDEX**。  
BM25 在 **text_chunks**，是检索质量重要通道，但是 **与图 BFS 正交**。

### 2.4 我们明确未使用（对 Lite 有利）

| 能力 | 使用？ |
|------|--------|
| Partition / partition key | **否** |
| Distributed / 多副本 | **否** |
| RBAC / 用户角色 | **否**（应用层 `owner_user_id` filter） |
| PQ / IVF 显式参数 | **否**（`AUTOINDEX`） |
| float16 / binary 向量 | **否**（`FloatVector`） |
| 跨库多 database 业务 | 仅可选 `dbName` |

---

## 3. 对照 Milvus Lite 能力矩阵

依据官方文档/仓库公开说明（2026 公开页）：Lite 与 Standalone **API 家族相近**，适合小规模；**不支持 partition**；无认证；稀疏/稠密/hybrid 有宣称支持；**全文检索在 Lite 上不完整或未齐**（多处文档写 full-text 仅 Standalone/Distributed，Lite roadmap）。

| 本仓库能力 | Lite 公开能力印象 | 判定 |
|------------|-------------------|------|
| FloatVector + AUTOINDEX + COSINE/L2 | 支持稠密检索 | **OK（待实测）** |
| entities/search、insert、delete、query、list/describe/create collection | 取决于是否同一 **REST v2** 或仅 gRPC | **P0 传输：未知/偏否** |
| SparseFloatVector + BM25 function + 中文 analyzer + SPARSE_INVERTED_INDEX | 稀疏有；**全文 BM25 管线高风险** | **高风险** |
| JSON 字段、nullable、filter `in` / `==` | 通常可用（过滤强度待测） | **中** |
| 多集合、小数据个人库 | Lite 定位匹配 | **OK** |
| 多跳 graph query 大 fan-out | 小规模 OK；大数据 query 性能/limit 风险 | **中（规模）** |
| 与云端完全同构运维 | 否 | **产品差异** |

---

## 4. 知识图谱路径专项

### 4.1 图谱写路径（ingest）

- Worker 抽 triplet → 写入 `kg_entities` / `kg_relations` / `graph_passages`（稠密向量 + 元数据）。  
- 不依赖 BM25 function。  
- **若 Lite 支持 FloatVector collection create/insert**：图谱 **写入** 有机会通过。

### 4.2 图谱读路径（`search_graph`）

```
seed entities (名字 + entity_dense ANN)
  → multi-hop: query kg_relations WHERE subject|object in boundary
  → 收集 supporting_chunk_ids → 组装 GraphSearchOutput
```

| 步骤 | 依赖 | Lite 风险 |
|------|------|-----------|
| entity dense ANN | FloatVector search | **低–中**（传输通则可能 OK） |
| relation scalar query + `in` 列表 | query + filter | **中**（filter 语法/`in` 大列表） |
| hop / fan_out | 应用层 | OK |
| passage dense（若上层使用） | FloatVector | **低–中** |

**结论：**  
「**向量知识图谱**」在本仓库的 **主路径是稠密 + 标量图扩展**，**不依赖** text BM25。  
因此：在 **解决传输** 且 **稠密检索可用** 的前提下，**图谱核心有较大概率不丢**；  
**BM25 质量通道** 仍可能丢，影响的是 **整体 RAG 质量**，不是图 BFS 本身。

---

## 5. 风险分级（给决策用）

### P0 — 阻断「无改代码切换」

1. **HTTP REST v2 vs Lite gRPC**  
   - 现网：`MilvusDataPlane::post_json("/v2/vectordb/...")`  
   - Lite：优先 gRPC / Py API。  
   - **必须** 二选一：为 Lite 加 **gRPC 适配层**，或确认某版 Lite 提供兼容 REST。

### P1 — 可能丢功能/质量

2. **text_chunks BM25 全文管线**（analyzer=chinese、BM25 function、SPARSE_INVERTED_INDEX、query 字符串搜 sparse 字段）  
3. **`dbName` 语义** 与 Lite 单库  
4. **规模**：`query` limit 16384、多跳 fan_out；个人小库可接受，大库不行  

### P2 — 可接受差异

5. 无 partition / 无 RBAC（我们本就没用）  
6. AUTOINDEX 参数差异导致召回略变  
7. 运维与云端 Standalone 不完全同构  

---

## 6. 与「客户端 37MB 安装包」的关系

- Lite **不能** magically 塞进现 37MB 壳而不增加体积；但可比 Docker Milvus 全家桶 **轻一个数量级以上**。  
- 即便 Lite 功能过关，仍要：  
  - Windows 可运行的 **Lite server 二进制或运行时**  
  - 安装/升级/数据目录  
  - 与 `avrag-api` 的 **连接适配**（REST 或 gRPC）  

---

## 7. 建议决策

| 选项 | 建议 |
|------|------|
| **A. 默认继续 Docker Standalone** | **推荐当前**（与云同构、BM25/图谱已验证路径） |
| **B. Lite 作为「轻量档」** | **仅在 spike 通过后**：个人小库 + 接受 BM25 降级或 PG FTS 兜底 |
| **C. 立刻把产品默认改 Lite** | **不推荐**（P0 传输 + P1 BM25 未证伪） |

### Spike 清单（若做 B）

1. 起 `milvus-lite server`（或文档中的 gRPC 形态），确认是否存在 **REST `/v2/vectordb`**。  
2. 若无 REST：估算 **storage-milvus gRPC 适配** 工作量（create/insert/search/query/delete）。  
3. 在 Lite 上跑现有集成测：`storage-milvus/tests/milvus_adapter.rs`（含 **BM25** 与 reindex）。  
4. 单独跑 **graph search** 冒烟（实体 ANN + 多跳 relation query）。  
5. 记录：BM25 是否失败；若失败，产品是否接受「轻量档关闭 BM25 / 改 PG FTS」。  

**通过标准（建议）：**

- ensure_schema 五集合成功  
- dense text + multimodal + graph 检索有命中  
- BM25：**Pass** 或 **明确产品降级策略**  
- 个人文档量级延迟可接受  

---

## 8. 代码锚点（审计依据）

| 区域 | 路径 |
|------|------|
| 集合与 schema | `avrag-rs/crates/storage-milvus/src/schema.rs` |
| 集合注册 | `avrag-rs/crates/storage-milvus/src/lib.rs` (`ensure_schema`) |
| HTTP REST 客户端 | `avrag-rs/crates/storage-milvus/src/lib_impl.rs` |
| Dense / BM25 / multimodal | `avrag-rs/crates/storage-milvus/src/ops/search.rs` |
| Graph 多跳 | `avrag-rs/crates/storage-milvus/src/ops/graph.rs` |
| 集合命名 | `avrag-rs/crates/storage-milvus/src/config.rs` |
| 集成测（含 BM25） | `avrag-rs/crates/storage-milvus/tests/milvus_adapter.rs` |

---

## 9. 审计结论表（最终）

| 能力域 | Lite 兼容判定 | 说明 |
|--------|---------------|------|
| 传输 REST v2 | **阻塞** | 现客户端绑 HTTP v2 |
| 图谱实体/关系稠密 + 多跳 query | **有条件可** | 不依赖 partition/BM25；需传输与 dense OK |
| 文本稠密 RAG | **有条件可** | FloatVector |
| BM25 全文 | **高风险** | 中文 analyzer + BM25 function 属 Standalone 级全文 |
| 多模态稠密 | **有条件可** | 单 dense 字段 |
| 与云端完全同构 | **否** | 轻量档必然分叉运维 |
| 无 Docker 默认客户端 | **需产品+工程 spike** | 非「已集成」 |

**最终建议：**  
把 Lite 记为 **可行的轻量候选（图谱主路径理论上可保）**，但 **当前实现不能无损切换**；下一步若推进，先做 **传输兼容 + BM25 实证**，再谈 Windows 打包体积。
