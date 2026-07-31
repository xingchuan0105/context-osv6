# struct-query 数值规整（finetype）交接：探针否决 + 项关闭（2026-07-31）

> 上游：`docs/plans/2026-07-31-struct-query-fts-handoff.md`（fts 收官）← `2026-07-31-struct-query-p2-handoff.md`（P2 四项队列）。
> 本文档 = P2 最后一项「数值规整」的终局：finetype 探针三路验证（D4）→ 确定性规整实施后**用户拍板撤销**，项关闭不做。P2 四项至此全部收官。

## 0. 一句话现状

数值规整项**关闭不做**：finetype 扩展探针证否（只读库 LOAD 即 abort），确定性硬编码规整被用户否决（万科括号负数当场破功 + 过度归整对 LLM 无用）；查询侧 LLM 自行用 SQL（TRY_CAST/strptime）理解原值即足。代码回退至 fts 窗口基线，12/12 struct_query + 31 struct-supervision 全绿；老库（ipd/白药）已重灌获得 FTS 索引（fts 残留 #3 消解）。

## 1. D4 探针结论（`crates/struct-supervision/examples/finetype_probe.rs`，保留作否决证据）

三路验证结果：

| 路线 | 结果 |
|---|---|
| R1 bundled 内建 | **否**：`ft_version` 不存在（finetype 是社区扩展，[meridian-online/finetype](https://duckdb.org/community_extensions/extensions/finetype.html)，非 core 内建，与 fts 处境不同） |
| R2 社区扩展 | 可写库 `INSTALL finetype FROM community` + LOAD 可用（ft_version 0.6.36，ft_infer/ft_cast 正常）；**但只读库上 LOAD 被双重否决**：① init 需 CREATE 注册 table macro → READ_ONLY 库拒 CREATE → 扩展 init **非 unwinding panic → 进程 abort（exit 134，不可 catch）**；② 加固连接（`enable_external_access=false`）直接 `Permission Error: Loading external extensions is disabled through configuration` 拒载。查询侧根本不可用 |
| R3 无扩展纯 SQL | **全通**：去逗号/货币符号/% 后 TRY_CAST、strptime 解析 `YYYY-MM-DD`/`2024/1/5`/`2024年3月8日`、列级可转比例判定，均在加固只读连接内可用；`read_csv` 仍被拦 |

R3 证明了另一件事：**查询侧 LLM 自己就有足够的规整手段**（TRY_CAST/strptime 不需要任何扩展、不需要灌入侧预处理）。

## 2. 用户拍板（设计 steer，三条）

确定性影子列规整（`<col>__num`/`__date`，本窗口曾实施到 Rust+Python 双侧冒烟通过）被**撤销**，理由：

1. **硬编码脆弱**：多数表格没有共同规律格式，编码一种格式遇到其他格式照样破功——实证：万科 2024 年报资产负债表括号负数 `(1,291,800,290.12)`（库存股/其他综合收益）不被去逗号/去¥正则覆盖，90% 阈值判定同表两列（2024/2023）一过一不过，影子列残缺。逐格式加正则是无底洞。
2. **归整范式归 supervision loop**：agent loop 监督的设计意图就是让 agent 参与归整——读原文/读解析结果、发现错误、用指令或 toolcall 让解析器归整（逐列或批量脚本）、观察、决定停止。归整若发生，走既有指令契约（agent 提议 → 确定性代码应用 → SQL 复验），是**指令目录扩展**，不是灌入侧硬编码启发式。
3. **不过度归整**：表头/表格内容不需要过度归整——agent 查询时通过 SQL 语法获取内容并自行理解；过度归整对人类有用，对 LLM 用处不大。

→ 数值规整项关闭。将来若某张表确需规整，按第 2 条扩 supervision 指令目录另议。

## 3. 撤销清单（均已回退/删除，未入库）

| 内容 | 处置 |
|---|---|
| `crates/struct-supervision/src/normalize.rs`（在途，含 NameError 的 `pipeline.py` 规整机器同款 Rust 实现） | 删除 |
| `pipeline.py` 规整影子列机器（在途 diff，含 `detect_normalized`/`num_cell_to_double` 等；其 `write_duckdb` 内 `i` 未定义 NameError 证明从未跑通过） | `git checkout` 回退 |
| `store.rs` 影子列写入 + 测试（本窗口） | `git checkout` 回退 |
| `struct_query.rs` catalog `normalized` 标注 + `Date32` 渲染 + 测试（本窗口） | `git checkout` 回退 |
| SKILL.md `normalized` 条目（本窗口） | 删除 |

## 4. 保留产出（随本窗口入库）

| commit | 内容 |
|---|---|
| 本窗口 | `crates/struct-supervision/examples/finetype_probe.rs`（D4 三路探针，含「已知致命放最后」的 abort 复现）、`prompts/clusters/knowledge-base/SKILL.md` 增补 **fts 语法条**（`fts: true` → `match_bm25` 谓词用法/中文单 token 边界/`fts: false` 报错语义；对应 fts 窗口已提交的 catalog `fts` 标注，此前 skill 无此条，是 fts 残留 #1「谓词从未被真实 LLM 触发」的成因之一） |

## 5. 重灌与验证证据

- **重灌（回退版 pipeline.py，仅 fts 无影子列）**：ipd（693eb189…）1 表 + fts schema，`match_bm25('LPDT')` 命中 47/370 行；白药（b38a4960…）9 表 + 9 fts schemas。两库 `_meta.evidence_chunk_id` 变更 → `load_evidence_chunks.py` 重载 PG：10 evidence chunks（幂等先删后插）。**fts 残留 #3（老库无索引）消解**。
- **测试 gate（回到 fts 基线）**：`cargo test -p avrag-rag-core --lib struct_query` 12/12；`cargo test -p avrag-struct-supervision` 31（18+13）全绿。

## 6. 残留与后续窗口（更新自 fts 交接 §5）

P2 四项（telemetry / supervision 工具化 / fts / 数值规整）**全部收官**。剩余路线：

| 序 | 窗口 | 内容 | 依赖 |
|---|---|---|---|
| 1 | 观察 | 真实 LLM 切片带 FTS 关键词题，观察 `tool_trace` 是否出现 `match_bm25`（fts 残留 #1；SKILL.md 语法条已补，触发条件改善）；telemetry 延续——repair/violation 触发率、预算 28K/12 轮调优数据积累 | — |
| 2 | S4 挂接 | ingestion worker 表格提取阶段 Rust 化 → pipeline 产品化挂接（worker 现为通用任务队列，无表格阶段；P2 文档 S4 ◐） | 产品化主线 |
| 3 | A5 补测 | 监督干预成功案例：构造「表头被吃」样本 → rotate_header → 复验过（主计划 §11 A5 ⬜） | 独立，小 |
| 4 | 提取鲁棒性 | 万科双栏面板行混入 + PDF 跨页/分栏（主计划附录 C 已知限制） | 独立 |
| 5 | 中文 fts | jieba 预切 / 灌入侧影子 token 列（D3 物理现实；优先级低——grep 已覆盖子串） | 独立 |
| 6 | 行级证据 | 切块行号元数据 → 行级 `__src_line → chunk_id` 映射（附录 C 另案） | 依赖 chunker 埋点 |
| — | （关闭） | ~~数值规整~~：本窗口关闭；若重启走 supervision 指令目录扩展（agent 提议→代码应用→SQL 复验），禁灌入侧硬编码启发式 | §2 |
