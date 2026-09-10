# 仓库树语义图：20题增量实验方案

日期：2026-09-10。状态：**方案和题集已编排，离线来源核对通过；四路执行适配与在线评测尚未启动。** 用户本轮要求先确定测试集、方法和对照。

完整方案维护在独立原型，避免两套执行约定漂移：

- [实验方法与对照](C:/Users/xingc/Documents/Codex/repository-tree/eval/SEMANTIC-GRAPH-EXPERIMENT.md)
- [20题完整题干、判据与来源](C:/Users/xingc/Documents/Codex/repository-tree/eval/SEMANTIC-GRAPH-20.md)
- [机器题集](C:/Users/xingc/Documents/Codex/repository-tree/eval/semantic-graph-20.json)
- [离线核对回执](C:/Users/xingc/Documents/Codex/repository-tree/evidence/semantic-graph20-design-validation.json)

## 核心设计

固定10份文件、872块与在线BGE-M3向量。图采用已实现的章节窗口近邻图：16块窗口、top-5、0.60阈值、无向并集；不根据本轮问题重切块或调参。

| 组 | 共有grep、原文树和连续阅读 | 在线query向量混合检索 | 语义图 |
|---|---|---|---|
| A grep-tree | 有 | 无 | 无 |
| B grep-tree-graph | 有 | 无 | 有 |
| C hybrid-tree | 有 | 有 | 无 |
| D hybrid-tree-graph | 有 | 有 | 有 |

主比较D−C，次比较B−A。B使用embedding派生图，不能称为无向量的纯grep。无图组必须同时移除图工具、附带图信息和实际图计算，不能只隐藏入口。所有组共用短引用与工具循环，重新生成答案；题干不要求使用图。

20题分为6题寻找其他来源、4题已知材料比较、2题三文档综合、4题基础回归、4题证据边界。6题逐字沿用原q100/q001/q022/q080/q112/q108，另外14题依原文编写。重点包括较完整版本补齐、4R记载年份不一致、重复乐旋案例不能当独立证据，以及范围内材料不足。

全量20题×4组×3次＝240答案；Qwen3.8 Flash，每路并发2、总8，不设token预算上限。先G02/G18四组8答案技术小批，配置不变时计入总数；质量低分或模型未用图不触发换题。在线阶段初估45–120分钟，以小批修正。

主指标为G01–G12的有据必要点覆盖，同时报告全部20题严格任务成功、引用、越界、无依据断言、tokens、延迟和图实际使用。逐点评分覆盖240答案，原生Eval v2只给6个沿用题的72答案另列桥接成绩；预定72答案去组名复核。按题配对，并报告主题相关性的局限。

## 本轮已完成的门

题集唯一性、原题干一致性、131条参考块位置、来源文件哈希、范围以及批次数量均已离线核对，在线调用0。来源主索引SHA-256仍为 `34d7756c267f23facf91a7c8c1e6a20ce5abfd0adb47794e21f5d7df4ba956c9`。

尚需图消融适配、实际prompt/schema和运行指纹冻结、隔离评分适配核对及在线技术小批。这一状态不等于新评测已经通过，也不改变[历史20题结果](2026-09-10-repository-tree-controlled-experiment.md)。图实现见[原型交付记录](2026-09-10-repository-tree-prototype.md)。
