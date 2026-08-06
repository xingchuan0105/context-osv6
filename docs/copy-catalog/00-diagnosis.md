# 文案诊断报告（00）— Context-OS 全量文案体检

> 生成：2026-08-06 · 基于 `docs/copy-catalog/01/02/03` 全量索引 + 源文件抽查
> 目标读者：改文案的人（AI + 用户）。本报告给出**问题清单、证据、术语决策草案和分层实施顺序**。

---

## 0. 范围与体量

| 域 | 规模 | 诊断结论 |
|----|------|---------|
| UI i18n（01） | 1006 键，16 个域文件 | 50% 为短标签（无需动）；**147 个长文案（≥20 字）为优化主体** |
| prompts（02） | 118 现行文件 | 结构/第三人称规范已达标；需单独立项做契约一致性评审 |
| 内联硬编码（03） | 106 处前端 + 邮件/通知后端 | 应迁 i18n；含 1 处转义 bug |
| 邮件/通知 | password_reset.rs + transport/storage/app-chat 等 | 后端通知 90% 为英文硬编码，无中英模板 |

---

## 1. 问题分类（按严重度排序）

### A. 重复键 / 死键（先删后改，避免两套文案漂移）

**A1. admin 域实际有三套文案来源（比快照显示的更复杂）**：
- `frontend_next/lib/i18n/messages/admin.ts`（78 键）：`adminShellTitle` / `adminNavLabel` / `adminNavAccounts`…（扁平）被 `admin-shell.tsx:104-131` 导航使用；同时 `"admin.nav.*"`（点分，12 组重复值）被各 `admin-*-surface.tsx` 的 `adminText(locale, "admin.nav.*")` 使用。
- `frontend_next/components/admin/i18n/copy.ts`（**184 键，未进 01 快照——索引盲区**）：admin 页面内部文案主体（`common.*`、`audit.*`、`users.*` 等），经 `components/admin/i18n/labels.ts` 的 `n(locale, key)` 读取。
- 处置：P4 时**两套字典同值同步改写**（保持结构不动，T5）；并评估将 `copy.ts` 纳入快照导出范围。


**A2. workspace.ts 进度键重复（1 组）**：`progress.reason_preview`（workspace.ts:637）与 `progress.reasonPreview`（workspace.ts:777）。
- 证据：`use-progress-tracker.ts` 引用其一（见 §5 处置时先确认哪个在用）。
- 处置：保留在用键，删除死键。

### B. 内部术语 / 工程腔（C 端用户看不懂的词）

| 现用词 | 出现 | 面向用户建议 | 证据示例 |
|--------|------|-------------|---------|
| RAG | 11 | 知识库检索（产品内已有"知识库/网络搜索"叫法） | `workspaceChatComposerPlaceholder` zh「可开启知识库 / 网络搜索…」en 却写「toggle RAG / Search below」——**中英自相矛盾** |
| 降级 / Degradation | zh 9 / en 5 | 限速（paywall 域已用「平台会暂时限速」，应统一） | `adminNavDegradation`「降级」、`usageSoftLimitWarning`「接近平台保护限速」（已用限速） |
| 执行器 / Workers | 3 | 后台语境可保留「执行器」；用户可见处避免 | `adminNavWorkers`「执行器」 |
| Owner / Owner-pays | 3+ | 工作区所有者 / 所有者付费 | `shareCenter.visitorModeHint` 直接给用户看「Owner-pays」 |
| 墙钟 / Wall clock | 1 | 用时 / 耗时 | `03:72` help/write 页「墙钟」 |
| 代理 / proxy | 2 | 访客（脱敏） | `dashboard-analytics-surface`「独立访客（代理）」 |
| 资源 ID | 1 | 资源编号 | `admin.searchPlaceholder`「按名称、邮箱或资源 ID 筛选」 |
| 当前环境 | 2 | 当前（产品语境） | `authResetUnavailable`「当前环境未启用密码找回」 |
| guardrail | 1 | 安全护栏 / 安全审查 | `workspaceGuardIntervened`「Guardrail 已介入当前回答」 |
| 指纹 / band | 2 | 质量校验 / 校验项 | help/write「指纹 band 未全部通过」 |

### C. 内部数据/字段名裸露给用户

| 键 | 现值 | 建议 |
|----|------|------|
| `shareAnalyze.trendSubtitle` | 「按日汇总的分享访问量（views_by_day）」 | 删除 `（views_by_day）` |
| `workspaceEmptyStateModeHint` | 「当前：{mode} · 可开启…」 | 用「知识库 / 网络搜索」等用户可读模式名 |
| `usageMarginNote` | 「参考乘数 M={m}（平台模型计费折算）」 | 「参考计费系数 {m}」 |
| `workspaceRightRail.viewerLocation` | 「第 {page} 页 · 游标 {cursor}」 | 游标 → 位置 |

### D. 中英不对称（en 口语化、zh 书面腔；方向：向 en 的用户视角对齐，保留 zh 简洁）

典型：
- `upgradeModal.subtitle` zh「选择适合你的方案，支付完成后即可立即生效。」 vs en「Pick a plan. Access updates as soon as payment completes.」→ zh 可简化为「选择方案，支付完成后立即生效。」
- `shareCenter.visitorModeHint` zh 长句堆砌（「匿名链接」「定向邀请 / 须登录」「Owner-pays」）vs en 短句分点 → zh 应改为「匿名链接：未登录也可提问（按访客限次）。定向邀请 / 须登录：仅登录访客可提问。模型费用由所有者余额或自定义 Provider 承担。」
- `admin.pageSubtitle` zh「查看账户、用量、健康状态和系统级运营数据。」 vs en「Review accounts, usage, health, and system-wide operational signals.」→ zh 可对齐「查看账户、用量、健康与系统级运营数据。」

### E. 一致性（术语 / 格式 / 标点）

- **产品名**：`Context-OS`（18）与 `Context OS`（2）混用 → 统一 `Context-OS`（品牌名）。
- **中英夹杂**：`shareCenter.inviteSectionSubtitle` zh「邀请成员参与当前 Workspace」→ 「工作区」。
- **标点**：265 个长文案无句末标点（`admin.pageSubtitle`、`authLoginSubtitle`、各 `sectionSubtitle` 等）；短标签无标点是对的，**长句/错误提示应统一句末句号**。
- **删除确认句式**：`workspaceDeleteSessionDialogBody`「确定删除会话「{title}」吗？此操作无法撤销。」vs `dashboardDeleteWorkspaceConfirm`「确定删除 {title} 吗？此操作无法撤销。」→ 统一。
- **省略号**：「加载中…」vs「加载中...」vs「创建中...」（半角/全角混用，`03` 里也有）→ 统一全角 `…`。
- **中英占位符风格**：`{title}` 一致，但 `quotaValue` zh「已用 {used} / 上限 {max}（{plan}）」en「{used} used / {max} max ({plan})」→ 语义对齐。

### F. 内联硬编码（03，106 处）——迁移 + 顺带修复

- **Bug（高优先）**：`frontend_next/components/dashboard/dashboard-surface.tsx:140` zh 为字面 `\u6765\u6e90`（转义序列未被解释，界面会显示 `\u6765\u6e90` 而非「来源」）。
- 富文本编辑器工具栏（tiptap 撤销/重做/粗体…）整组可迁 `common` 或新 `editor` 域。
- 分析页/升级弹窗/分享邀请面板等新组件内联文案 → 迁 i18n（README 已列流程）。

### G. prompts（02，118 文件）——单独立项

- 现状已符合 prompts-in-md + 第三人称观察规范（`contract.md`、loop 观察均达标）。
- 剩余工作为**契约一致性评审**：capability contract 与 SKILL 措辞重叠度、`reference/strategies-*` 与 knowledge-base SKILL 的职责边界是否清晰、中英混合是否统一。
- 注意：prompts 的读者是模型，优化方向 ≠ C 端 UI 文案；**不要**用本节 UI 术语表去改 prompts（prompts 内 `client.dense`、`doc_id` 等机制名词保留）。

### H. 邮件 / 通知后端

- 邀请信中文模板硬编码在 `password_reset.rs`（`is_zh` 常 true）。
- 余额/分享限流/密码修改等后端通知**全部英文硬编码**（transport-http / storage-pg / app-chat 等，见 03 尾部）。
- 处置：属「后端文案模板化」专项（涉及 Rust 改动 + 通知渠道抽象），建议排在 UI 之后，另行评估是否纳入本轮。

---

## 2. 建议的分层实施顺序（每层端到端验证后再进下一层）

| 层 | 内容 | 验证门 |
|----|------|--------|
| P0 | 术语决策确认（下表）+ 死键/重复键清理 | `rg` 确认无残留引用；`pnpm` 类型检查 |
| P1 | 高触达域：auth / dashboard / workspace 输入与空态 / upgrade / paywall / usage | `pnpm test` 相关页 |
| P2 | share（分享/访客，增长场景） | 同上 |
| P3 | settings | 同上 |
| P4 | admin（管理员视角，可保留部分技术词） | 同上 |
| P5 | 内联 106 处迁 i18n（含 `\u6765\u6e90` bug） | 全量 `pnpm test` + typecheck |
| P6 | prompts 契约一致性评审（单独立项） | prompts 校验测试（含 host_markers parity） |
| P7 | 邮件/通知模板化（另行评估） | Rust 相关测试 |

> 每层改完需**重新导出快照**（01/03 是快照，改源后需同步），并更新 `log`/索引。

## 3. 术语决策草案（待用户确认，确认后作为 P1+ 的统一替换表）

| 术语 | 替换为（面向用户文案） | 保留场景 |
|------|----------------------|---------|
| RAG | 知识库检索 / 知识库 | admin 内部页、prompts 机制名 |
| 降级 / Degradation | 限速 / Throttling | 管理后台「降级策略」运维语义 |
| Owner | 工作区所有者（或所有者） | prompts |
| 墙钟 | 用时 | — |
| proxy / 代理 | 访客（脱敏） | — |
| 资源 ID | 资源编号 | — |
| guardrail | 安全护栏 | prompts |
| 指纹 band | 质量校验 | prompts |
| Workspace（中文句内） | 工作区 | 产品名词首次出现可附英文 |

## 4. 执行状态（2026-08-06）

| 层 | 状态 | 说明 |
|----|------|------|
| P0 | ✅ | 死键 `progress.reason_preview` 删除；admin 三来源确认（lib 78 键 + 组件 copy.ts 184 键，后者已补入索引盲区说明） |
| P1 | ✅ | auth/dashboard/workspace/upgrade/paywall/usage 改写完成；typecheck 零新增错误 |
| P2 | ✅ | share 236→242 键改写（22 处） |
| P3 | ✅ | settings 改写完成（15 处 + 内联键） |
| P4 | ✅ | lib admin.ts 4 处 + 组件 copy.ts 17 处（含 workspace 误译「知识库→工作区」、废弃词 notebooks/Org ID 修正） |
| P5 | ✅ | 106 处内联全部迁入 i18n（+60 键）；修复 `\u6765\u6e90` 转义、chat-pane 冗余分支；typecheck 零新增错误 |
| P6 | ✅ | 01 快照重导出（1106 键）、03 快照重写、README 键数同步 |
| P6.5 | ⏸ 另立项 | prompts 契约一致性评审 |
| P7 | ✅ 邮件 + 站内通知 | SMTP `avrag-rs/email/`；bell `avrag-rs/notifications/` + `common::notification_copy`；API 错误串仍 open |

> 注意：本轮执行期间检测到 settings 组件被**并发进程重构**（providers-panel 405 行重写、settings-surface 布局改动），已基于重构后的新版本完成迁移。

