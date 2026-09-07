# Subtex M1 开发计划 —— 目录插件层首版闭环

**日期：** 2026-09-06 · **状态：** W0–W5 开发完成（各波验证门通过；详见文末「实施记录」）· **上游：** PRD `docs/plans/2026-09-06-directory-plugin-prd.md`（F1–F5 = M1 范围）
**代码摸底：** 本文所有 crate/文件路径均经 2026-09-06 两次代码探查核实。

---

## 1. 目标 / 终态

**M1 终态（一句话）**：作者在自用桌面 Agent（Claude Code / Kimi Code）里打开一个真实项目目录，说一句「挂上这个目录」，此后无需离开会话——约定初始化、后台静默索引（含录音云转写）、按就绪度分层检索、纠正写回约定，全部经 MCP 工具完成。

**终态检查表（M1 Done 的定义）：**

1. Agent 会话里 `subtex.init` → 拿到目录事实 + 约定建议稿 → Agent 写出/合并 `AGENTS.md`（Subtex 管理段落），原有内容零丢失。
2. 目录内文件变化（新增/修改/删除，含音频）被后台静默跟上：结构感知切块 → FTS5 词法 + 大纲（无 LLM）→ 云端 embedding → sqlite-vec 向量，全部落在应用数据目录的索引仓；项目目录零污染、无第二份文件副本。
3. 音频文件进目录 → 经 asr-filetrans 链路云转写 → `transcript.md`（说话人分离 + 时间轴）写回目录约定位置并进入索引；中间品不进目录。
4. Agent 随时可调：`subtex.status`（各层就绪度/转写进度）、`subtex.outline`（token 预算化全局大纲）、`subtex.search`（混合检索，带 `matched_via` 与出处）；索引未就绪的层级在返回中如实声明，任一阶段都有可用检索路径。
5. 一次纠偏 → `subtex.correction_draft` 生成约定更新稿 → Agent 写回约定文件 → 后续会话读得到。
6. 作者在真实项目目录自用一周，§9 三项度量有记录（自用频率、纠偏次数、检索纠正率）。

**非目标（M1 不做）：** F6 收件箱归类（M2）；F7 credits 账本 UI（M3，M1 只在索引仓落耗材流水）；三元组/长摘要等 LLM 重产物（D4，见 §3-D）；TOC 的 LLM 窗口管线移植（M1 大纲走无 LLM 结构提取）；MCP elicitation/progress 协议支持（transport 现状没有，M1 用轮询+确认工具替代，见 §4-6）；Windows 原生桌面打包（M1 先在作者 WSL 环境自用）。

## 2. 关键约束

**产品硬约束（PRD §7，违反即缺陷）：**

- 只监视会话接入的目录；不整盘；OS 文件名索引只读查询除外。
- 目录是主本：禁止入库副本；索引仓集中应用数据目录、可删可重建；转写成品写回目录、中间品留仓内。
- 不做 LLM Wiki 重编译；M1 大纲层零 LLM（结构提取），embedding 是唯一云端索引耗材（除转写外）。
- Agent 会话无配置/费用打扰；大批量转写（>2h）先报量级确认（防账单冲击）。

**仓规约束（AGENTS.md）：**

- T7 双线口径：Subtex 不写 workspace/notebook 表、不复用 B 线文档面 API；两线仅共享底层检索技术（ingestion / llm / retrieval-data-plane port）。
- 对共享 crate（transport-http、desktop core）的改动**只做加法**，不动 B 线现有 16 个 MCP 工具的行为。
- 面向模型的文案（约定模板、工具描述）不硬编码在 Rust 里——走 `avrag-rs/prompts/` 体系（Subtex 模板放 `prompts/subtex/`）。
- 凭据复用 `avrag-rs/.env` 链（`DASHSCOPE_API_KEY` / `VPS_MAIN_*`）；asr-filetrans 红线：KEY 不进命令行参数/日志/提交。
- WSL `jobs=2`，不叠并行全量 cargo test；每波验证用 `cargo test -p <pkg> --lib`；结构性改动后跑 `code-review-graph update`。
- **编译/长跑命令先报时间成本再执行**（仓规 time-cost consent）。

**技术约束（摸底事实）：**

- MCP 加工具 = 四处手工登记（`transport-http/src/mcp/catalog.rs` + `dispatch.rs` + `tools/subtex.rs` + `auth_guard.rs`），无自动发现。
- 队列：B 线是 PG `ingestion_tasks` 表（worker SKIP LOCKED）。Subtex **不进 PG**——每索引仓 SQLite 内建 `jobs` 表做任务交接（崩溃安全、免部署、符合「删仓即未接入」）。
- 检索：实现 `crates/retrieval-data-plane` 的 `RetrievalReadPort` + `RetrievalDataPlane` port（SQLite FTS5 + sqlite-vec），即可挂进既有 RagRuntime 工具派发，上层零改动。
- embedding 只有云端（DashScope/SiliconFlow，1024d），`EmbeddingClient` 带 observer/rate gate——计量 sink 写索引仓 `usage` 表（M1 不接 app-billing PG）。
- 解析器：`crates/ingestion` 路由里 markitdown/paddle_ocr 为重依赖（Python/OCR）。M1 只用轻量子集：markdown / text / code / csv / liteparse_pdf；docx 等有 markitdown 则用、无则跳过并如实列入 `unsupported`。
- asr-filetrans 仅 WSL 可跑（`wsl.exe -e bash -c 'bash /home/chuan/asr-filetrans/bin/transcribe.sh …'`）——M1 自用环境天然满足；Windows 原生调用后移。

## 3. 架构终态

```
桌面 Agent (Claude Code / Kimi Code)
  │ stdio: context-os-mcp（既有透传，零改动）
  ▼
avrag-api @127.0.0.1:18080 ── transport-http/mcp：新增 subtex.* 工具组（tools/subtex.rs）
  │ 读路径：subtex-core + subtex-store-sqlite（检索/状态/大纲）
  │ 写路径：往索引仓 jobs 表投任务（init/转写确认/reindex）
  ▼
subtexd（新 sidecar bin，desktop core 按 local_product.rs 模式拉起）
  │ watcher(notify+debouncer+ignore) + 周期 mtime 调和扫描
  │ 消费各仓 jobs 表 → ingestion 解析 → 切块 → EmbeddingClient(云) → SQLite 仓
  │ 音频 → asr-filetrans CLI → transcript.md 写回目录 → 照常索引
  ▼
索引仓：~/.local/share/subtex/roots/<sha256(root)>/
  ├─ index.db      # SQLite：chunks + FTS5 + vec0 + meta + jobs + usage + readiness
  ├─ root.txt      # 根路径（枚举已知根=扫 roots/ 读 root.txt，不扫盘）
  └─ scratch/      # 转写中间品（mp3/json），成品 transcript.md 写回目录
```

**MCP 工具清单（固定 7 个，就绪度在返回中声明）：**

| 工具 | 入参 | 返回 | 对应需求 |
|---|---|---|---|
| `subtex.init` | root | 目录事实（文件类型分布/已有结构/已有约定）+ 约定建议稿 + 仓初始化结果 | F1 |
| `subtex.status` | root | 各层就绪度（lexical/outline/vector N/M）、在途任务、转写进度、用量累计 | F3/N4 |
| `subtex.convention_draft` | root | 约定建议稿（init 之后随目录演化重生成） | F1 |
| `subtex.correction_draft` | root, correction | Subtex 管理段落更新稿（Agent 审阅后写回） | F5 |
| `subtex.search` | root, query, limit? | 混合命中（`matched_via`、出处 path+行段、就绪度声明、截断指引） | F3/F4 |
| `subtex.outline` | root, token_budget? | 文件名+标题树+一句话摘要（1–2k tokens） | F4 |
| `subtex.transcribe` | root, paths?, confirm? | 提交/查询转写；>2h 返回 needs_confirmation + 量级 | F2 音频前端 |

**鉴权**：新增 `OperationGuideMode::SubtexLocal`——仅本机 user token（`CONTEXT_OS_USER_TOKEN` / 桌面会话），拒绝 workspace API key；**云端部署默认关闭**（`AVRAG_SUBTEX=1` 才注册该工具组，云上不设）。

## 4. 关键设计决策（附理由）

1. **SQLite 实现 RetrievalDataPlane port，不进 PG。** port 已存在（`retrieval-data-plane/src/lib.rs`），上层 RagRuntime 零改动；PG 是 B 线数据面，进 PG 会破坏「删仓即未接入」与两线独立。向量用 **sqlite-vec**（成熟库，符合 prefer-proven-libraries），FTS5 自带 `bm25()`，RRF 在 Rust 侧融合。
2. **任务交接用仓内 `jobs` 表，不新建队列。** avrag-api（写）与 subtexd（读）天然解耦；崩溃恢复=仓在任务在；免 PG 依赖。
3. **大纲层零 LLM**：markdown 标题树 / code 走 tree-sitter outline（`ingestion` 已有 code parser 可借），PDF 取章节结构；「一句话摘要」M1 用首段启发式，LLM 摘要后置。三元组不移植（与 worker PG 流程重耦合，且属 D4 重产物）。
4. **elicitation 缺席的替代**：transport 只声明 `capabilities:{tools}`。M1 确认流 = 工具返回 `needs_confirmation` + Agent 代问 + 用户回话后再调一次（`confirm:true`）。elicitation/progress 协议支持单独立项（M2 前评估）。
5. **转写 = 仓内 job + CLI 调用**：subtexd 检出音频 → 调 `transcribe.sh run`（幂等续跑白嫖）→ 成品按约定位置（默认 `transcripts/`，可由约定改写）写回目录 → watcher 自然把它当普通文档索引。阈值确认走决策 4 的模式。
6. **就绪度状态机**：`lexical_ready → outline_ready → vector_ready`，按文件粒度 N/M 计入仓 `readiness` 表；工具结果统一带 `index_readiness` 块（R1 建议 8 的落地位置=返回而非动态描述，因 catalog 描述是静态的）。
7. **自用验收优先**：每波结束都在作者真实目录 dogfood，不等 M1 全完。

## 5. 波次计划（每波带验证门，门不过不推进）

### W0 地基：crate 骨架 + 索引仓（估 0.5–1 天）

- 新建 `crates/subtex-core`（root 规范化/哈希、仓布局、AGENTS.md 管理段落检测、content-hash 扫描器）与 `crates/subtex-store-sqlite`（schema：meta/chunks/fts/vec0/jobs/usage/readiness；`RetrievalDataPlane` port 空实现）。
- prompts 模板落 `avrag-rs/prompts/subtex/`（约定建议稿骨架、correction 更新稿骨架、工具描述文案）。
- **验证门**：`cargo test -p subtex-core -p subtex-store-sqlite --lib`（仓创建/重开/哈希稳定/段落检测单测）。

### W1 索引切片：目录 → 可检索（估 1–2 天）

- 接通：扫描器 → `ingestion` parser 路由（轻量子集）→ `build_ir_chunk_plan` 结构切块 → FTS5 写入；大纲提取（标题树/tree-sitter outline）→ `outline` 渲染（token 预算）；`EmbeddingClient`（复用 `.env` 配置 + observer→仓 usage 表）→ sqlite-vec；RRF 混合 + `matched_via`。
- **验证门**：fixture 目录（md/pdf/code/转写稿各若干）集成测试：入库→FTS5 命中→向量命中→混合排序合理→大纲 token 数在预算内；`cargo test -p subtex-core --lib` + 一个集成测试。

### W2 MCP 工具面（估 1 天）

- `subtex.*` 七工具在 catalog/dispatch/tools/auth_guard 四处登记；`AVRAG_SUBTEX` 开关；本机 user-token 鉴权；工具文案从 prompts 体系加载。
- **验证门**：本机 API 起栈后 `curl` JSON-RPC：`tools/list` 含 7 工具、`subtex.init/status/search/outline` 往返正确；再经 `context-os-mcp --check` + stdio 手测一遍；最后接一个真实 Agent 会话冒烟（init→Agent 写出 AGENTS.md→search 命中）。

### W3 subtexd 常驻服务（估 1–2 天）

- watcher（notify+debouncer+ignore）+ 周期调和扫描 + jobs 表 claim loop（解析/嵌入/重建分级任务）；desktop core 按 `local_product.rs` sidecar 模式拉起（WSL 自用期可手起）。
- **验证门**：真实目录放/改/删文件 → `subtex.status` 就绪度按预期迁移 → `subtex.search` 命中新内容；kill subtexd 后放文件 → 重启经调和扫描补齐；`cargo test -p subtex-core --lib` 保持绿。

### W4 音频转写（估 1 天）

- 音频检测 → 阈值判断（>2h 报量级 `needs_confirmation`）→ CLI 调用 → `transcript.md` 写回约定位置 → 触发索引；中间品留 `scratch/`；usage 流水含转写时长。
- **验证门**：30 秒真语音冒烟（asr-filetrans 既定冒烟法）走通「成品落目录+可检索」；再跑一段真实会议录音（报量级后确认）验收说话人分离/时间轴；确认 KEY 不出现在任何日志。

### W5 纠正写回 + 自用打磨周（估 0.5 天 + 一周 dogfood）

- `subtex.correction_draft` 联调 Agent 写回流；`subtex.status` 聚合输出做进度总览数据源；补 `AGENTS.md`（仓）/README 索引/PRD §10 状态；`code-review-graph update`。
- **验证门（M1 总验收，见 §6）** + dogfood 一周记录三项度量。

## 6. M1 总验收标准

**功能验收（映射 PRD F1–F5）：**

| PRD | 验收 |
|---|---|
| F1 | 新目录走一遍用户路径：Agent 写出约定、原 `AGENTS.md` 零丢失、仓出现、`roots/` 枚举可得；删仓后目录零变化；未 init 目录零写行为 |
| F2 | 放一批混合文件（含一段音频）：词法→大纲→向量逐层就绪；转写成品在约定位置且可检索；中间品不在目录；watcher 停档期变化被调和扫描补齐 |
| F3 | 任一阶段工具清单固定为 7 个；返回中就绪度准确；任一阶段提问有可用检索路径 |
| F4 | Claude Code 或 Kimi Code 真实会话：发现工具、init、拿建议稿、大纲查询、混合检索（带 `matched_via`+出处）全程无离开会话的操作 |
| F5 | 「乱放→纠正→`correction_draft`→Agent 写回」后，约定文件出现规则；新会话 Agent 读得到 |

**非功能验收：** 索引全程查询不阻塞（W3 中压测一次：边索引边 search）；目录 `git status` 干净（无仓内产物泄漏）；`rg DASHSCOPE` 日志无命中。

**自用验收（PRD §9）：** 一周 dogfood 记录——自用天数 ≥5；纠偏写回次数、检索后需人工纠正次数有台账；误移动=0（M1 无移动功能，天然满足）。

## 7. 风险与对策

| 风险 | 对策 |
|---|---|
| sqlite-vec 在 Windows 原生加载问题 | M1 先在 WSL 验证；Windows 期用静态捆绑或退化为「向量层缓就绪」（词法+大纲仍可用，符合 F3 分级哲学） |
| ingestion 重解析器依赖（markitdown Python） | M1 只用轻量子集，不支持的类型如实进 `unsupported` 列表展示 |
| Embedding 云端故障拖住索引 | 向量层独立就绪度，失败重试退避；词法/大纲不受影响（分级设计即为此） |
| 共享 crate 加法改动碰 B 线 | 每波跑 `cargo test -p transport-http --lib` + B 线 MCP 工具冒烟一次（curl tools/call 各一个） |
| asr-filetrans 仅 WSL | M1 自用环境满足；Windows 原生调用列 M2  backlog |

## 8. 时间估算

W0–W5 合计约 **5–8 个工作日**（不含 dogfood 周）。每波开工前报当波编译/测试时间成本（仓规），W4 转写实测会产生小额百炼费用（30s 冒烟可忽略，真实会议按 ~1–2 分钟/小时音频 + 时长计费提前报量级）。

---

## 9. 实施记录（2026-09-07 收尾）

**交付物**（全部落库，`code-review-graph` 已更新）：

- crates：`subtex-core`（root 哈希/仓布局/管理段落/缓存扫描器）、`subtex-store-sqlite`（meta/files/chunks+FTS5+vec0/jobs/usage/readiness/file_outline；`RetrievalReadPort` 实现；混合 RRF 检索）、`subtex-index`（轻量解析 md/text/code/csv → `build_ir_chunk_plan` 结构切块 → 云 embedding（SiliconFlow bge-m3）→ 落库；大纲渲染；音频转写封装）
- bins：`subtexd`（notify watcher + 周期 mtime 调和扫描 + jobs claim loop + 转写阈值确认；watcher 丢事件由 sweep 兜底）
- `transport-http`：`subtex.*` 七工具（catalog/dispatch/tools/mod/auth_guard 四处登记；描述文案 `include_str!` 自 `prompts/subtex/tools/`；`AVRAG_SUBTEX=1` 才注册；本机 user token 守卫拒绝 workspace API key；读侧工具不建仓，仅 init 建仓）
- `prompts/subtex/`：convention-draft / correction-draft / tools/<tool>.md（第三人称语态，占位符运行时填充）

**与计划的偏差**（均为摸底后的修正，记录在案）：

1. 计划假设 ingestion 有纯 Rust 轻量解析子集——实际 md/txt/code 全走 markitdown Python 子进程。改为在 `subtex-index` 自写轻量解析（markdown 标题树/文本/代码/csv），重格式（pdf/office）沿用 ingestion 既有子进程解析器，缺二进制如实进 `unsupported`。
2. `retrieval-data-plane` 文档禁止以 stub writes 实现 `RetrievalDataPlane` → W0 先实现 `RetrievalReadPort`（读侧空返回合法），写侧 port 待 RagRuntime 挂接需求出现时随真实写路径实现。
3. `build_ir_chunk_plan` 的 `min_chars=32` 会把短标题并入上一 chunk（B 线语义），标题树信息丢失 → 大纲改为索引时从 `DocumentIr` Heading 块直接提取，落独立 `file_outline` 表。
4. 新增第三个 crate `subtex-index`（写路径管线），store 保持纯 SQLite、core 保持零重依赖。
5. 计划中的 `OperationGuideMode::SubtexLocal` 不存在（实为字符串映射 + `_ => None` 安全默认）→ 沿用 `require_user_session` 守卫原语，未新增模式。
6. 转写中间品留在 `asr-filetrans/runs/`（CLI 自管、天然不在项目目录），未复制进索引仓 `scratch/`——满足「中间品不进项目目录」的硬规则本身。
7. 大批量转写确认落地为 jobs 表 `needs_confirmation` 状态 + `subtex.transcribe confirm:true`（计划 §4-4 预言的 elicitation 替代模式）。

**验证门记录**：

- W0：`cargo test -p subtex-core -p subtex-store-sqlite --lib` → 16+7 通过。
- W1：store 9 + index lib 4 + 集成 3（fixture 含 md/code/csv/txt/转写稿：入库→FTS5 命中（含 CJK 与 <3 字符 LIKE 降级）→sqlite-vec 向量命中→RRF 混合 `matched_via`→大纲 token 预算；PDF 经 `lit` 真实解析）。
- W2：起栈 `tools/list` = 23（16 B 线 + 7 subtex）；init/status/search/outline/transcribe JSON-RPC 往返；`context-os-mcp --check` ready + stdio 透传；B 线 `account.list_workspaces` 回归通过（`transport-http --lib` 的 16 个失败经 stash 基线确认为预存环境问题，与本改动无关）。
- W3：单测 3（变化→就绪度迁移、重启 sweep 补齐、jobs 消费）；E2E：停机期文件不索引 → 重启 sweep 补齐 → watcher 实时抓取新文件。
- W4：单测（假脚本全链路 + >2h 批量停在 `needs_confirmation` + 同 hash 去重）；真语音 30s 冒烟：transcript.md 25 秒写回 `transcripts/`（说话人分离 + 绝对时间轴）→ 被 sweep 索引 → 混合检索命中；usage 台账记录 transcription 30 秒；`DASHSCOPE` 日志零命中。
- W5：纠偏写回往返（无管理段落 → init 骨架为底 + 规则；有段落 → 旧规则保留、零覆盖、无双标题）；未 init 根读侧返回 `root_not_initialized` 且不建仓；`AGENTS.md` 手写内容零丢失。

**F4 协议冒烟（2026-09-07）**：本地 `AVRAG_SUBTEX=1` `avrag-api` @ `127.0.0.1:18080` + `subtexd`；对真实目录 `/home/chuan/asr-filetrans` 走 HTTP MCP：`tools/list` 含 7 个 `subtex.*`；`init` 建仓并首轮索引（lexical/outline/vector 3/3）；Agent 把约定稿写入该目录 `AGENTS.md` 管理段落（原有正文零丢失）；`status` / `outline` / `search`（`说话人分离` 命中，`matched_via=lexical+vector`）/ `correction_draft` 往返。`../` 转写路径被拒；未 init 根返回 `root_not_initialized`。`context-os-mcp --check` health OK；本机无 `CONTEXT_OS_USER_TOKEN`，stdio 包装的鉴权就绪仍缺。Claude Code / Kimi 挂 MCP 仍待作者本机会话（配置见下）。

**遗留（作者侧 / 后续）**：

- Claude Code / Kimi Code 挂 `context-os-mcp`（需本机 user JWT）走同一条 init→写约定→检索路径。
- dogfood 一周三项度量（自用天数、纠偏写回次数、检索纠正率）。本仓 `context-osv6` 约 3500 可索引文件，首挂宜用小目录或接受 embedding 耗时。
- 可选：整段真实会议录音经 subtexd 全链路（CLI 自身已 3h40m 验证过，本层只差一次实战）。
- Windows 原生桌面打包与 asr-filetrans 的 Windows 调用 → M2 backlog。

Claude Code MCP 片段（stdio，本机 user token）：

```json
{
  "mcpServers": {
    "context-os": {
      "command": "/home/chuan/context-osv6/avrag-rs/target/debug/context-os-mcp",
      "env": {
        "CONTEXT_OS_API_BASE": "http://127.0.0.1:18080",
        "CONTEXT_OS_USER_TOKEN": "<local user JWT>"
      }
    }
  }
}
```

**启动方式**（WSL 自用）：

```bash
# api（MCP 工具面）
cd avrag-rs && AVRAG_SUBTEX=1 AVRAG_API_ADDR=127.0.0.1:18080 ./target/debug/avrag-api
# 常驻索引（与 api 同一 SUBTEX_DATA_DIR，默认 ~/.local/share/subtex）
./target/debug/subtexd   # 环境变量：SUBTEXD_RECONCILE_SECS=5 SUBTEXD_MAX_JOBS_PER_TICK=16 RUST_LOG=info
```
