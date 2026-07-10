# Ingestion 卡住修复 + 彻底去掉 Org 概念 — 统一方案（2026-07-10）

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-10 |
| 状态 | **In progress** — I1–I6 + B1–B5 已落地；现场 PDF 已验收 `completed`+`chunk_count=483`；**O 波未开** |
| 类型 | 统一修复 / 迁移方案（Solo local trunk） |
| 上游诊断 | [`INGESTION_PDF_STUCK_DIAGNOSIS_2026-07-10.md`](./INGESTION_PDF_STUCK_DIAGNOSIS_2026-07-10.md)（§3.1 根因已订正，见本文 A 部） |
| 命名先例 | [`WORKSPACE_RENAME_DECISIONS_2026-07-09.md`](./WORKSPACE_RENAME_DECISIONS_2026-07-09.md)（notebook→workspace，**无长期双挂**） |
| 计费先例 | [`docs/adr/0001-user-level-billing-b2c.md`](../adr/0001-user-level-billing-b2c.md)（订阅已 `user_id`） |
| 产品边界 | 个人知识产品：账号 + workspace；**无**团队/组织产品面 |

---

## 0. 一句话

1. **Ingestion**：simple_pdf 已解析出 IR，却在 `build_ir_chunk_plan` 对每个 micro-block 重复构造 `cl100k_base` tokenizer，debug 下烧满 300s → 0 chunks / timeout 循环。  
2. **Org**：产品已无 org 语义，但代码/schema/RLS/Admin 仍以 `org_id` 为租户轴 → agent 读库必歧义。按 workspace rename 同纪律 **彻底去掉 org 概念**，租户轴改为 **账号所有者 `user_id`（owner）**，资源范围以 **workspace** 为准。

两波可并行启动，但 **Org 波涉及 migration 大改，与 ingestion 热修解耦提交**。

---

## 1. 执行顺序总览

```text
Wave I  (P0, 1–2 本地 commit)   Ingestion 卡住热修 + 终态校验 + 阶段日志
Wave O0 (决策冻结, 同日文档)     Org 目标模型 / 禁词表 / AGENTS 纪律
Wave O1 (schema + RLS)         列/表/策略迁移：org_id → owner_user_id（或等价）
Wave O2 (runtime auth)         AuthContext / JWT / headers / pool GUC
Wave O3 (data plane)           PG repos / Milvus filter / object key / worker
Wave O4 (wire + MCP + FE)      contracts / API / MCP 工具名 / frontend_next
Wave O5 (admin + docs + rg)    Admin 去 organizations；全库禁词验收
```

| 波次 | 依赖 | 默认验证 |
|------|------|----------|
| **I** | 无 | `cargo test -p ingestion --lib`；worker 对该 PDF requeue → `chunk_count>0` |
| **O0** | 无 | 本文 + `AGENTS.md` 节落地 |
| **O1–O5** | O0 冻结后串行 | 每波 `cargo test -p …`；O5 末 `rg` 零命中生产路径 |

**不做**：为 org 保留长期 alias、双 GUC、双 JWT claim、「兼容半年」双路径。与 notebook 决策一致——**未上线可硬切**。

---

# A 部 — Ingestion PDF 卡住修复

## A.1 订正后的根因（证据）

复现对象（诊断时）：

| 字段 | 值 |
|------|-----|
| Workspace | `f57a24e8-fc4a-4edf-872f-ed9841e20ef5` |
| Document | `9b9a1c86-605d-477c-b6b8-d9216ce8aeed`（`2606.10209v1.pdf`，17 页） |

| 证据 | 含义 |
|------|------|
| 日志停在 `PDF page routing plan prepared`（simple_pdf, 17 text pages） | 路由 + LiteParse probe **已成功** |
| `document_parse_runs`：**多行 `running`**（查表须带 **owner 作用域 GUC**，见 A.5） | `create_document_parse_run` **已成功**；超时未 finish |
| `document_blocks`：**1502** paragraph，0 assets | parse + IR project **已成功** |
| `chunks`：**0** | materialize **未 commit** body chunks |
| 主线程高 CPU 数分钟 → `task timeout after 300s` | 同步重 CPU，非 ClamAV / 坏 PDF |

**主因代码**（`avrag-rs/crates/ingestion/src/chunker.rs`）：

- `build_ir_chunk_plan` 对 **每个** text block 调用 `split_text_segments`  
- 每次 `token_chunk_config` → `cl100k_base()` **完整解析 vocab + 建 BPE**（非 singleton）  
- LiteParse 产出 **~1502 micro-blocks**（短句/行级）→ ~1502 次构造  

本地 release 实测量级：

| 操作 | 耗时 |
|------|------|
| `cl100k_base()` ×50 | ~4.1s（~82 ms/次） |
| `cl100k_base_singleton()` ×50 | ~0.055s |
| 1502 × (`cl100k_base` + encode 短串) | **~128s** |

Debug worker 再放大数倍 → 轻松超过 `AVRAG_INGESTION_TASK_TIMEOUT_SECS` 默认 **300**。

**非主因（本 PDF）**：pdf-visual-renderer 未起（`visual_raster_pages=0`）；ClamAV fail-open；坏文件（PyMuPDF ~0.07s 可抽全文）。

原诊断文档 §3.1「卡在 create 前后」**订正为**：create 已成功；卡在 **IR → chunk 的 tokenizer 热路径**。原「无 parse_run」多为 **RLS 未设租户 GUC** 假阴性。

## A.2 修复任务（I 波）

| ID | 任务 | 文件 / 位置 | 验收 | 状态 |
|----|------|-------------|------|------|
| **I1** | `cl100k_base()` → `cl100k_base_singleton()`（sizer 进程内复用） | `crates/ingestion/src/chunker.rs`；`crates/llm/src/summary.rs` 同修 | 单元：1500 micro-blocks &lt; 10s | **Done** |
| **I2** | LiteParse micro-block 按页/行距 coalesce | `crates/ingestion/src/parser/liteparse_ir.rs` | 相邻行合并；heading 分离 | **Done** |
| **I3** | 阶段日志 + elapsed：parse_run create / parse / project / chunk plan / body / index | `document_pipeline/*`、`processor.rs` | 再卡可定位 | **Done** |
| **I4** | 零 text+multimodal chunks → 失败；`processed_chunk_count` 不再 `.max(1)` 假成功 | `materialize.rs`、`mod.rs` | 无假 completed | **Done** |
| **I5** | timeout 后 finish parse_run=`failed`（lease 仍有效时）；advisory 靠 Drop | `processor.rs` | 不再长期残留 `running` | **Done** |
| **I6** | product-dev-up 拉起 pdf-renderer；worker/api tee `.dev-logs/` | `scripts/product-dev-up.sh`、worker runbook | 扫描件可渲；日志落盘 | **Done** |

**明确不做（I 波）**：重写整个 PDF 路由；上真实 OCR 依赖修文本 PDF；扩 CI。

## A.3 最小复现 / 验收

```bash
cd /home/chuan/context-osv6/avrag-rs
set -a && source .env && set +a
bash scripts/pdf-renderer-up.sh   # 扫描件需要；本 PDF 非必须

# 单元
cargo test -p ingestion --lib chunker
# 可选：targeted worker lib if any

# 现场 requeue：优先 API reindex；或诊断文档 §6.4 SQL（列名随 O 波会改）
# 期望：
# - 日志越过 plan prepared，出现 chunk plan / body chunks
# - documents.status=completed 且 chunk_count >= 1
# - parse_run status=completed
```

## A.4 现场运维注意（修前）

- 僵尸 `ingestion_tasks.status=processing`：worker 死后需 requeue 或等 stale lease。  
- 查 derived 表：必须设置 **当前租户 GUC**（O 波前为 `app.current_org`；O 波后见 B.3）。  
- 对象文件路径仍可能含旧 org 段，直到 O3 对象键迁移。

## A.5 诊断查询（当前 schema；O 波后替换）

```sql
-- O 波前：派生表 RLS 按 org
SELECT set_config('app.current_org', '<org_uuid>', false);
SELECT set_config('app.current_role', 'super_admin', false); -- 仅部分策略需要

SELECT id, status, chunk_count FROM documents WHERE id = '<doc_uuid>';
SELECT run_id, status, created_at FROM document_parse_runs WHERE document_id = '<doc_uuid>';
SELECT count(*) FROM document_blocks WHERE document_id = '<doc_uuid>';
SELECT count(*) FROM chunks WHERE document_id = '<doc_uuid>';
```

---

# B 部 — 彻底去掉 Org 概念

## B.1 产品拍板（对标 workspace rename）

| # | 决策 |
|---|------|
| 1 | **产品与代码公开层不再使用 org / organization / OrgId 作为业务概念** |
| 2 | **租户隔离轴** = 账号所有者 **`user_id`（owner）**；**协作/资料范围** = **`workspace_id`** |
| 3 | **删除** `organizations` 表与 Admin「组织」面；运营只认 **users** |
| 4 | **删除** `x-org-id`、JWT `org_id` claim、MCP `org.*` 工具名、`/org/*` 路由 |
| 5 | **存储硬切**：列 `org_id` → `owner_user_id`（全库统一命名，见 B.3）；GUC `app.current_org` → `app.current_user`（与计费 RLS 对齐） |
| 6 | **无长期 alias**（不保留 `org_id` JSON 字段双写）；未上线，允许破坏性 migration |
| 7 | 历史文档可标 deprecated；**新 agent 规则以本文 + AGENTS.md 为准** |
| 8 | 计费已 user 化（0035）→ 乘势拆掉 org 桥接（`get_org_id_by_user_id` 等） |

### 目标心智模型

```text
User (账号 / 计费 / 限流 / RLS 默认主体)
  └── Workspace (资料、成员、API key、分享)
        └── Documents / sessions / chunks / vectors …

AuthContext:
  user_id (required for User/ApiKey subjects)
  workspace_id (optional scope)
  permissions, request_id, …
  // 不再有 org_id
```

**注册**：不再 `INSERT organizations`；`users` 不再 `org_id` FK。  
**个人场景**：以前「1 user : 1 隐藏 org」→ **直接 user 作用域**，去掉无中间层。

## B.2 为何必须做（agent 歧义）

当前并存三套叙事：

| 叙事 | 来源 | 歧义 |
|------|------|------|
| 「产品无 org」 | `api-access-for-agents.md`、B2C ADR | agent 以为可忽略 org |
| 「Auth/RLS 全是 org」 | `AuthContext`、PG RLS、Milvus | agent 每个 SQL 都写 org_id |
| 「Admin 管 organizations」 | admin routes/UI | 以为是 B2B 多租户 SaaS |

结果：诊断写错 GUC、新 API 误加 `org_id`、测试夹具伪造 `org-1`、与 workspace 主语义冲突。

## B.3 命名冻结（O0 必须先锁）

| 旧 | 新（规范） | 说明 |
|----|------------|------|
| `organizations` 表 | **删除** | 无替换表 |
| `org_id` 列（资源表） | **`owner_user_id`** | 资源归属账号；**不是**「操作者」时仍用此列做 RLS |
| `users.org_id` | **删除列** | user 自身即根 |
| `OrgId` / `org_id()` | **`UserId` / `user_id()`**（contracts 已有或补齐） | runtime |
| `app.current_org` | **`app.current_user`** | 与 0035 subscriptions RLS 一致 |
| `x-org-id` | **删除**；鉴权只认 JWT / API key /（测试）`x-user-id` | 禁止新 proxy org header |
| JWT claim `org_id` | **删除**；保留 `user_id` + `auth_version` | |
| MCP `org.create_workspace` | **`workspace.create`**（或现有 `workspace.*` 已覆盖则删） | wire 禁 `org.` 前缀 |
| 路由 `/org/api-keys` | **`/account/api-keys` 或 `/me/api-keys`** | 账号级 key（若仍需要） |
| Admin `/admin/organizations` | **删除**；能力并入 `/admin/users` | |
| Object key `{org}/{ws}/{doc}/…` | **`{owner_user_id}/{workspace_id}/{doc}/…`** | 迁移脚本 rewrite 或双读一波内切完 |
| Milvus `org_id` field | **`owner_user_id`**（或复用现有字段 rename） | filter 同步 |

**禁止**用 `tenant_id` 当「改名 org」继续走私；新代码若需要隔离词，只用 **`owner_user_id` / `user_id` / `workspace_id`**。

## B.4 现状库存（执行时再 `rg` 刷新）

量级（2026-07-10 扫描）：**~250+ 文件** 含 `org_id` / `current_org`（除 target/node_modules）。

| 层 | 代表路径 |
|----|----------|
| Contracts | `contracts/src/auth_runtime.rs`（`OrgId`）、`workspaces.rs` / `documents.rs` / `admin.rs` |
| HTTP | `transport-http` middleware JWT、`routes/admin.rs`、`routes/workspaces.rs` `/org/api-keys` |
| PG | `storage-pg` 几乎所有 repository；`migrations/0001` 起 + RLS `0029` 等 |
| Auth 注册 | `app-bootstrap/.../pg_auth_store.rs` 建 org+user |
| Worker / ingestion | task payload `org_id`；parse_run insert |
| Milvus | `storage-milvus` `doc_filter` org |
| Object store | 路径前缀 org |
| Frontend | `app/admin/organizations/**`、`lib/admin/client.ts`、generated contracts |
| 文档 | `FUNCTIONAL_ACCEPTANCE_CHECKLIST.md`、ingestion 诊断 SQL、多份 plan/archive |

计费：**0035 已 user_id**；仍残留 org 桥接代码需 O 波删。

## B.5 波次任务

### O0 — 纪律与文档（先做）

| ID | 任务 | 验收 | 状态 |
|----|------|------|------|
| **O0.1** | 本文为权威方案；诊断文档指向本文 | 链接互指 | **Done** |
| **O0.2** | `AGENTS.md` §8.3b / T8；`CLAUDE.md` T8 + Org 节 | agent 默认读到 | **Done** |
| **O0.3** | 禁词表写入本文 B.3；PR/提交自检 `rg` 命令 | 见 B.7 | **Done**（表在 B.3/B.7；执行 O5 时跑 `rg`） |

### O1 — Schema + RLS

| ID | 任务 | 验收 |
|----|------|------|
| **O1.1** | 新 migration：资源表 `org_id` → `owner_user_id`（数据：`owner_user_id = users.id` 从旧 org 映射——取该 org 下主用户/最早 admin/唯一 user） | up/down 可逆或标注 irreversible |
| **O1.2** | `users` 去 `org_id`；`DROP organizations CASCADE` 前拆 FK | 无 organizations |
| **O1.3** | 全部 RLS：`owner_user_id = current_setting('app.current_user')` 或 workspace 成员策略（成员访问走 workspace membership，**不**回退 org） | 单测 + 手工 SQL |
| **O1.4** | `ingestion_tasks` / cleanup / audit / usage_events 等同迁 | worker claim 正常 |

**映射规则（1 隐藏 org : 1 user 为默认）**：

```text
owner_user_id := (SELECT id FROM users WHERE users.org_id = <old_org> ORDER BY created_at ASC LIMIT 1)
```

多 user 共 org 的遗留行（若有）：迁移前脚本列出；策略 = 最早 user 为 owner，其余保留为 workspace members（需有 membership 行则 backfill）。

### O2 — Auth runtime

| ID | 任务 | 验收 |
|----|------|------|
| **O2.1** | `AuthContext` 去掉 `OrgId`；`new(user_id, subject_kind)` | contracts 测试绿 |
| **O2.2** | JWT 只签 user；删 `x-org-id` 解析 | 登录/刷新 E2E 或 L1 |
| **O2.3** | `PgPool` session GUC 只设 `app.current_user`（+ role） | RLS 生效 |
| **O2.4** | `ensure_same_org` → `ensure_same_owner` / workspace 授权 API | 无 org 符号 |

### O3 — Data plane + worker

| ID | 任务 | 验收 |
|----|------|------|
| **O3.1** | 全 `storage-pg` SQL 字符串换列名 | `cargo test -p storage-pg` |
| **O3.2** | Milvus schema/filter rename + 重建或 migration 脚本 | 检索仍隔离 |
| **O3.3** | Object key rewrite（或读旧写新一波内删旧） | 上传/ingestion 读对象 OK |
| **O3.4** | Worker task payload / heartbeat / parse_run | 本 PDF + 小文件 ingestion |

### O4 — Wire + Frontend + MCP

| ID | 任务 | 验收 |
|----|------|------|
| **O4.1** | contracts DTO 去 `org_id`；重生 TS | `pnpm` typecheck |
| **O4.2** | MCP 工具目录无 `org.` 前缀 | tools/list |
| **O4.3** | 路由 `/org/*` 删除；账号级 API 新路径 | 无双挂 |
| **O4.4** | 产品 UI 类型与测试夹具去 `org-1` | frontend tests |

### O5 — Admin + 文档 + 清场

| ID | 任务 | 验收 |
|----|------|------|
| **O5.1** | 删除 admin organizations 页与 API；users/usage 只按 user | admin 可列用户 |
| **O5.2** | 更新 `api-access-for-agents.md`、验收清单、runbook；archive 旧 org 叙述 | 新文档零「请设 org」 |
| **O5.3** | 全库禁词 `rg`（B.7）生产路径零命中 | CI 可选 local script，不强制扩 GitHub CI |

## B.6 与 Ingestion 的交叉

| 点 | 处理 |
|----|------|
| 诊断 SQL 里的 `org_id` 列 | I 波可暂用旧列；O1 后改 `owner_user_id` |
| requeue `INSERT ingestion_tasks` | payload 与列随 O1/O3 改 |
| parse_run RLS | O1 改为 `app.current_user`；修 ingestion 时查表用对 GUC |
| 对象路径 | I 波不改路径；O3 统一 rewrite |

**建议 commit 切分**：`fix(ingestion): …` 与 `refactor!: remove org tenant axis` **分开**，避免热修被大 migration 绑死。

## B.7 禁词与验收命令

生产路径（`avrag-rs/{bins,crates}`、`contracts/`、`frontend_next/` 除 archive）在 O5 后应：

```bash
# 应为空（允许 THIRD_PARTY / archive / docs/engineering 本文历史段落）
rg -n '\bOrgId\b|\borg_id\b|\bcurrent_org\b|\bx-org-id\b|\borganizations\b' \
  contracts avrag-rs/bins avrag-rs/crates frontend_next \
  --glob '!**/target/**' --glob '!**/node_modules/**' --glob '!**/generated/**' \
  --glob '!**/archive/**'
```

允许的例外（若必须）：

- 本文与「历史决策」段落中的旧名（加 **历史** 标注）  
- `migrations/*.down.sql` 回滚脚本中的旧列名  
- 一次性 data migration 脚本文件名含 `org` 的 **migrate_from_org_***  

## B.8 风险

| 风险 | 缓解 |
|------|------|
| 共 org 多用户数据 | O1 前 inventory 脚本；最早 user 为 owner |
| Milvus 全量重写 | 维护窗或别名 collection；dev 可 drop 重建 |
| 对象路径 404 | rewrite + 校验抽样；失败任务 requeue |
| Agent 仍读旧文档 | O0 更新 AGENTS；旧 plan 文首加 superseded 链接 |
| 范围膨胀 | 禁止顺手做 C4 / notebook 二次处理；只做 org 轴 |

---

# C 部 — Agent 编码纪律（落地到 AGENTS 时用）

执行 O0 时把下列 **铁律** 写入 `AGENTS.md` / `CLAUDE.md`（与 workspace 节并列）：

1. **禁止** 新代码引入 `org` / `OrgId` / `org_id` / `organizations` / `x-org-id` / `app.current_org`。  
2. 租户 / 归属：**`owner_user_id` 或 `user_id`**；资源范围：**`workspace_id`**。  
3. 鉴权上下文：**`AuthContext` 以 user 为根**，可选 workspace scope。  
4. 若测试或 SQL 仍出现 org： **先迁模型，禁止「对齐回 org」**。  
5. 文档对外只写 personal account + workspace；Admin 只写 users。

---

# D 部 — 完成定义

## D.1 Ingestion（I 波 Done）

- [ ] `build_ir_chunk_plan` 不再 per-block `cl100k_base()` 全量构造  
- [ ] 复现 PDF：`status=completed` 且 `chunk_count >= 1`，parse_run `completed`  
- [ ] 无 `completed + chunk_count=0`  
- [ ] 阶段日志可观测  

## D.2 Org 移除（O 波 Done）

- [ ] DB 无 `organizations` 表；资源表无 `org_id`  
- [ ] 运行时无 `OrgId` / `x-org-id` / JWT org claim  
- [ ] MCP/API/FE 生产路径无 org 命名  
- [ ] Admin 无 organizations 面  
- [ ] B.7 `rg` 验收通过  
- [ ] `AGENTS.md` 铁律已写；本文状态改为 **Done** 并注 commit  

---

## 相关文档

| 文档 | 关系 |
|------|------|
| [`INGESTION_PDF_STUCK_DIAGNOSIS_2026-07-10.md`](./INGESTION_PDF_STUCK_DIAGNOSIS_2026-07-10.md) | 现场诊断；根因以本文 A 为准 |
| [`WORKSPACE_RENAME_DECISIONS_2026-07-09.md`](./WORKSPACE_RENAME_DECISIONS_2026-07-09.md) | 硬切命名先例 |
| [`docs/adr/0001-user-level-billing-b2c.md`](../adr/0001-user-level-billing-b2c.md) | 计费已 user |
| [`frontend_next/public/docs/api-access-for-agents.md`](../../frontend_next/public/docs/api-access-for-agents.md) | 产品「无 org UI」叙述；O4/O5 时改 MCP 段 |
| [`SOLO_DISCIPLINE.md`](./SOLO_DISCIPLINE.md) | 本地 trunk、不扩 CI theater |

---

## 修订记录

| 日期 | 说明 |
|------|------|
| 2026-07-10 | 初版：订正 ingestion 根因（tokenizer × micro-blocks）；冻结 org→owner_user 硬切方案；O0 已写 `AGENTS.md` §8.3b / `CLAUDE.md` T8；诊断文档已回链 |
