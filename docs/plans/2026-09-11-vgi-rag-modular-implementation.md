# VGI-RAG 模块化原型实施记录

更新：2026-09-11，承接 [完成度审计](2026-09-11-vgi-rag-modular-prototype-audit.md)。本次完成独立工程的模块化代码与本地验证；尚未完成真实语料重建、新一轮在线评测和质量/规模性能验收。

## 当前事实与入口

独立工程：`C:\Users\xingc\Documents\Codex\repository-tree`。合同、算法、参数、预设、CLI、接口及边界集中在 [MODULAR-RETRIEVAL.md](C:/Users/xingc/Documents/Codex/repository-tree/docs/MODULAR-RETRIEVAL.md)；最终测试数量、时间和源文件哈希见 [验证回执](C:/Users/xingc/Documents/Codex/repository-tree/evidence/modular-retrieval-validation.json)。

服务/公开 MCP 与 HardAgent 共享 `retrieval/` 产品核心。BM25S 已进入运行依赖；在线 ChatClient 和用量账本抽到产品公共模块，评测角色/裁判配置仍在 evaluation。原文 rg 与规范文本映射从评测目录移到公共 SourceCorpus。旧子串排序仅留在历史评测目录。

## 审计门槛关闭情况

| Gate | 本次交付 | 验证边界 |
|---|---|---|
| G0 | 不可变配置、通道权重、异步 Proposal/模块状态、有效组合校验、完整 A–F 预设 | 可执行合同已落地；related/jump 分开贡献，默认权重未由新保留集验证 |
| G1 | 统一发现/扩张/融合；基础分数复用；limit 到 CLI、schema、执行和回执；公共 rg | HTTP/MCP 实际调用与服务/Agent 同序同分测试通过；E 是 narrow=false 的平铺混合 |
| G2 | 确定性文档向量主题树、overview/zoom；章节开关独立；材料化保留路径/URL/标题/偏移 | 合成输入验证通过；已有 248 文档索引未重建，不能宣称历史材料已经修复 |
| G3 | 跨文档向量边实际遍历、TF-IDF 文档关系、URL/目录/符号/日期关联、related 融合 | 合成跨文档扩张、贡献和 scope 验证通过；实际检索增益待测 |
| G4 | 在线抽取适配、逐字 quote/object 校验、已访问文档去重、BM25 jump、SQLite 版本缓存 | 使用模拟 HTTP 的真实管线测试通过；未调用线上模型验证抽取质量/延迟 |
| G5 | 完整六模块装配、公共 API/MCP、真实 rg、trial 配置和失败标记、相关回归 | 本地部分完成；真实在线六组/保留集与规模性能验证待执行 |

新 A 的六模块为 sections、topic_tree、semantic_graph、tfidf_graph、structure、route_b；B 关闭三个图源但保留普通向量、树和路线 B；C 关闭章节与主题树；D 保留桌面 Agent 的 grep/read；E 全库直接混合；F 仍是独立产线 SAC。所有新 trial 保存完整生效配置，不能只靠 A/B/C 字母与历史实验直接比较。

## 已处理的可靠性边界

- 候选、模块输入、缓存和模型可见结果遵守 scope，runner 再独立审计；模块失败不会当作合法空检索结果。
- 回读验证原始来源和规范文本哈希；rg 使用文件批次、总期限和输出字节上限。
- 来源路径、offset 与 source_id 保留；新材料化输入元数据变化时拒绝复用旧索引目录。
- 路线 B 缓存限制在相同查询、原文、索引/模型/提示版本和范围内；对象出现在种子文档中不再被错误过滤。
- 图与跳转通道独立融合；同一候选每模块仅保留最强路径，避免重复种子扩大 Agent 上下文。
- 新增提示/skill 原稿在 `avrag-rs/prompts/vgi-rag/`；独立工程用同步脚本打包并校验原稿。

## 下一阶段

先用新目录重建真实材料与向量，验证结构恢复、在线抽取成功率和耗时；再冻结 A–F 配置，在固定题集/并发下做 paired repeats、盲评和保留集比较。正确性、完整性、引用、基础设施失败、请求/token、墙钟和内存分别报告。此类在线/较长运行应另按仓库耗时约定确认范围；本次没有重启现有服务、调用线上题集或覆盖旧评测产物。
