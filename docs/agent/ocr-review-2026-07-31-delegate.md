# 代码审查报告（委托模式 / 宿主 agent = GLM-5.2）— context-osv6

> 审查方式：**OCR delegate 模式**（OCR 仅做文件筛选+规则解析，**不调任何 LLM**）；审查推理由宿主 agent（pi，`PI_MODEL=glm-5.2`）独立完成。
> 审查时间：2026-07-31
> 审查对象：commit `d6d60661` 之后的工作区未提交变更（prompt 布局重构 + 代码路径跟随）
> 对照：上一轮 `ocr review`（DeepSeek）报告见 `ocr-review-2026-07-31.md`

## 范围与覆盖

OCR `delegate preview` 标记 reviewable 仅 3 个（它把 `build.rs`/`prompt_leak.rs`/3 个 yaml 过滤了，且把全部 `.md` 标 `unsupported_ext` 排除）。
**本次作为懂项目规则的宿主 agent，主动覆盖全部真实变更**：

| 类别 | 文件 | 说明 |
|------|------|------|
| Rust | `mode_assemble.rs` `pipeline_tests.rs` `build.rs` `prompt_leak.rs` | prompt 路径跟随 + 组装逻辑重构 |
| YAML | `chat/rag/search/write_refine.yaml` | `system_prompt_base` 路径迁移 |
| Prompt（产品代码）| `system/agent-base.md` `system/write-refine.md` `capabilities/workspace.md` `capabilities/web.md`（新增）+ 4 个删除的旧 `orchestrators/*.md` | **本次审查重点**：迁移完整性 |

> 注：本次审查价值增量在于覆盖了 **13 个 `.md`**——它们是产品代码（`prompts-in-md` 规则），但 OCR 默认排除，DeepSeek 那轮未审。

## 变更主线

把 `prompts/orchestrators/` 下的 4 个 system prompt 拆分重组：
- `chat-base.md` → `system/agent-base.md`（所有模式通用底座）
- `capability-rag.md` → `capabilities/workspace.md`
- `capability-search.md` → `capabilities/web.md`
- `write-refine-system.md` → `system/write-refine.md`
- 旧文件归档到 `deprecated/pre-system-layout-2026-07-31/`
- `mode_assemble.rs`：system prompt 组装从「纯 chat 用 chat-base」改为「恒定 agent-base + 按能力挂载 workspace/web」
- `build.rs`：编译期扫描目录 `orchestrators` → `system` + `capabilities`

---

## 发现（按严重度）

### 🟠 MEDIUM

#### M1. `capabilities/workspace.md` — 删除了「对照示例」few-shot（内容回归风险）

| 字段 | 值 |
|------|----|
| path | `avrag-rs/prompts/capabilities/workspace.md` |
| category | maintainability / 内容完整性 |
| severity | medium |

旧 `capability-rag.md`（79 行）有一整节「对照示例（虚构）」，含 **4 个情境→observation→读出** 的 few-shot：多文档计数、相似点+边界、市场份额数字未覆盖、文档 vs 网页时间冲突并陈。新版 `workspace.md`（45 行）**完全删除了示例节**。

这些示例正是项目 `third-person observation` 范式的核心教学手段（情境→观察→事实读出），对模型正确处理「半截覆盖终答写法」「SELECTED/[[web:n]] 不混挂」「数字未覆盖≠不存在」极关键。workspace/web 是 **always-mounted** 能力段，示例放在这里的覆盖面广于 on-demand 的 codegen skill。

**建议**：恢复精简版示例（至少保留「半截覆盖」和「冲突并陈不混挂」两个），或确认等价示例已迁入 codegen skill 并在该能力段引用。需核实示例未迁入 skill 才算真丢失。

#### M2. `capabilities/web.md` — 删除了「引用示例」few-shot

| 字段 | 值 |
|------|----|
| path | `avrag-rs/prompts/capabilities/web.md` |
| category | maintainability / 内容完整性 |
| severity | medium |

旧 `capability-search.md` 有「引用示例」代码块（`[[web:n]]` + `SELECTED: #3` 的具体写法示范）。新版 `web.md` 删除，仅留 prose 描述「网页引用形态为 `[[web:n]]`」。同 M1，few-shot 对引用格式正确性贡献大。

#### M3. `capabilities/workspace.md` — 方法「负向约束」丢失

| 字段 | 值 |
|------|----|
| path | `avrag-rs/prompts/capabilities/workspace.md` |
| category | maintainability |
| severity | medium |

旧版明确列出可用方法 `dense/lexical/grep/doc_profile/doc_summary/save/load` 且带**负向约束**「无 top_k；无 graph_search、read_lines」，以及 `grep.total_hits=命中行数`、`truncated` 语义。新版改为「方法…见 codegen」，负向约束未保留。负向提示能直接省掉模型试调不存在方法的轮次/token。建议在能力段保留一行方法清单 + 否定项，或确认 codegen skill 首屏有同等提示。

### 🟡 LOW

#### L1. `mode_assemble.rs:52` — `system_prompt_base` 冗余赋值

| path | lines | category | severity |
|------|-------|----------|----------|
| `avrag-rs/crates/app-chat/src/mode_assemble.rs` | 52, 119 | maintainability | low |

第 52 行 `config.system_prompt_base = AGENT_BASE.to_string()` 后，第 119 行又用 `system_prompt_parts.first().cloned().unwrap_or(AGENT_BASE)` 覆盖。由于 parts 现恒以 AGENT_BASE 开头（第 102 行 `vec![AGENT_BASE.to_string()]`），第 52 行赋值必被覆盖，且 `unwrap_or` 分支永不触发。建议删第 52 行赋值（保留末尾的 first-based 赋值更准确反映"base = parts 首项"语义）。

#### L2. `prompt_leak.rs` — `orchestrator-base.md` 孤悬于未扫描目录

| path | category | severity |
|------|----------|----------|
| `avrag-rs/crates/guardrails/src/output/prompt_leak.rs` | maintainability | low |

`prompt_leak.rs` 仍 `include_str!("prompts/orchestrators/orchestrator-base.md")`，但 `build.rs` 已不再扫描 `orchestrators/` 目录。该文件本次未被删（仅 4 个兄弟文件被删/迁），现孤悬于 `orchestrators/`。不影响编译（include_str! 独立于 build.rs 扫描），但目录语义割裂。建议把它也迁入 `system/` 或注释说明其为何保留。

#### L3. `system/agent-base.md` — 角色职责描述弱化（可接受的设计取舍）

| path | category | severity |
|------|----------|----------|
| `avrag-rs/prompts/system/agent-base.md` | style | low |

旧 `chat-base.md`：「你是 Context OS 的**对话助手**。你帮助用户思考、写作、讨论与创意表达。」新版：「你是 Context OS 的助手。」职责定位句被删。因 agent-base 现为所有模式通用底座（不再专指 chat），弱化是合理的；但若希望保留产品人设温度，可补一句通用职责。非必须。

#### L4. `modes/rag.yaml` / `search.yaml` — `system_prompt_base` 字段对产品路径不生效（配置占位）

| path | category | severity |
|------|----------|----------|
| `avrag-rs/modes/rag.yaml`, `search.yaml` | maintainability | low |

两 yaml 把 `system_prompt_base` 改为 `capabilities/workspace.md` / `capabilities/web.md`，但 `mode_assemble.rs` 对 rag/search 路径会用代码组装（agent-base + capability）并覆盖该字段。yaml 值实际不生效，仅占位。注释已说明（"Product path: assemble_mode always loads…"）。可接受，但读者可能困惑 yaml 值与实际不符。

---

## 代码侧总体评价

`build.rs` / `prompt_leak.rs` / `mode_assemble.rs` / `pipeline_tests.rs` / 4 yaml 全部为**机械的路径跟随 + 组装逻辑重构**，方向正确，测试断言同步更新，**未发现 Rust 层 bug**（所有权/错误处理/并发均无问题）。唯一代码冗余是 L1 的重复赋值。

## Prompt 侧总体评价（本次核心）

重构方向正确且符合项目规则：
- ✅ third-person observation 语气贯彻（"本轮已挂载…""回传里未出现的内容处于未知"）
- ✅ DRY：方法细节委托 codegen/search skill，避免重复
- ✅ 能力按需挂载（agent-base 恒定 + workspace/web 条件挂载），架构更清晰
- ⚠️ **但系统性删除了 few-shot 示例（M1/M2）和部分负向约束（M3）**，这是真实的内容回归风险，可能让模型在引用格式、半截覆盖终答、冲突并陈等行为上退化。

**这是 DeepSeek 那轮未能发现的**——它把全部 `.md` 当文档排除，而本次变更的主体恰恰是 prompt 迁移。

## 建议（优先级）

1. **恢复 M1/M2 的 few-shot 示例**（精简版即可），或核实等价示例已迁入 codegen/search skill 且该能力段有引用。
2. **M3**：能力段保留一行方法清单 + 否定项。
3. L1：删 `mode_assemble.rs:52` 冗余赋值（一行）。
4. L2/L3/L4：可选清理。
