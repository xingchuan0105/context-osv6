# 交接文档: Thermo-Nuclear Code Quality Review

> 会话 `sess_1783483903448_auwvicey2ss` (2026-07-08 04:11~05:36 UTC+8)
> 因 GLM-5.2 provider 端 tool-call 映射 bug 卡死,新窗口继续执行。

---

## 背景

使用 `$skill: thermo-nuclear-code-quality-review` 对整个代码库进行"核弹级"代码质量审查(排除 Rust 前端目录)。

## 已完成的阶段

### ✅ 第 1 阶段: 审查(Survey) — 已完成

8 个并行 subagent 分别审查了:
| Agent | 审查范围 |
|-------|---------|
| Elena | transport-http & cross-cutting |
| Gita | frontend_next |
| Finn | contracts crate |
| Aria | app-chat crate |
| Hugo | scripts, desktop, auxiliary |
| Dario | ingestion, worker, heavytail |
| Cleo | RAG & retrieval stack |
| Bram | app-layer crates |

产物: **[THERMO_NUCLEAR_REVIEW_2026-07-08.md](file:///home/chuan/context-osv6/THERMO_NUCLEAR_REVIEW_2026-07-08.md)** (项目根目录,未跟踪)

**发现: 6 CRITICAL + 25 HIGH 问题。** 6 个 CRITICAL:
- **S1** — `include!` 扁平命名空间(transport-http 7 文件 + storage-pg 24 文件)
- **S2** — ~1,750 行死代码(replay.rs + audit.rs + app 残余架构)
- **S3** — 业务逻辑在 HTTP handler 中(reset.rs, billing/api.rs)
- **F1** — `ChatResponse.agent_operation_guide` 从 TS 契约中静默漂移(CRITICAL)
- **C1** — `run_document_pipeline_inner` 620 行上帝函数
- **S4** — Memory fallback 内联到 HTTP handler 中

### ✅ 第 2 阶段: 修复计划编排 — 已完成

用户确认了 4 项决策后,生成了 6 个里程碑的完整执行计划。

计划文件: `~/.dimcode/v2/data/plan-drafts/sess_5f1783483903448_5fauwvicey2ss/draft.md`
(已复制副本到 `TN_REVIEW_PLAN.md`,见项目根目录)

### ✅ 第 3 阶段: M0 零风险删除 — 已完成 ✅

- **分支:** `fix/tn-m0-deadcode`
- **提交:** `679d7de refactor(m0): delete ~13k lines of dead code, dead files, and WIP stubs`
- **变更:** 138 files changed, +50/-13,021 lines
- 已合并到 `fix/tn-m0-deadcode` 分支,未合并到 `master`

#### M0 的子工作包(全部完成):

| Workstream | Agent | 任务 | 状态 |
|-----------|-------|------|------|
| W0a | Kai | app-chat 死代码(删除 replay.rs 725L, audit builder, dead fields) | ✅ |
| W0b | Leo | app crate 残余架构(vestigial Secure*Service/Runtime) | ✅ |
| W0c | Mia | 删除死脚本 + ~105 个跟踪死文件 | ✅ (后因 stash/pop 回退,由主 agent 恢复) |
| W0d | Nora | 死 PG 全文搜索(104L) | ✅ |

**M0 验证:** `cargo test --workspace` ✅, `cargo clippy --all-targets -- -D warnings` ✅

---

## 卡死的点

在 M0 提交完成后,用户输入 **"继续M1"**,GLM-5.2 的 API 返回了:

```
PROVIDER_INPUT_MAPPING_ERROR
Tool result references unknown tool_use id: call_d2373e82420e44c2a125ae59
```

**原因:** GLM-5.2 服务端的 tool-call 映射 bug,非上下文溢出(那时单次上下文仅 145K token,GLM-5.2 窗口 1M)。该会话已连续 3 次在"继续"时被同一错误打回,消息历史中已固化了损坏的 tool_use id,无法在同一会话中恢复。

---

## 未完成的工作: M1 ~ M5

### ▶ M1 — 契约完整性(修复 CRITICAL 生产风险)

用户已表达的意图: **"继续M1"**。

| 子任务 | 内容 |
|-------|------|
| W1a | 修复 `agent_operation_guide` 漂移: `ToolSpec` + `AnswerBlock` 加 `#[typeshare]`,删除 codegen 脚本中的内联 heredoc |
| W1b | 添加 TS key-completeness 金标准测试 |
| W1c | 删除 92 个冗余 `serialized_as = "number"` 整数注解 |

### ▶ M2 — 前端卫生

| 子任务 | 内容 |
|-------|------|
| W2a | 统一 `ApiEnvelope` + `unwrapApiData` 为 `requestEnvelope<T>()` |
| W2b | 统一 notebook→workspace 映射器 |
| W2c | SSE 解析器用 zod schema 替代手动转型 |
| W2d | 分解上帝组件(tiptap 852L, history-pane 743L) |

### ▶ M3 — 模块墙(include! → mod,关键路径)

| 子任务 | 内容 |
|-------|------|
| W3a | transport-http: 7 个 `include!` → mod 树 |
| W3b+W3c(合并) | storage-pg: 24 个 `include!` → mod → 拆 `PgAppRepository` → 8 结构体,提取独立 crate |

### ▶ M4 — 逻辑提取(依赖 M3)

| 子任务 | 内容 |
|-------|------|
| W4a | 从 HTTP handler 提取业务逻辑(reset.rs, billing/api.rs) |
| W4b | 提取 `run_document_pipeline_inner` 逻辑 |
| W4c | Memory fallback 从 HTTP handler 提取 |
| W4d | `app-storage-pg` 独立 crate(与 M3b 合并) |

### ▶ M5 — 复制粘贴坍缩(可独立于 M3 的部分并行)

| 子任务 | 内容 |
|-------|------|
| 3x `SignatureCheck` 变体统一 |
| 2x `agent_operation_guide` 枚举合并 |
| 3x `MilvusFile` 定义合并 + `Option<serde_json::Value>` → 强类型 |
| 2x `BudgetCheck` + 同一枚举合并 |
| 2x `mapping_ttl_for_collection` 合并 |

---

## 当前 Git 状态

| 信息 | 值 |
|------|-----|
| 当前分支 | `fix/tn-m0-deadcode` |
| 最后一个提交 | `679d7de refactor(m0): delete ~13k lines of dead code` |
| M0 合并状态 | 未合并到 `master`,(可在该分支继续,或开新分支继续) |
| 未跟踪文件 | 大量 WIP 文件(新功能研发,不在审查范围) |
| 检查项 | 工作树干净(只改过 `.serena/project.yml`,其他都是未跟踪) |

## 新窗口启动建议

在新窗口可以:

1. **读取计划:** 项目根目录 `TN_REVIEW_PLAN.md` 有完整计划副本
2. **从 M1 继续:** 在 `fix/tn-m0-deadcode` 基础上切 `fix/tn-m1-contracts` 分支
3. **或者整合作业:** 将 `679d7de` 合并回 `master` 再开新分支
4. **模型:** 用户之前指定了 subagent 用 `deepseek4pro`
