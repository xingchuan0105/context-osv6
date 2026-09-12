# VGI-RAG Challenge-20 题集交接

更新：2026-09-12。用户认为当前 20 题过于简单，要求寻找更难的问题。题目候选已整理；随后已下载完整 BrowseComp-Plus 语料并写入独立原型的本地索引，**没有**使用 F 组原生 SAC 灌库，也没有在线向量化或新评测。

## 交付

- [完整方案与 20 题摘要](C:/Users/xingc/Documents/Codex/repository-tree/eval/CHALLENGE20.md)
- [冻结的 ID、来源版本与哈希清单](C:/Users/xingc/Documents/Codex/repository-tree/eval/challenge20-selection.json)
- [本地准备回执](C:/Users/xingc/Documents/Codex/repository-tree/evidence/challenge20-preparation.json)
- [全库下载与本地索引回执](C:/Users/xingc/Documents/Codex/repository-tree/evidence/challenge20-corpus.json)
- [20 道英文原题](C:/Users/xingc/Documents/Codex/repository-tree/.eval/hard/challenge20-v1/evaluation-only/QUESTIONS.md)

20 道主集来自 BrowseComp-Plus，每题有 9–13 份标注证据、1–2 份包含答案的标注文档。选择关注间接线索、历史时间截面、跨作品/论文/机构关系、具体数值与原文细节。另准备 8 道 BRIGHT 纯检索诊断，不把其相关性分数与主集答案正确率混合。

本轮选择没有调用模型，也没有根据 VGI 结果挑题。新 20 题与旧 60 题的已知证据家族连通分量隔离；旧 60 题内容未改、40 道保留题未运行。新集合属于经过问题审阅的开发挑战集，难度和逐条证据尚未验证。

BrowseComp-Plus 七个分片（1,761,580,486 字节，修订 `b27b02bc3e45511b8b82a13e6f90ce761df726f6`）已下载到独立工程 `.eval/hard/challenge20-v1/shards/`。本地 HardRepository 索引在 `.eval/hard/challenge20-index-v1/`：100,195 文档、18,437,321 块、无 embedding。20 题标注证据 ID 均在库中。入口：`scripts/materialize-challenge20-corpus.py`。BRIGHT 全域候选集未下载。

## 与上一轮的关键区别

上一轮 248 份文档的选题材料库不能代表完整公共基准。新主集要求 BrowseComp-Plus 全部 100,195 份文档，不能只装载这 20 题的 207 份标注证据。完整压缩 Parquet 1.76 GB 已下载并完成本地词法索引；尚未在线向量化。先完成语料/证据审核及单次 BM25、dense、hybrid 检索校准，再冻结并进行 Agent 对照。现有 runner 会把全部块载入内存，18M 块上还不能直接开跑。

BRIGHT 诊断需要各域完整候选集，遵守 `excluded_ids`。原始 ID 中的题目主题词只供评估映射，不能变成可供 grep 的人为索引提示。

## 下一次评测前的修正

1. B 按用户原意只移除 `semantic_graph`，保留普通向量、其他图源、树与路线 B。上一轮 B 实际去掉三个图源，旧成绩不能解释为纯语义图贡献。
2. 裁判须接收 Agent 实际可见的合法标题、日期与出处；目前正文拼接会遗漏 `source_titles`。没有规范引用时，检索上下文支持度不能当作引用准确率。
3. 核验完整语料、文档/片段映射、在线向量与图、大库性能、文件分页；F 必须完成相同大库的 SAC 入库与范围绑定后才可比较。
4. 后续保留 qwen3.8-flash、在线 embedding、每组 2 / 总并发 8、不设总 token 预算偏好。题数仍为主 20 题；BRIGHT 是可选单独诊断。新规模的费用与运行范围需具体化后再执行。

本次下载并完成本地词法索引；没有修改产品提示、运行配置、裁判代码，也没有向 SAC 灌库或调用在线 embedding。详细事实、来源和待办以独立工程的 Challenge-20 方案为准。
