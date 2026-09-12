# VGI-RAG 模块化原型实施记录

更新：2026-09-12，承接 [完成度审计](2026-09-11-vgi-rag-modular-prototype-audit.md)。模块化代码、真实语料和在线向量重建、六组技术冒烟已完成；20 题在线对照的结果及恢复记录见 [本轮报告](C:/Users/xingc/Documents/Codex/repository-tree/eval/MODULAR-V2-DEV20-REPORT.md)。开发集结果与保留集、规模性能和人工验收分别管理。

## 当前事实与入口

独立工程：`C:\Users\xingc\Documents\Codex\repository-tree`。合同、算法、参数、预设、CLI、接口及边界集中在 [MODULAR-RETRIEVAL.md](C:/Users/xingc/Documents/Codex/repository-tree/docs/MODULAR-RETRIEVAL.md)；本轮测试、索引、在线冒烟和评测状态见 [验证回执](C:/Users/xingc/Documents/Codex/repository-tree/evidence/modular-v2-online-validation.json)。先前 `modular-retrieval-validation.json` 是模块初版记录。

服务/公开 MCP 与 HardAgent 共享 `retrieval/` 产品核心。BM25S 已进入运行依赖；在线 ChatClient 和用量账本抽到产品公共模块，评测角色/裁判配置仍在 evaluation。原文 rg 与规范文本映射从评测目录移到公共 SourceCorpus。旧子串排序仅留在历史评测目录。

## 审计门槛关闭情况

| Gate | 本次交付 | 验证边界 |
|---|---|---|
| G0 | 不可变配置、通道权重、异步 Proposal/模块状态、有效组合校验、完整 A–F 预设 | 可执行合同已落地；related/jump 分开贡献，默认权重未由新保留集验证 |
| G1 | 统一发现/扩张/融合；基础分数复用；limit 到 CLI、schema、执行和回执；公共 rg | HTTP/MCP 实际调用与服务/Agent 同序同分测试通过；E 是 narrow=false 的平铺混合 |
| G2 | 确定性文档向量主题树、overview/zoom；章节开关独立；材料化保留路径/URL/标题/偏移 | 已在新目录重建 248 文档 / 7,228 块；多标题文档从 12 增至 156，来源正文哈希保持一致；旧索引保留 |
| G3 | 跨文档向量边实际遍历、TF-IDF 文档关系、URL/目录/符号/日期关联、related 融合 | 合成跨文档扩张、贡献和 scope 验证通过；开发集已实测，未证明质量增益，保留集仍待验证 |
| G4 | 在线抽取适配、逐字 quote/object 校验、已访问文档去重、BM25 jump、SQLite 版本缓存 | 真实 qwen3.8-flash JSON 抽取及 Agent 冒烟通过；已修复串行超时、抽取思考模式和单个无据桥接导致整体失败的问题 |
| G5 | 完整六模块装配、公共 API/MCP、真实 rg、trial 配置和失败标记、相关回归 | 196 项全量本地回归通过；另有 2 项恢复选择测试。六组各 5 题技术冒烟通过，20 题在线对照与盲评另附逐项结果；保留集、规模性能及人工验收仍待执行 |

新 A 的六模块为 sections、topic_tree、semantic_graph、tfidf_graph、structure、route_b；B 关闭三个图源但保留普通向量、树和路线 B；C 关闭章节与主题树；D 保留桌面 Agent 的 grep/read；E 全库直接混合；F 仍是独立产线 SAC。所有新 trial 保存完整生效配置，不能只靠 A/B/C 字母与历史实验直接比较。

## 已处理的可靠性边界

- 候选、模块输入、缓存和模型可见结果遵守 scope，runner 再独立审计；模块失败不会当作合法空检索结果。
- 回读验证原始来源和规范文本哈希；rg 使用文件批次、总期限和输出字节上限。
- 来源路径、offset 与 source_id 保留；新材料化输入元数据变化时拒绝复用旧索引目录。
- 路线 B 缓存限制在相同查询、原文、索引/模型/提示版本和范围内；对象出现在种子文档中不再被错误过滤。
- 图与跳转通道独立融合；同一候选每模块仅保留最强路径，避免重复种子扩大 Agent 上下文。
- 新增提示/skill 原稿在 `avrag-rs/prompts/vgi-rag/`；独立工程用同步脚本打包并校验原稿。
- 路线 B 每次查询将最多 4 份文档合为一次抽取，每份最多 16,000 字符。独立抽取客户端关闭 thinking，温度 0，使用 JSON Object 模式，期限 120 秒；回答 Agent 的思考模式保持开启。
- 无据 quote/object 桥接逐条剔除，只有已验证桥接进入缓存和跳转；结构损坏、越界来源、缓存损坏仍属于完整性错误。抽取缓存按组、按运行隔离。
- 恢复分析只取每组每题首次有效执行，不按答案质量选择；保留全部原始请求、失败和未知用量。Windows 跨夜睡眠及凭证过期单独记录，不纳入性能成绩。

## 下一阶段

本轮在线范围已由用户连续授权：固定 20 道开发题，A–F 六组，每组 2 并发、总并发最多 8，qwen3.8-flash，不设总 token 预算。嵌入使用在线 Pro/BAAI/bge-m3；新生成 7,228 条向量、904 次请求。40 道保留题未运行。

最终 120 个唯一试验位置均已执行：118 条有效回答，C/E 各有 1 条超时；原生语义判分有效 117 条，A 有 1 条裁判协议错误。19 道共同可评分题上，A/B 正确性均分为 100、C/D/E 为 99.47、F 为 91.74（分数乘 100，并非严格成功率）。普通 grep 和平铺混合已接近饱和，尚不能据此证明图树优越性。SAC 实时元数据核对为 247 completed / 1 failed；失败论文不在本轮 QASPER 范围及金标中，但整体语料未全部就绪。

下一阶段以本轮语义正确性、证据覆盖、引用与稳定性问题为输入，先审查题集及常规检索的实际难度，再确定保留集与独立重复方案。B 当前消融全部三个文档图源，不等于仅消融 embedding 语义图；若要测其独立贡献，需另建配置并重新冻结实验。SAC 是完整系统对照，其段落证据映射、用量口径、服务端原文字节回读仍有限制。未进行服务重启或部署，未覆盖历史索引/答案，未宣称大语料性能达标。
