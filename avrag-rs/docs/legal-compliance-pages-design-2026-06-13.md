# 法律合规页面设计 — 2026-06-13

Phase 1a 法律页与注册同意链路的验收规格。自动化检查见仓库根目录 `scripts/verify-legal-p0.sh`。

## 9.3 P0 验收标准（摘要）

### 9.3.1 信息架构（P0-IA-*）

- `/legal` 索引、`/legal/terms`、`/legal/privacy`
- `/legal/licenses` 摘要、`/legal/licenses/third-party`、`/legal/licenses/project`（与根 `LICENSE` 同步）
- 首页 / 定价 / 登录页脚含法律三链（`LegalFooterLinks`）

### 9.3.2 页面内容（P0-CNT-*）

- ToS ≥10 章；Privacy 披露 PostgreSQL / 对象存储 / LLM 提供方；明确文档不用于模型训练
- ToS/Privacy `status: published` 方可上线（`scripts/check-legal-publish-gate.sh`）
- 摘要页五段结构；`THIRD_PARTY_NOTICES.md` 与 `public/legal/third-party-notices.md` 无漂移

### 9.3.3 前端实现（P0-FE-*）

- `(marketing)/legal` 路由组，无需登录
- `LegalLayout` / `LegalDocRenderer` / `LegalFooterLinks` / `ConsentCheckbox`
- `scripts/sync-legal-assets.sh`；CI `license-check.yml` 含 NOTICE 漂移 job

### 9.3.4 视觉与可访问性（P0-UX-*）

- 正文区 ~48rem；版本与更新日期；长文 TOC；注册勾选 label 关联

### 9.3.5 用户同意（P0-CON-*）

- 未勾选无法注册；勾选文案链到 terms/privacy
- 落库 `legal_acceptances`（`terms_version`、`privacy_version`、`accepted_at`）
- **版本单一事实源：**
  - 前端：`frontend_next/lib/legal/versions.ts`（与 MDX `version` 字段同步）
  - 后端：`avrag-rs/crates/app-core/src/legal_versions.rs`
- 注册时用户创建与同意记录**同一数据库事务**（`register_user` + `legal_acceptances` INSERT）
- 支付 / 重新确认等场景使用 `record_legal_acceptance`（`context`: `payment` | `re_acceptance`）

### 9.3.6 仓库资产与 CI（P0-PIPE-*）

- 根目录 `LICENSE`；`check-licenses.sh`；`license-check.yml`

### 9.11 签 off（非技术阻塞）

- `legal_review: approved` 需法务完成；技术 P0 仅要求 `status: published` 与版本一致性

## 相关迁移

- `avrag-rs/migrations/0041_legal_acceptances.up.sql`
