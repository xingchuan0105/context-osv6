# 迁移后未竟事项收口计划：生产接线 / 密钥卫生 / 工作区提交 / review 欠账

| 项目 | 内容 |
|---|---|
| 类型 | 收口计划（多来源未竟项汇总） |
| 日期 | 2026-08-03 |
| 范围 | SiliconFlow 迁移生产侧接线；密钥泄漏处置；工作区剩余改动提交；两轴 review 未修项；验证欠账 |
| 前序 | `docs/engineering/2026-08-03-siliconflow-migration-handover.md`（迁移+门禁）；commit `14bd76c9`（迁移）/ `e027ea67`（ingestion 提速）/ `db1d052c`（E2E 工具） |
| 状态 | 执行中（2026-08-03）：P0(a) 全文 seed 已在 e027ea67；pandoc timeout 已齐；.env.example 已 scrub；live .env 未用泄漏 key。**仍需在百炼控制台 revoke `sk-292b16…`（git 历史残留）** |

---

## 0. 一句话

迁移与提速已 commit、E2E 侧全绿；剩下的是**生产切流量、一个必须轮换的泄漏 key、73 项工作区残留的分桶提交、review 发现的 1 个 P0 质量回退（summary 输入截断）+ 若干 P1/P2、以及三条验证欠账**。无一是新设计，全是收尾。

---

## 1. 生产侧接线（SiliconFlow 切流量）

现状：`.env`（dev）已切 SiliconFlow 且门禁绿；**生产未切**（迁移 handover §8.4）。

| # | 步骤 | 验收 |
|---|---|---|
| 1.1 | 生产 `.env` 四槽位按 handover §4 终态写入（key 复用 `SILICONFLOW_API_KEY`；`EMBEDDING_DIMENSIONS` 留空、`RERANK_API_STYLE` 留空） | 配置落位 |
| 1.2 | 部署仅走 `scripts/deploy-*.sh`（仓库硬规则）。`deploy-backend.sh` 的 pandoc 安装段在工作区未提交 → 先随 §3 桶 C 提交再部署 | worker 重启后 `command -v pandoc` 可见 |
| 1.3 | **生产库重灌**（bge-m3 与百炼向量不能混排）：生产 Milvus 集合按既有 prefix/drop 机制重建 → 全量重灌。重灌期间查询侧质量退化（旧向量仍在、新模型查询向量空间不匹配）→ **先切 embedding 配置并重灌完成，再放查询流量**；或选低峰窗口 | 重灌完成、集合维度 1024 |
| 1.4 | 切换后观察：rerank 在查询关键路径（实测 32 文档 SF 比百炼慢 ~0.25s/次），看 P95；mm embed 图文 ~0.86s/张（扫描 PDF 页图路径会累积） | 无超基线告警 |
| 1.5 | 回滚预案：四组键改回百炼（handover §6）+ 重灌；代码分支（OpenAiVl*）保留无害 | 预案写进部署记录 |

## 2. 密钥卫生（P0，先做）

- **轮换 `sk-292b16...`**：该 key 以明文出现在已提交的 `.env.example` 历史中（`DASHSCOPE_API_KEY`、`MM_*_API_KEY` 行仍在用此值）——**git 历史已泄漏，删文件不够，必须轮换**。
- 轮换后：`.env.example` 所有真实 key 痕迹改占位符（`sk-your-...`）；`.env` 静默换值。
- 顺手核对仓库无其他明文 key（`grep -rn "sk-" --include="*.example" --include="*.md"` 全仓扫）。

## 3. 工作区剩余改动提交（73 项分桶）

| 桶 | 内容 | 处置 |
|---|---|---|
| **A. ingestion 部署补漏** | `scripts/deploy-backend.sh`（pandoc + office-direct venv 安装段）——ingestion 提速（`e027ea67`）的部署侧尾巴 | 单独 commit `chore(deploy): pandoc/office-direct 安装段`。**附带小问题**：该文件新增 echo 行含 `\$(command -v …)` 转义，输出会打印字面 `$()` 而非路径（cosmetic，顺手修） |
| **B. QC 主题残留** | `AGENTS.md` 剩余 hunk（evidence_missing/required_action 结构性门，18 行）——属 `4646e934` 特性文档 | 单独 docs commit `docs(agents): QC 结构性门规则补录` |
| **C. 文档体系梳理** | `avrag-rs/docs/**` + `docs/**` 共 54 文件 +208/-157（多为"已取代"标注两行改）+ `docs/README.md`、`docs/agent/code-review-graph.md`（untracked） | 一个 `docs:` 提交 |
| **D. 配置/杂物** | `.gitignore`、`.mcp.json`、`opencode.json`、`CONTEXT.md`、`DESIGN.md`、`avrag-rs/CLAUDE.md`、`avrag-rs/README.md`、`avrag-rs/modes/rag.yaml`、`avrag-rs/prompts/capabilities/**`、`avrag-rs/handoff-rag-evaluator-fix.md`、`figma-parity-gate.md`、`frontend_next/next-env.d.ts` | 逐个过 diff，同主题归并提交；纯自动生成churn（next-env.d.ts）随最近主题带上 |
| **E. 设计参考废纸** | untracked：`DESIGN-cursor.md`、`design-md/`、`login-light.png` | **gitignore**（本地保留，不入库） |

## 4. 两轴 review 未修项（2026-08-03 review；已复核当前码态）

> 已被后续会话闭环、**不再欠**的：pyproject 依赖残留、media/ 断言缺失、ENABLE_THINKING 死开关、deploy 脚本 pandoc。以下为仍开口项。

### P0（已闭环 · a 全文 seed）

- ~~**summary/profile 输入从全文降级为 1200 字节 preview**~~ **已修**（`build_session_seed_user_message` + `pg_side_effects` seed 全文；preview 仅 `build_section_index_user_message` 独立路径）：seed 轮复用 `build_section_index_user_message`，每 chunk 截 1200B（`section_index.rs:190`）；summary 轮模板只含 `{title}/{filename}` 不携带正文。与设计文档 §4.2 探针口径（全文 72KB 进缓存、实测命中 ~100%）不符。二选一：
  - **(a) 全文 seed**（推荐）：session 新增全文 chunks 消息构造器，对齐探针口径，summary 质量恢复；preview 仅用于 profile 轮的 section-index 消息（保持现样）——即 seed 用户消息 = 全文 chunks + profile 指令。
  - (b) 接受 preview 口径：改设计文档 §4.2 + `interaction-session.system.md`「文档已经在上下文中」改为真值陈述，并接受 summary 只能基于预览的质量上限。
- 连带（无论选哪个都修）：`interaction-session.system.md:8`「未提供的字段不臆造」是祈使禁令，违反仓库 third-person 硬规则 → 改陈述式。

### P1（行为/正确性）

- ~~`pg_side_effects.rs` empty index_chunks 跳过 session~~ **已修**（fallback seed 全文，summary 不连带跳过）。
- visual triplet `completion_cache`：**接受**（与 text triplet/profile/summary 同 completion_cache 口径；重灌去重是收益不是 creep）。写进 handover 一句即可。
- ~~`main.py:_extract_docx` pandoc timeout/FileNotFoundError~~ **已修**。

### P2（卫生，可打包一个 chore commit）

- `route/client.rs`：`build_dashscope_responses_route` 与 `build_openai_responses_route` ~15 行重复 → 加 path 参数合一。
- `schema/events.rs`：`LlmResponse::response_id()` getter 全仓无调用 → 删（字段 pub 直接用）。
- 孤儿代码决策：`SummaryGenerator`/`SectionIndexGenerator` 已无调用方；app-core 仍解析 `TRIPLET_LLM_*` 而 `.env.example` 已摘文档——删除或显式标记保留（回退用途），二选一写清。
- 调用点魔数 `Some(0.1)`/`Some(0.3)`（profile/summary/triplet 三处）→ 命名常量。
- （既有，非本次引入）`build_triplet_extraction_user_message` 内联英文指令 prose 违反 prompts-in-md → 迁入 `prompts/pipeline/`。

## 5. 验证欠账

| # | 项 | 来源 | 口径 |
|---|---|---|---|
| 5.1 | Rust 侧 `cached_tokens` 探针（会话缓存命中确认） | 设计文档 §6.4 | Python 侧已实测 ~100%，Rust `complete_response` 路径补一次 |
| 5.2 | 单文档 E2E <2min 复测 | 设计文档 §6.5 | 门禁跑批旁证已达（thesis index 阶段 149.7s 含会话+mm+graph；text embedding 仅 4.7s）；正式跑一次记数 |
| 5.3 | 全量 149 新管道基线 | 设计文档 §6.6 | 用 `scripts/test-full149.sh`（不强制重灌）取新基线；**定位是回归参照，不是 embedding 质量门**（用户已拍板后者作废） |
| 5.4 | struct caption=None 的选表退化观测 | 设计 review §7 | 无硬门，观测项：多表文档 struct_query 选表准确率 |
| 5.5 | （可选）bge-m3 vs qwen3.7 dense-only A/B | 迁移 handover §8 备注 | 同 golden 问题直打 Milvus 比 top-15 命中率；只有要给"embedding 质量"留档时才做 |

## 6. 已拍板不做 / 暂缓

- **149 题混合检索评测作 embedding 门禁**——废除（用户 2026-08-03：混合口径测不出 dense 好坏；门禁 = 灌库全绿 + 查询冒烟）。
- **embedding A/B 轨管道并行**——暂缓：实测 embedding 非瓶颈（100 块 4-7s），遮蔽收益单薄；如未来 mm 页图路径成瓶颈再议。
- **triplet 会话轮序调整**——不动（顺序对缓存命中无影响；失败面考量保持 triplets 最后）。

## 7. 执行顺序

1. **§2 密钥轮换**（P0，独立于代码，先做）
2. §3 桶 A-D 提交（E 桶问用户后定）→ 工作区清零
3. §4 P0（拍板 a/b 后实施）+ P1 三条 → `cargo test -p avrag-llm --lib` + `-p avrag-struct-supervision --lib` + office-direct pytest 过门
4. §1 生产接线（1.2 依赖桶 A 先提交；1.3 选窗口重灌）
5. §5 验证欠账择机补（5.1/5.2 可与 §4 同批跑）
6. §4 P2 卫生包随手收
