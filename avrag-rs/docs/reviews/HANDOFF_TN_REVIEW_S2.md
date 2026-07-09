# 交接文档: Thermo-Nuclear Code Quality Review (Session 2)

> 会话 2 (2026-07-08 14:35~15:15 UTC+8)
> 从 HANDOFF_TN_REVIEW.md 继续，完成 M1 + M2。

---

## 已完成的里程碑

### M0 — 零风险删除 (Session 1)
- 分支: `fix/tn-m0-deadcode`, 提交 `679d7de`
- 138 files changed, +50/-13,021 lines

### M1 — 契约完整性 (Session 2)
- 分支: `fix/tn-m1-contracts` (基于 M0)
- 3 个提交:

| 提交 | 内容 |
|------|------|
| `eab3704` W1a | **修复 F1 CRITICAL**: 给 `ToolSpec` 加 `#[typeshare]`，typeshare 恢复生成 `AgentOperationGuide` 和 `ChatResponse.agent_operation_guide` 字段。删除 3 行 no-op sed。 |
| `0561321` W1b | 新增 `contract-completeness.test.ts` 金标准测试 |
| `fc356f4` W1c | 删除 `typeshare.toml` 中 7 行无效整数映射 |

**计划偏差:**
- `AnswerBlock` 无法迁移到 typeshare (内部标签枚举无 `content` 被 typeshare 1.13.4 拒绝)，保留 ts-rs + Python heredoc 桥接
- W1c 实验失败 (toml 映射不生效)，走备选路径删除 toml 死配置

### M2 — 前端卫生 (Session 2)
- 分支: `fix/tn-m2-frontend` (基于 M1)
- 5 个提交:

| 提交 | 内容 | 变化 |
|------|------|------|
| `38fba54` W2a | 统一 `ApiEnvelope` + `unwrapApiData` -> `requestEnvelope<T>()` | -68 行 |
| `565ba5a` W2b | 统一 notebook->workspace 映射器 | 重组 |
| `99f66b9` W2c | SSE 解析器用 zod schema | +25 行 |
| `fa9555b` W2d-tiptap | 手写 HTML 消毒器 -> DOMPurify | -69 行 |
| `6a1408e` W2d-history | 提取 session title 工具 | -128 行 |

**W2d 延迟项** (低优先级):
- tiptap link panel / toolbar 提取
- tiptap CSS import 修复
- history pane transcript hook 合并

---

## 未完成的工作: M3 ~ M5

### M3 — 模块墙 (include! -> mod, 关键路径)
- W3a: transport-http 7 个 include! -> mod 树
- W3b+W3c: storage-pg 24 个 include! -> mod -> 拆 PgAppRepository -> 8 结构体 -> 提取 app-storage-pg crate

### M4 — 逻辑提取 (依赖 M3)
- W4a: PasswordResetService
- W4b: BillingService
- W4c: Memory*Store port + AUTH_VERSION_BYPASS 清理
- W4d: app-storage-pg crate (与 M3b 合并)

### M5 — 复制粘贴坍缩 (长尾去重)
- W5a~W5f: 各种去重

---

## 当前 Git 状态

| 信息 | 值 |
|------|-----|
| 当前分支 | `fix/tn-m2-frontend` |
| 分支链 | master <- fix/tn-m0-deadcode <- fix/tn-m1-contracts <- fix/tn-m2-frontend |
| 最后提交 | `6a1408e` |
| 合并状态 | 未合并到 master |
| 工作树 | 干净 (只有 .serena/project.yml 改动 + 未跟踪 WIP 文件) |

## 新窗口启动建议

1. **读取计划:** `TN_REVIEW_PLAN.md` 有完整计划
2. **从 M3 继续:** 在 `fix/tn-m2-frontend` 基础上切 `fix/tn-m3-mod-walls` 分支
3. **或先合并:** 将 M0+M1+M2 合并回 master 再开新分支
4. **M3 是关键路径:** 解锁 M4/M5，风险最高，建议串行执行
