# 调研：context-helper 命名候选与查重

**日期：** 2026-09-06 · **性质：** 时间点调研快照 · **服务对象：** context-helper PRD §12-O4

**产品：** 桌面 AI Agent 的「目录层」插件。指定项目目录后静默保持索引、维护 AGENTS.md 约定（纠正写回）、经 MCP 向 Agent 暴露分层检索工具。人几乎不进 UI。代号 context-helper（太泛）。母产品家族 Context-OS / ContextLM；受众中英文开发者。禁用 KB/知识库框架。

## 命名最佳实践（简述）

dev 工具命名共识：短、可拼读、可搜索、隐喻驱动（[Smol Launch](https://smollaunch.com/guides/app-name-ideas)、[Simple Thread](https://www.simplethread.com/taming-names-in-software-development/)）。本轮证实「context/memory/atlas」类词已严重拥堵——understory、rhizome、hypha、waymark、cartographer 在 MCP/agent 生态均已被用作同类产品名；「安静底层/菌丝」恰是 2025–26 AI 工具热门取名方向。

## 候选表

| 候选 | 中文联想 | 判定 | 证据 |
|---|---|---|---|
| **Subsoil** | 底土层 | ✅ **CLEAR** | 仅有土壤学论文，无软件项目冲突（[检索](https://bsssjournals.onlinelibrary.wiley.com/doi/10.1111/ejss.70150)） |
| **Humus** | 腐殖层 | ✅ CLEAR（有保留） | 仅 NeurIPS [HUMUS-Net](https://github.com/z-fabian/HUMUS-Net) MRI 模型 |
| **Vestibule** | 门厅 | 🟡 基本 CLEAR | 仅冷门 [AWS bridge 仓库](https://github.com/rym002/vestibule-bridge-gateway-aws)；拼写/发音对中国用户偏难，有解剖学含义（耳前庭） |
| Understory | 林下层 | ❌ TAKEN | [thecodacus/understory](https://github.com/thecodacus/understory)：「memory for AI agents, MCP」同类产品直接撞车；另有 Obsidian 插件 |
| Mycel | 菌丝 | ❌ TAKEN | [heurema/mycel](https://github.com/heurema/mycel)、[mycel-labs](https://github.com/mycel-labs/mycel) 区块链 |
| Hypha | 菌丝体 | ❌ TAKEN | [CodeSoul-co/Hypha](https://github.com/CodeSoul-co/Hypha)：441⭐ agent 框架 |
| Rhizome | 根茎 | ❌ TAKEN | [rhizome-mcp](https://glama.ai/mcp/servers/Odrin/rhizome-mcp)、[Rhizome-Project](https://github.com/Rhizome-Project/rhizome-runtime) |
| Loam | 沃土 | ❌ TAKEN | [loambuild/loam](https://github.com/loambuild/loam) 智能合约 SDK、azavea/loam GIS |
| Tendril | 卷须 | 🟠 CROWDED | [serverless-dna/tendril](https://github.com/serverless-dna/tendril)（Tauri+agent 桌面工具，形态高度相似） |
| Cartographer | 制图师 | 🟠 CROWDED | Google Cartographer、[code-cartographer MCP](https://lobehub.com/pl/mcp/mark-satterfield-code-cartographer) |
| Archivist | 档案员 | 🟠 CROWDED | [@stellarwp/archivist](https://www.npmjs.com/package/%40stellarwp%2Farchivist) |
| Sidecar | 边车 | 🟠 CROWDED | [jrenaldi79/sidecar](https://github.com/jrenaldi79/sidecar)（Claude Code 伴侣 UI）、Tauri 官方 sidecar 概念 |
| Trailhead | 登山口 | ❌ TAKEN | Salesforce Trailhead + [@trailhead/cli](https://www.npmjs.com/package/@trailhead/cli) |
| Waymark | 路标 | ❌ TAKEN | [shaifulshabuj/waymark](https://github.com/shaifulshabuj/waymark)、waymark MCP |
| Foyer | 门厅 | 🟠 CROWDED | [foyer-rs](https://github.com/foyer-rs/.github)（知名 Rust 缓存库） |
| Bedrock | 基岩 | ❌ TAKEN | AWS Bedrock |
| Atlas | 地图集 | ❌ TAKEN | [MCP-Atlas benchmark](https://arxiv.org/html/2602.00933v3)、MongoDB Atlas |
| Rootstock | 砧木 | ❌ TAKEN | [Rootstock 区块链](https://dev.rootstock.io/developers/quickstart/hardhat/) |
| Duff | 枯枝层 | ❌ 避免 | homebrew duff 查重工具；英式俚语 duff=「坏掉的」、"up the duff"=怀孕 |

## TOP 3 推荐

1. **Subsoil（底土）** — 唯一完全干净的「安静底层」词。语义精准：表层之下、默默滋养、无人注视。中英发音都简单，中文可直接叫「底土」。无负面含义。
2. **Humus（腐殖层）** — 隐喻最贴：枯落物转化为养分，恰如插件把项目沉淀物转化为 agent 可用性。保留：与 hummus（鹰嘴豆泥）同音；「腐」字中文略负面，中文品牌建议用「腐殖层」或另起中文名。
3. **Vestibule（门厅）** — 「agent 进门第一站」隐喻独特干净，但拼写长、发音难，仅备选；若选建议配极简 CLI 别名（如 `vest`）。

**总体判断**：菌丝/根茎家族已被抢注一空；「安静的地下层」方向里 Subsoil 是最安全的空位。定名前需在 npm、crates、GitHub 与 `subsoil.dev` 等做最终可用性实测。


---

# 第二轮（2026-09-06）：拉丁 / 法语 / 中文词根

**新方向**：Subsoil 被判太直白。要求：拉丁、法语、中文词根；概念接近「安静底层」但更泛化（不局限于土壤）；更好听。

## 候选表

| 候选 | 含义/联想 | 发音·输入 | 判定 | 证据与备注 |
|---|---|---|---|---|
| Subtext | 字面之下的意义 | 中英皆易 | ❌ TAKEN | [fullstorydev/subtext](https://github.com/fullstorydev/subtext)：FullStory agentic session review，直连 Claude Code/Cursor 且自带 Subtext MCP——同赛道直接撞车 |
| Coulisse（法·舞台侧翼） | 「en coulisse」幕后 | 中文用户拼写/发音难（ku-LISS） | 🟡 CLEAR 有保留 | 软件圈仅 [hamradio 仓库](https://github.com/coulisse/spiderweb/)；荷兰窗帘公司 Coulisse B.V. 活跃于 [homebridge 生态](https://github.com/TedTolboom/com.coulisse.motionblinds) |
| Envers（法·背面） | 「l'envers du décor」 | 中等 | ❌ TAKEN | 开发者心智 = [Hibernate Envers](https://github.com/GEDOPLAN/hibernate-envers)（知名 Java 审计框架） |
| Solum（拉丁·地基） | 根基 | 中英皆易 | 🟠 CROWDED | [clariusdev/solum](https://github.com/clariusdev/solum) 医疗影像；另有华人 [SOLUM](https://tannerlab.cn/) local-first AI 助手（Rust+记忆账本），方向太近 |
| Sous-sol（法·地下室） | 地下层 | 连字符劝退 CLI | 🟡 不推荐 | 无软件冲突，但对法语用户就是「地下室」，等于换皮 Subsoil |
| Tréfonds（法·最深处） | 事物最里层 | tray-FON 中英用户都不会读 | ✅ CLEAR | 无软件结果；é 输入是负担 |
| **Latebra**（拉丁·隐匿处） | 悄然藏在暗处之物 | la-TEB-ra，中英可读 | ✅ **CLEAR** | GitHub 无同名项目；动物学术语（鸟卵黄心系），词根偏 hidden 略 secretive |
| Sedes（拉丁·居所） | 席位/驻地 | 易 | 🟡 基本 CLEAR | 西语常用词（分校）与以太坊 [sedes 术语](https://github.com/q9f/eth.rb)；听写歧义（seeds/said's） |
| Dessous（法·下面） | 底部 | 中等 | ❌ 避免 | 法语「le dessous」常指**内衣**，尴尬义确凿 |
| Assise（法·基座） | 地基 | 中等 | ❌ TAKEN | [Assise 分布式文件系统](https://mcanini.github.io/papers/assise.osdi20.pdf)（OSDI'20）直接撞车；assises=法国重罪法庭 |
| 润物 Runwu | 杜甫「润物细无声」 | 拼音输入自然；英语者 run-wu 尚可 | 🟠 CROWDED | 中文公司高频名（[潤物控股](https://www1.hkexnews.hk/listedco/listconews/sehk/2022/1229/2022122900013.pdf) 等），辨识度被稀释 |
| **伏流 Fuliu** | 地下潜行之河 | 拼音极佳；英语者对 liu 会卡 | ✅ **CLEAR**（软件圈） | 仅针灸穴位「复溜」同音，无软件冲突；诗意与「静默输送」隐喻俱佳 |
| 潜流 Qianliu | undercurrent | qian 对英语者极难 | 🟠 CROWDED | [潜流 AI Business Studio](https://www.code89757.com/)、爱奇艺《潜流》节目 |
| 底蕴 Diyun | 积淀的内里厚度 | yun 对英语者难 | 🟡 轻占用 | 词偏褒义泛用，缺画面感 |
| **Subtex**（自荐·拉丁 subtexere 在下方编织） | 底层织网者 | 中英皆易、短 | ✅ CLEAR | 仅一个 LaTeX 类语言小仓库；注意会被读作 sub-TeX |

## TOP 3

1. **伏流 Fuliu** — 中文名天然成立且极美：地下河静默奔流、偶尔涌出（恰如插件 silent indexing、需要时才出现）；拼音输入零成本，软件圈零冲突。代价：英语用户读不准 liu，与穴位「复溜」同音（无害）。适合双语品牌 Fuliu / 伏流。
2. **Latebra** — 拉丁词，音律好（la-TEB-ra），中英可读可拼，语义「安静藏在暗处、使一切运转」精准，GitHub/npm 全空。代价：略生僻，「隐藏」义需品牌叙事化解成「幕后」。
3. **Subtex** — subtexere（在下方编织）截短造词：短、ASCII、CLI 友好、「底层+文本+编织语境」三重暗示，与 Context-OS 家族气质契合。代价：可能被误读 sub-TeX，需一行 tagline 固定语义。

**淘汰提示**：Subtext/Envers/Assise 在 agent、Java、存储三方向均撞名；Dessous（内衣义）与 Sous-sol（换皮 Subsoil）划掉。定名前在 npm/crates/GitHub 及 `fuliu.dev`、`latebra.dev` 做最终可用性实测。
