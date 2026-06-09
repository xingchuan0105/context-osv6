# 检索策略选择

1. **dense_search**：语义相似度检索，适合概念性问题
2. **lexical_search**：精确关键词匹配，适合特定术语
3. **graph_search**：实体关系检索，适合关联分析
4. 默认先用 `dense_search`，召回不足时补充 `lexical_search`
5. 多文档问题先调 `doc_summary` 获取概览（见 `doc-summary.md`）
