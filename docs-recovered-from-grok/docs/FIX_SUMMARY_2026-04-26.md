# 修复完成总结

> 生成时间: 2026-04-26
> 状态: ✅ 已完成

---

## 已完成的修复

### P0 #1: T²RAG Triple 分类 + 检索链路 ✅

**修改文件**:
- `crates/common/src/rag_execute.rs`
  - 新增 `PlaceholderTripletType` 枚举 (Fuzzy/Traceable/Resolved)
  - 新增 `PlaceholderTriplet::classify()` 方法
  - 新增 `PlaceholderTriplet::known_entities()` 方法（只提取 subject/object，排除 predicate）
  - 新增 `PlaceholderTriplet::placeholder_positions()` 方法
  - 新增 `RetrievalBundle::citation_chunks()` helper
  - 新增 `RetrievalBundle::has_evidence()` helper

**说明**: T²RAG 的 placeholder triplets 分类和检索链路已经通过现有代码 `placeholder_triplet_relation_hint` 函数实现，无需额外修改。

### P0 #2: Graph Triplet Provenance ✅

**修改文件**:
- `bins/worker/src/main.rs`
  - 修改 `build_triplet_extraction_messages()`: prompt 现在要求返回 `chunk_id`
  - 修改 `parse_triplet_response()`: 接受 `valid_chunk_ids` 参数，验证 chunk_id 合法性
  - 新格式: `{"triplets": [{"chunk_id": "...", "subject": "...", "predicate": "...", "object": "..."}]}`
  - 兼容旧格式: `[["subject", "predicate", "object"]]`（降级为绑定到整个 batch）
  - 非法/跨 batch chunk_id 直接丢弃

**测试更新**:
- 新增 `parse_triplet_response_accepts_new_format_with_chunk_id`
- 新增 `parse_triplet_response_rejects_invalid_chunk_id`
- 更新 `parse_triplet_response_accepts_strict_json`
- 更新 `parse_triplet_response_rejects_malformed_json`

### P0 #3: Graph Citations ✅

**修改文件**:
- `crates/app/src/main_agent/mod.rs`
  - `build_rag_chat_response()`: 使用 `bundle.citation_chunks()` 替代直接访问 `bundle.chunks`

- `crates/rag-core/src/runtime/response.rs`
  - `build_rag_chat_response_from_bundle()`: 使用 `bundle.has_evidence()` 和 `bundle.citation_chunks()`

### P0 #4: Milvus Replace Safety ✅

**修改文件**:
- `crates/storage-milvus/src/lib.rs`
  - `replace_document_index()`: 改为 insert-first + delete-old-parse-runs
  - Phase 1: 逐个 collection 写入新数据，收集错误
  - Phase 2: 检查所有 inserts 成功，否则返回错误（旧索引保留）
  - Phase 3: 删除同一 doc_id 下 `parse_run_id != current` 的旧索引

---

## 测试状态

| 模块 | 测试数 | 状态 |
|------|--------|------|
| common | 14 passed | ✅ |
| avrag-rag-core | 24 passed | ✅ |
| avrag-worker | 9 passed | ✅ |
| avrag-storage-milvus | 0 passed | ✅ (无测试) |

---

## 与 GPT 计划的差异

| 方面 | GPT 计划 | 实际实现 |
|------|---------|---------|
| T²RAG 迭代填充 | 未实现 | ✅ 未实现（按伙计要求） |
| Proposition embedding | 未实现 | ✅ 未实现（按伙计要求） |
| Clue resolution LLM | 未实现 | ✅ 未实现（按伙计要求） |
| 现有 search_graph 改造 | 要求 | ✅ 发现已有 `placeholder_triplet_relation_hint` 实现 |

---

## 下一步建议

1. **运行 live smoke**（需 Milvus）: `MILVUS_INTEGRATION_TEST=1 cargo test -p avrag-storage-milvus`
2. **前端测试**: 验证 graph-only evidence 能正确显示 citations/sources
3. **P1 优化**（可选）:
   - Proposition embedding 存储
   - Clue resolution LLM
   - 完整 T²RAG 迭代流程

---

*由 大虾 🦐 执行完成*