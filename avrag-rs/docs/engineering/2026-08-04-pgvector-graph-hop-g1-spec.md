# pgvector 图 hop 验证门禁规格（G1 为主）

**日期**: 2026-08-04  
**状态**: **G1 + G2 implemented**（2026-08-04）— G1：`storage-pgvector/tests/graph_hop_g1.rs`（17）；G2：`rag-core/tests/vgrag_pgvector_g2.rs`（3）；G3 未做；**不改**产品默认栈  
**动机**: 产品 VGRAG（dense 内 hop=2）已在 **Milvus** 路径验收；`storage-pgvector` 有多跳 BFS 实现，但仅有 `#[ignore]`、**hop=1** 人造 smoke。桌面若要以 pgvector 替代 Docker+Milvus，必须先过本规格门禁。  
**相关代码**:

| 层 | 路径 |
|----|------|
| 端口 | `crates/retrieval-data-plane` — `GraphSearchRequest` / `GraphSearchOutput` |
| pgvector 实现 | `crates/storage-pgvector/src/graph.rs` |
| Milvus 实现 | `crates/storage-milvus/src/ops/graph.rs` |
| 现有 smoke | `crates/storage-pgvector/tests/smoke_pgvector.rs`（不足） |
| 产品 hop | `crates/rag-core/src/runtime/tools/vgrag.rs` — `VGRAG_HOPS = 2` |
| Schema | `migrations/0060_rag_pgvector.up.sql` — `rag_kg_entities` / `rag_kg_relations` |

**门禁总览**（本文件详写 G1；G2/G3 只定口径）:

| Gate | 范围 | 通过标准 | 实现优先级 |
|------|------|----------|------------|
| **G1** | `PgvectorDataPlane::search_graph` 契约（无 LLM） | 本文件全部 **MUST** 用例 PASS | **Done** |
| **G2** | `DENSE_BACKEND=vgrag` + `RETRIEVAL_BACKEND=pgvector` 编排 | 固定小库上 `relation_n`/`graph_n` 非空 | **Done** |
| **G3** | graph 质量抽样（graph81 子集或等价） | 与里程碑阈值对比（可低于云端） | P2 |

**桌面决策**: G1 未过 → 禁止宣称「本机 pgvector = VGRAG 就绪」。**G1+G2 已过 → 可开发便携 PG 栈**；G3 过 → 可切换桌面默认检索后端 / 宣称 VGRAG 质量对齐。

---

## 0. 契约摘要（实现不得偏离）

### 0.1 输入 `GraphSearchRequest`（与端口一致）

| 字段 | 语义 |
|------|------|
| `owner_user_id` | 强制租户过滤 |
| `doc_ids` | `Some([])` → 空结果；`Some(ids)` → 仅这些文档；`None` → 该 owner 下全部（G1 以 `Some` 为主） |
| `entity_names` | 种子实体 **原样** 加入 boundary（不强制 lower） |
| `query_entities` | trim 后 **`to_lowercase`** 加入种子 |
| `query_entity_vectors` | 对 `rag_kg_entities.entity_dense` ANN，取 `name` 进种子 |
| `hop_limit` | BFS 层数；VGRAG 产品为 **2** |
| `fan_out_limit` | **每跳** 关系行上限 |
| `relation_limit` | 全局累计关系路径上限 |
| `supporting_chunk_limit` | supporting_chunks 上限 |

### 0.2 扩展语义（与当前 pgvector 实现对齐后固化）

每一 hop：

1. 查询 `rag_kg_relations`：  
   `owner_user_id = $owner AND doc_id = ANY($docs) AND (subject = ANY($boundary) OR object = ANY($boundary))`  
   `LIMIT fan_out_limit`
2. 边加入 `relation_paths`（按 `relation_id` 去重）
3. 边的另一端实体进入 **下一跳** boundary（未访问过的）
4. 若累计 relations ≥ `relation_limit` 则停止

**字符串匹配**: 当前实现是 **SQL 精确匹配** `subject`/`object` 与 boundary 字符串。  
种子若与边上字面不一致（大小写、空格、全半角），**扩不出去**——G1 必须显式覆盖，避免「实现有 hop 循环但种子永远对不上」。

### 0.3 固定维度

测试向量 dim = **1024**（与 `0060` / 产品 `MILVUS_*_DIM` 默认一致）。  
用 one-hot 或正交 unit 向量区分实体即可，不依赖真实 embedding 模型。

---

## 1. G1 测试环境

### 1.1 运行时

| 项 | 要求 |
|----|------|
| 库 | Postgres + **pgvector** 扩展（`CREATE EXTENSION vector`） |
| 迁移 | 至少 `0060_rag_pgvector` 已应用（`rag_kg_entities` / `rag_kg_relations` 存在） |
| 推荐 | CI：`pgvector/pgvector:pg16` 容器；本地可复用 monorepo PG 或桌面 `:5433` **仅当** 已装 pgvector |
| 包 | `avrag-storage-pgvector` 的 **非 ignore** integration test（或 `tests/graph_hop_g1.rs`） |
| 清理 | 每测例独立 `owner_user_id` + `doc_id`；结束 `delete_document_index` 或 TRUNCATE 该 owner |

### 1.2 禁止

- 依赖外网 LLM / embedding API  
- 依赖 Milvus  
- 仅靠 `#[ignore]` 的 smoke 充当 G1（可保留 smoke，**不能**替代 G1）

### 1.3 建议文件布局（实现时）

```text
avrag-rs/crates/storage-pgvector/tests/
  graph_hop_g1.rs          # 本规格全部用例
  support/graph_fixture.rs # 可选：构图 helper
```

或 `src/graph.rs` 旁 `#[cfg(test)]` 模块 + testcontainers；以 **CI 默认可跑** 为准。

---

## 2. 标准图夹具（Fixture Graph-Chain）

所有「跳数」用例共用同一逻辑图，除非用例另有说明。

### 2.1 文档与租户

| 符号 | 类型 | 说明 |
|------|------|------|
| `OWNER` | UUID | 本测例租户 |
| `DOC` | UUID | 单文档 scope |
| `OTHER_OWNER` | UUID | 隔离负例 |
| `OTHER_DOC` | UUID | 同 owner 另一文档（可选） |
| `PARSE` | UUID | `parse_run_id` |
| `CHUNK_*` | UUID | supporting chunks |

### 2.2 实体（写入 `entities[]`）

| entity_id 符号 | name（边上/检索用字面） | normalized_name | 向量热点 index |
|----------------|-------------------------|-----------------|----------------|
| `E_A` | `Alpha` | `alpha` | 1 |
| `E_B` | `Beta` | `beta` | 2 |
| `E_C` | `Gamma` | `gamma` | 3 |
| `E_D` | `Delta` | `delta` | 4 |

向量：`unit_vec(1024, hotspot)` — 仅 index `hotspot` 为 1.0，其余 0。

### 2.3 关系（写入 `relations[]`）

链：`Alpha --depends_on--> Beta --uses--> Gamma --owns--> Delta`

| relation_id | subject | predicate | object | relation_text | 向量热点 |
|-------------|---------|-----------|--------|---------------|----------|
| `R_AB` | `Alpha` | `depends_on` | `Beta` | `Alpha depends_on Beta` | 10 |
| `R_BC` | `Beta` | `uses` | `Gamma` | `Beta uses Gamma` | 11 |
| `R_CD` | `Gamma` | `owns` | `Delta` | `Gamma owns Delta` | 12 |

每条边 `supporting_chunk_ids = [CHUNK_AB]` 等（可共用一个 chunk 或分三个；G1 只要求 `supporting_chunks` 非空且可关联）。

### 2.4 可选岔路边（fan-out / limit 用例）

从 `Beta` 再出 5 条边到 `Z1..Z5`（name=`Z1`…），用于测 `fan_out_limit` / `relation_limit` 截断。默认夹具 **不含** 岔路，仅 **G1-F** 系列启用。

### 2.5 写入方式

统一走 `PgvectorDataPlane::replace_document_index(DocumentIndexBatch { ... })`，与生产索引路径一致。  
`text_chunks` 至少 1 条（内容随意），`multimodal_chunks` / `graph_passages` 可空。

---

## 3. G1 用例目录

命名：`g1_<area>_<id>`。  
断言里「边」用 `(subject, predicate, object)` 三元组；允许 `relation_paths` 顺序不稳定，用 **集合** 比较。

### 3.1 跳数语义（核心 — MUST）

#### G1-H1 — hop=1 只见直接邻居

| | |
|--|--|
| **预置** | Fixture Graph-Chain |
| **请求** | `entity_names=["Alpha"]`, `hop_limit=1`, `fan_out_limit=50`, `relation_limit=50`, `doc_ids=Some([DOC])`, `owner_user_id=OWNER` |
| **MUST** | `relation_paths` 的三元组集合 **等于** `{(Alpha, depends_on, Beta)}` |
| **MUST NOT** | 出现 `Beta uses Gamma` 或 `Gamma owns Delta` |
| **MUST** | `supporting_chunks.len() >= 1` |

#### G1-H2 — hop=2 到达二跳（**VGRAG 对齐关键）

| | |
|--|--|
| **预置** | 同上 |
| **请求** | 同 H1，但 `hop_limit=2` |
| **MUST** | 三元组集合 **⊇** `{(Alpha, depends_on, Beta), (Beta, uses, Gamma)}` |
| **MUST NOT** | 出现 `(Gamma, owns, Delta)`（三跳边） |
| **说明** | 产品 `VGRAG_HOPS=2`；本测是桌面切 pgvector 的 **最低硬门槛** |

#### G1-H3 — hop=3 走完全链

| | |
|--|--|
| **请求** | `hop_limit=3`，种子 `Alpha` |
| **MUST** | 三元组集合 **⊇** 三条边 `R_AB, R_BC, R_CD` |

#### G1-H0 — hop_limit=0

| | |
|--|--|
| **请求** | `hop_limit=0`，种子 `Alpha` |
| **MUST** | `relation_paths` 为空（循环 `0..0` 不跑） |

#### G1-H2-MID — 从中段种子 hop=2

| | |
|--|--|
| **请求** | `entity_names=["Beta"]`, `hop_limit=2` |
| **MUST** | 至少包含与 Beta 相邻的边：`R_AB` 与/或 `R_BC`（无向邻接：subject **或** object 命中） |
| **MUST** | 在 hop=2 下应能触及 `Alpha` 侧与 `Gamma` 侧（具体边集合：`{(Alpha,depends_on,Beta),(Beta,uses,Gamma)}` 的超集或相等，视 fan-out） |
| **说明** | 防止实现只沿 subject→object 单向扩 |

---

### 3.2 种子入口（MUST）

#### G1-S1 — `entity_names` 表面形式

| | |
|--|--|
| **请求** | `entity_names=["Alpha"]`, `query_entities=[]`, `hop_limit=1` |
| **MUST** | 命中 `R_AB`（与 H1 一致） |

#### G1-S2 — `query_entities` 小写 vs 边表面大写（**已知风险**）

| | |
|--|--|
| **请求** | `entity_names=[]`, `query_entities=["alpha"]`, `hop_limit=1` |
| **期望（产品意图）** | 应能扩到 `R_AB` |
| **实现** | **(a) 已落地**：`resolve_entity_surface_names` + `lower(subject|object) = ANY(boundary_lower)`；测例 `g1_s2_query_entities_lowercase_matches_surface_edges` |

#### G1-S3 — 实体 ANN 种子

| | |
|--|--|
| **预置** | 实体向量见 §2.2 |
| **请求** | `entity_names=[]`, `query_entities=[]`, `query_entity_vectors=[unit_vec(1024,1)]`（贴近 Alpha）, `hop_limit=1` |
| **MUST** | ANN 返回的 name 含 `Alpha`（或等价），且 `relation_paths` 含 `R_AB` |
| **容差** | 若 HNSW 在极小数据上不稳定，可设 `hnsw_ef_search` 提高或小表改用顺序扫；**禁止** 因 flaky 删测 |

#### G1-S4 — 空种子

| | |
|--|--|
| **请求** | 三类种子皆空，`hop_limit=2` |
| **MUST** | 空 `GraphSearchOutput`，且不 error |

#### G1-S5 — 多种子

| | |
|--|--|
| **请求** | `entity_names=["Alpha","Gamma"]`, `hop_limit=1` |
| **MUST** | 集合 ⊇ `{R_AB, R_CD}`（Alpha 与 Gamma 的一跳边） |

---

### 3.3 截断与去重（MUST）

#### G1-L1 — `relation_limit`

| | |
|--|--|
| **预置** | Graph-Chain + 从 Beta 出 5 条岔路（§2.4） |
| **请求** | 种子 `Alpha`, `hop_limit=2`, `relation_limit=2`, `fan_out_limit=50` |
| **MUST** | `relation_paths.len() <= 2` |
| **MUST** | 不 panic |

#### G1-L2 — `fan_out_limit` 每跳

| | |
|--|--|
| **预置** | Beta 出度 ≥ 5 |
| **请求** | 种子 `Beta`, `hop_limit=1`, `fan_out_limit=2`, `relation_limit=50` |
| **MUST** | `relation_paths.len() <= 2` |

#### G1-L3 — 同一边不因双向/重复 hop 重复计入

| | |
|--|--|
| **预置** | 仅 `R_AB` 一条边（可缩小夹具） |
| **请求** | `entity_names=["Alpha","Beta"]`, `hop_limit=2`, `relation_limit=50` |
| **MUST** | `R_AB` 只出现 **一次**（`relation_id` 去重） |

---

### 3.4 隔离与边界（MUST）

#### G1-I1 — owner 隔离

| | |
|--|--|
| **预置** | OWNER 写入 Graph-Chain；OTHER_OWNER **不** 写边 |
| **请求** | `owner_user_id=OTHER_OWNER`, 种子 `Alpha`, `hop_limit=2`, `doc_ids=None` 或 OTHER 的空文档 |
| **MUST** | 无 `R_AB` |

#### G1-I2 — doc_ids 过滤

| | |
|--|--|
| **预置** | DOC 上 Graph-Chain；OTHER_DOC 上另写 `Foo→Bar` |
| **请求** | `doc_ids=Some([DOC])`, 种子 `Alpha`, `hop_limit=2` |
| **MUST** | 仅 DOC 的边；不得出现 `Foo→Bar` |

#### G1-I3 — `doc_ids=Some([])`

| | |
|--|--|
| **MUST** | 立即空结果（与 Milvus/pgvector 现逻辑一致） |

#### G1-I4 — 未知种子

| | |
|--|--|
| **请求** | `entity_names=["NoSuchEntity"]`, `hop_limit=2` |
| **MUST** | 空 paths，不 error |

---

### 3.5 回归对照（SHOULD，有则更佳）

#### G1-P1 — 与现有 smoke 兼容

保留 `replace_search_and_graph_roundtrip` 行为：dense 命中 + hop=1 `Alpha→Beta`。  
G1 通过后可将 smoke 的 graph 部分 **委托** 给 G1-H1，避免双份维护。

#### G1-P2 — Milvus 旁路金标（可选，非 G1 阻塞）

同一 Fixture 在 `RETRIEVAL_BACKEND=milvus` 上跑 H1/H2，**集合断言相同**。  
用于证明「契约一致」；失败时分清是 fixture 问题还是后端偏差。  
**不**要求桌面跑 Milvus。

---

## 4. 断言辅助（实现约定）

```text
fn triple(p: &RelationPathCandidate) -> (String, String, String) {
    (p.subject.clone(), p.predicate.clone(), p.object.clone())
}

fn set_of(paths: &[RelationPathCandidate]) -> HashSet<(String,String,String)> { ... }

// 边方向：夹具规定方向；若未来改为无向存储，更新本规格而非放宽 assert
```

失败消息应打印：`hop_limit`, seeds, 实际 triples —— 便于对照大小写问题（G1-S2）。

---

## 5. G2 / G3 口径（本文件不展开用例，仅挂钩）

### 5.1 G2 — 编排冒烟（`rag-core` 或 app 级）— **implemented**

| 项 | 内容 |
|----|------|
| 测试 | `crates/rag-core/tests/vgrag_pgvector_g2.rs` |
| 环境 | 活 `DATABASE_URL` + pgvector；`RagRuntime` + `PgvectorDataPlane`（无 LLM chat） |
| 数据 | G1 同构链夹具（Alpha→Beta→Gamma→Delta） |
| 动作 | `graph_augment_from_terms(hops=2)`；产品 `fuse_vgrag_into_dense` |
| **MUST** | `relation_n > 0` **且** `graph_n > 0`；hop=2 含 Beta→Gamma |
| **实测** | `relation_n=3 graph_n=1 fused_len=1`（2026-08-04） |

```bash
cargo test -p avrag-rag-core --test vgrag_pgvector_g2 -- --nocapture
```

### 5.2 G3 — 质量抽样

| 项 | 内容 |
|----|------|
| 集 | graph81 子集（例如 10～20 题已知依赖图 hop 的题）或内部 multihop probe |
| 对比 | 同题 Milvus D1 基线 |
| 门槛 | 由产品定（例：子集准确率 ≥ 0.9× Milvus 或绝对 ≥ N）；**未定门槛前 G3 只出报告不出「通过」** |

---

## 6. 实现检查清单（PR 自检）

- [x] G1-H1, H2, H3, H0, H2-MID 全绿  
- [x] G1-S1, S3, S4, S5 全绿  
- [x] G1-S2 选项 **(a)** 修复 + 断言  
- [x] G1-L1, L2, L3 全绿  
- [x] G1-I1, I2, I3, I4 全绿  
- [x] 非 `#[ignore]`；无 `DATABASE_URL`/schema 时 soft-skip（与 storage-pg 一致）  
- [x] 状态行 → `G1 implemented`  

**运行**:

```bash
cd avrag-rs && set -a && source .env && set +a
cargo test -p avrag-storage-pgvector --test graph_hop_g1 -- --nocapture
```

---

## 7. 明确非目标（G1 不做）

- 真实 embedding 模型质量  
- 中文分词 / BM25 vs Milvus sparse 对齐  
- 桌面 Docker 拆除、便携 PG 打包  
- 修改云端默认 `RETRIEVAL_BACKEND=milvus`  
- graph_passages ANN 通道（表可空；hop 不依赖它）

---

## 8. 建议落地顺序

```text
1) ✅ graph_hop_g1.rs + S2 大小写修复
2) ✅ G2 vgrag_pgvector_g2.rs（augment hop2 + fuse_vgrag）
3) ✅ 桌面栈瘦身：无 Milvus；RETRIEVAL_BACKEND=pgvector
4) ✅ 去 Docker 默认路径（2026-08-04）：STACK_MODE=auto → native
     pg_ctl + redis-server；Docker 仅回退
5) G3 harness：`scripts/run-graph81-pgvector-g3.sh`（slice dry 已跑；全量需 nightly LLM）
   离线门禁 G1+G2 已过 → 可装客户端，质量对标云端仍待 online G3
6) ✅ Rust native ensure（`desktop/.../native_stack.rs`，无 bash 优先路径）
7) Windows 便携 PG/Redis 二进制捆绑（仍依赖本机安装系统包时的缺口）
```

**一句话**: G1 用固定链 `Alpha→Beta→Gamma→Delta` 证明 **hop=1/2/3 集合语义 + 种子/隔离/截断**；其中 **H2 是 VGRAG 桌面化的硬门槛**；**S2 大小写** 是已知实现风险，必须在 G1 内显式处理。
