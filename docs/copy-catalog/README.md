# Context-OS 文案总索引（集中修改用）

> 生成时间：2026-08-06  
> 用途：把**产品可见文字**与**LLM 提示文**的出处做成一张地图；改文案时按「权威源文件」改，不要改本文件当运行时源。

## 怎么用

| 你想改的内容 | 改哪里 | 不要改哪里 |
|-------------|--------|-----------|
| 前端界面中英文（按钮/空态/设置/分享…） | `frontend_next/lib/i18n/messages/*.ts` | 组件里硬编码 `locale === "zh-CN" ? …`（应迁入 i18n） |
| LLM 系统/技能/loop 观察 | `avrag-rs/prompts/**`（现行树） | Rust 源码里的多行指令字符串 |
| 法律正文 | `frontend_next/content/legal/*.mdx`、`frontend_next/public/legal/` | — |
| 邮件（邀请/重置密码） | `avrag-rs/crates/app-bootstrap/src/services/password_reset.rs` | — |
| 后端通知 title/body（余额等） | 各 emit 点（见下文「后端硬编码」） | — |

**本目录文件：**

| 文件 | 内容 |
|------|------|
| `README.md`（本文件） | 地图、分区、统计、修改约定 |
| `00-diagnosis.md` | **文案体检报告**：问题分类、证据、术语决策表、分层实施顺序 |
| `01-ui-i18n-full.md` | **全部** UI i18n 键 + 中英文对照（主编辑面） |
| `02-prompts-inventory.md` | 现行 prompts 清单 + 标题/预览（LLM 文） |
| `03-inline-and-backend.md` | 前端内联中英 + 邮件/通知硬编码线索 |

---

## 1. 产品 UI 文案（i18n）

- **键总数**：1105
- **加载**：`frontend_next/lib/i18n/messages/index.ts` 合并各域 → `formatUiMessage(locale, key)`
- **形状**：每个键 `{ zh, en }`（`zh` 对应界面 `zh-CN`）

| 文件 | 键数 | 域 |
|------|------|-----|
| `share.ts` | 242 | 分享中心/访客/分析 |
| `workspace.ts` | 244 | 工作区对话壳/右栏/会话 |
| `settings.ts` | 196 | 设置（会员/资料/安全/通知…） |
| `admin.ts` | 78 | 管理后台 |
| `auth.ts` | 63 | 登录/注册/重置密码 |
| `dashboard.ts` | 76 | 工作台列表 + 分享分析 |
| `help.ts` | 57 | 帮助页（含 Write 模式用量表） |
| `common.ts` | 33 | 通用（模态、升级弹窗壳、产品 chrome） |
| `pricing.ts` | 31 | 定价页 |
| `desktop.ts` | 31 | 客户端下载/激活 |
| `usage.ts` | 24 | 用量 |
| `paywall.ts` | 9 | 用量墙 |
| `upgrade.ts` | 6 | 升级相关 |
| `legal.ts` | 5 | 法律页壳 |
| `gate.ts` | 3 | 功能门控 |
| `home.ts` | 0 | 落地/入口 |

完整键表见 → [`01-ui-i18n-full.md`](./01-ui-i18n-full.md)

### 修改流程（UI）

1. 在 `01-ui-i18n-full.md` 里搜中文/英文定位 **key**
2. 到对应 `frontend_next/lib/i18n/messages/<file>.ts` 改 `zh` / `en`
3. 本地 `pnpm test` 相关页；类型 `UiMessageKey` 会约束 key 名
4. **禁止**在组件里新增 `locale === "zh-CN" ? "…" : "…"`（应先加 i18n key）

当前扫描到的内联中英 residual：**0 处产品文案内联**（原 106 处已全部迁入 i18n，2026-08-06；剩余 `locale` 判断为字典层/Intl/标记等合理保留，见 `03`）

---

## 2. LLM 提示文（prompts）

权威：`avrag-rs/prompts/README.md` + 根 `AGENTS.md`（prompts-in-md、第三人称观察）。

- **现行文件数**（排除 `_backups` / `deprecated` / README）：118
- **含归档总数**：160

| 族 | 路径 | 角色 |
|----|------|------|
| system | `prompts/system/` | agent-base + hints |
| capabilities | `prompts/capabilities/` | 知识库 / 联网 contract+SKILL |
| clusters | `prompts/clusters/` | 厚技能（memory/writing/docscope…） |
| loop | `prompts/loop/` | 运行时观察注入 |
| synthesis | `prompts/synthesis/` | 终答契约块 |
| pipeline | `prompts/pipeline/` | 摄取/query-card/摘要/三元组 |
| templates | `prompts/templates/` | worker user 模板 |
| agent-guide | `prompts/agent-guide/` | 外部 API 摘要 |
| deprecated / _backups | 同名目录 | **勿作产品入口** |

清单 + 预览 → [`02-prompts-inventory.md`](./02-prompts-inventory.md)

---

## 3. 其它文案源

| 类型 | 路径 | 说明 |
|------|------|------|
| 法律 MDX | `frontend_next/content/legal/` | 条款正文 |
| 法律静态 | `frontend_next/public/legal/` | LICENSE、第三方声明 |
| 产品页静态 docs | `frontend_next/public/docs/` | 帮助类 md |
| 邮件 | `password_reset.rs` | 邀请/重置（部分硬编码中文） |
| 通知 | transport/app-bootstrap emit | 部分英文硬编码（复审已标） |
| 进度标签 | `agent-loop/.../progress/labels.rs` | 短 UI 进度（非模型指令） |

线索汇总 → [`03-inline-and-backend.md`](./03-inline-and-backend.md)

---

## 4. 不纳入本索引（非产品文案）

- 测试 fixture / golden-set 实体（禁止进产品 prompts）
- 机器码：`exit_reason`、error code、host marker tag
- CSS class / testid / 路由 path

---

## 5. 建议的集中改文节奏

1. **产品 UI 中文**：`01-ui-i18n-full.md` 按域扫 settings / workspace / share / dashboard
2. **产品 UI 英文**：同一表对照 en 列
3. **用户可见硬编码**：`03` 内联清单 → 回填 i18n
4. **模型侧**：按 `02` 选族改（agent-base / capability / loop 观察优先）
5. **邮件/通知**：`03` 后端段

