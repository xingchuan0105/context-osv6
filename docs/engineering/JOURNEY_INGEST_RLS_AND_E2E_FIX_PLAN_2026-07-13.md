# Journey 失败修复计划（Ingest RLS · 测试契约 · Org 残留）

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-13 |
| 状态 | **Done (journey core)** — W1–W4 + invite/write/pdf/citation/analyze + patho claim；本地 11 journey 已绿（2026-07-13） |
| 触发 | `bash scripts/test-l3-journey.sh`：**3 passed / 9 failed**（31.5 min）；上传类文档永久「排队中」 |
| 诊断结论 | Worker `claim_next_ingestion_task` 走 raw pool **无 RLS 抬权** → 强制 RLS 下可见行=0；叠加 workspace 响应形状 / history testid / org E2E 钩子过期 |
| 约束 | Solo local trunk；**不**默认扩 CI；修产品 P0 优先于扩测试；与 T8「无 product org」一致 |
| 上游 | Journey 整轮诊断（会话）；[`L3_TEST_INTEGRATION_AND_CORPUS_PLAN_2026-07-13.md`](./L3_TEST_INTEGRATION_AND_CORPUS_PLAN_2026-07-10.md)；[`ACCEPTANCE_PYRAMID_STABILIZATION_PLAN_2026-07-10.md`](./ACCEPTANCE_PYRAMID_STABILIZATION_PLAN_2026-07-10.md)；org 删除 `4add8d7` |
| 关联 | `ingestion_tasks` 策略 `tenant_isolation_ingestion_tasks`（`relforcerowsecurity=t`） |

---

## 0. 问题陈述

### 0.1 现象（2026-07-13 跑次）

| 结果 | 用例 |
|------|------|
| ✅ | web search · workspace-crud · workspace-share |
| ❌ ingest 超时 | upload-rag · upload-pdf-rag · citation-interaction · analyze-workflow |
| ❌ API 形状 | chat-session · session-history（`notebook.notebook.id`） |
| ❌ org 钩子 | invite-collaboration（`ensure-org-member` 404 owner not found） |
| ❌ UI 断言 | workspace-chat general（`query-library-panel`） |
| ❌ write 完成态 | workspace-write（540s 无 assistant 气泡） |

### 0.2 根因分层

```text
P0 产品  Worker claim 无 super_admin/GUC
         → RLS 过滤 ingestion_tasks → poll forever "no tasks"
         → 文档 status=queued / UI「排队中」

P0 测试  createWorkspaceViaAPI 返回 { workspace.id }
         调用方仍用 notebook.notebook.id

P1 产品+测试  ensure_e2e_org_member / users 查询同样受 RLS；
               invite 仍 org 语义，与 B2C 冲突

P1 测试  history 断言 query-library-panel（UI 已是 history-item）

P2 产品/稳定性  write 流式完成态（done / 可见气泡）
```

### 0.3 非目标

- 重写全部 Playwright journey  
- 把 quality / smoke_v5 塞进 journey  
- 取消 `ingestion_tasks` RLS（应用 **抬权 claim**，不是拆策略）  
- 恢复 product `org` 作为租户模型  

---

## 1. 目标与成功标准

### 1.1 目标

1. **本地 REUSE worker 能 claim `queue_group=default` 的 queued 任务**（与 API 入队一致）。  
2. Journey **上传→completed→RAG** 主路径可绿（至少 `workspace-upload-rag`）。  
3. 测试契约对齐 **workspace** 与当前 history UI。  
4. Invite E2E 不依赖 org member 钩子（或钩子在 super_admin 下可靠 provision 个人账号）。  
5. Write 旅程：有明确完成/失败信号，避免无文案干等 9 分钟。  

### 1.2 DoD

| ID | 标准 | 验证 |
|----|------|------|
| D1 | 无 GUC 时 `SELECT count(*) FROM ingestion_tasks WHERE status='queued'`=0；claim 路径抬权后 worker 日志出现 claim/processing | 手工 SQL + worker log |
| D2 | 积压 `queued/default` 任务被消费或明确 dead_letter | `psql` + super_admin |
| D3 | `pnpm exec playwright test --project=journey e2e/specs/journey/workspace-upload-rag.spec.ts` 绿 | 单测 |
| D4 | chat-session / session-history 不再 `undefined.id` | 单测 |
| D5 | invite 不再 404 `owner not found`（或 skip 并登记债） | 单测 |
| D6 | general chat 历史断言用 `history-item` / rail | 单测 |
| D7 | （stretch）`test-l3-journey.sh` ≥ 上传+CRUD+share+search 绿；全 12 绿为理想 | 全量 |

---

## 2. 修复设计

### 2.1 P0-A — Worker / 队列 claim 抬权（产品）

**原则：** 后台 claim 是 **跨租户调度**，必须 `super_admin`（或等价 BYPASS），与 orphan cleanup 已有模式对齐（`orphan_object_jobs.rs` 已 `set_config super_admin`）。

| 位置 | 改动 |
|------|------|
| `claim_next_ingestion_task` | 在 **同一事务** 内先 `set_config('app.current_role','super_admin', true)`，再 CTE claim；或 `begin` + role + claim + commit |
| `complete` / `fail` / `renew_lock`（若 raw + RLS） | 同样抬权，避免 processing 后写回失败 |
| `claim_next_document_cleanup_task` 等 | 审计：凡 raw pool 扫全局队列表，一律抬权 |
| 单元/集成 | storage-pg 或 worker 测：插入 queued 任务（owner=其他 user）→ 无 GUC 的普通 select 0 行 → claim 抬权后拿到任务 |

**推荐实现形态（二选一，优先 1）：**

1. **Claim 内置抬权**（深模块）：`repository_ingestion_queue.rs` 的 claim/complete/fail 内部开事务并 `set_current_role(..., "super_admin")`，调用方无感。  
2. Worker 入口统一 `WorkerDbSession::with_admin` 包装（浅包装，易漏路径）。

**禁止：** 在应用层关闭 RLS；禁止 journey 测试绕过改成 mock ingest 掩盖 P0。

**积压处理：** 修复部署后 worker 应自动吃掉现有 `queued/default`；可选运维脚本列出/重试 dead_letter。

### 2.2 P0-B — E2E API helper 与 wire（测试）

| 文件 | 现状 | 目标 |
|------|------|------|
| `createWorkspaceViaAPI` | 返回 `{ workspace: { id } }` | 保持 |
| `chat-session.spec.ts` | `notebook.notebook.id` | `workspace.workspace.id` 或 `const { id } = (await create...).workspace` |
| `session-history.spec.ts` | 同上 | 同上 |
| 其他 `notebook.` 引用 | grep 清扫 | 一律 workspace |

可选：helper 增加 `workspaceIdFromCreateResponse(body): string`，兼容过渡期 `notebook` 别名（只读不写）。

### 2.3 P1-A — ensure_e2e_org_member / invite

| 步骤 | 内容 |
|------|------|
| 1 | `ensure_e2e_org_member` 内所有跨用户 SQL 使用 **super_admin 事务**（与 claim 同模式） |
| 2 | 语义改名（可选）：`ensure_e2e_collaborator_user` — 只保证 member 个人账号存在+legal，**不**写入 org |
| 3 | Invite 测：owner invite email → member 登录 accept；断言走 `workspaces/.../members` |
| 4 | 若产品 invite 仍依赖已删 org 字段：先修 product invite API，再绿测 |

### 2.4 P1-B — History UI 断言

| 文件 | 改动 |
|------|------|
| `workspace-chat.spec.ts` general | 去掉 `query-library-panel`；改为 `desktop-history-rail` / `history-item` 可见且含 runId 或会话标题 |
| `workspace-page.ts` | `getQueryLibraryPanel` 标记 deprecated 或改为 history helpers |

### 2.5 P2 — Write 旅程

| 步骤 | 内容 |
|------|------|
| 1 | 确认 UI write 模式是否仅在 `done` 后挂 assistant 气泡；与 SSE 契约对齐 |
| 2 | 产品：异常路径必发 `error` 或 `done`（禁静默断流） |
| 3 | 测试：超时前检查 error toast / progress card；必要时缩 topic 或加 soft 完成条件 |
| 4 | 已修 `web_fetch` CJK 截断（`77883a8`）— 回归写路径时确认无 panic |

### 2.6 观测 / 防回归

| 项 | 内容 |
|----|------|
| Worker 日志 | claim 成功打 `stage=claim document_id= queue_group=` |
| `dev-stack-check` 可选扩展 | `queued` 任务在 super_admin 下 >0 且 worker 心跳后 N 秒应减少（可选，防静默空转） |
| L1/L2 | storage-pg claim 单测进 L1 或 L2-patho（`patho_ingest_claim_sees_cross_owner_queue`） |

---

## 3. 实施波次

### W1 — P0 产品 claim 抬权（0.5–1d）— **阻塞 Journey 上传**

- [ ] `claim_next_ingestion_task`（及 complete/fail/renew 若需要）事务内 super_admin  
- [ ] 审计其他 raw 全局 claim  
- [ ] 单测：跨 owner 任务可被 claim  
- [ ] 本地：重启 worker → 积压 queued 被消费  
- [ ] 验证：`workspace-upload-rag.spec.ts` 单文件绿  

**验收：** worker 日志有 processing；UI 文档 completed。

### W2 — P0/P1 测试契约（0.25–0.5d）

- [ ] workspace id 调用方全量 grep 修复  
- [ ] history 断言修复  
- [ ] 验证：chat-session、session-history、workspace-chat general  

**验收：** 三测不再 0.3s TypeError / 错误 testid。

### W3 — Invite 去 org（0.5–1d）

- [ ] ensure helper 抬权 + 语义清理  
- [ ] invite journey 对齐 workspace 邀请流  
- [ ] 验证：invite-collaboration  

### W4 — Write 与全量 journey（0.5–1d）

- [ ] write 完成态/超时策略  
- [ ] `bash scripts/test-l3-journey.sh` 全量  
- [ ] 更新本计划状态 → Done；在 L3 计划中链回  

**理想：** 12 绿；底线：上传 RAG + CRUD + share + search + history/session 绿，write/invite 登记 residual。

---

## 4. 风险与回滚

| 风险 | 缓解 |
|------|------|
| super_admin claim 权限过大 | 仅 worker/后台连接串使用；应用用户连接仍走 RLS |
| 抬权遗漏 complete 导致 processing 卡死 | complete/fail 与 claim 同事务模式；加 patho 测 |
| 积压任务突然全部执行打爆 CPU | 先观察 claim 速率；必要时限流 max_attempts |
| Invite 产品仍缺 API | W3 可 skip + issue，不阻塞 W1 |

回滚：revert claim 抬权 commit；worker 恢复旧二进制（上传仍红，与今一致）。

---

## 5. 验证命令

```bash
# 修 W1 后
cd avrag-rs && cargo test -p avrag-storage-pg --lib claim  # 或新增 patho 名
# 重启 worker
# 观察：
#   psql … set super_admin; select status,count(*) from ingestion_tasks group by 1;

cd frontend_next
pnpm exec playwright test --project=journey e2e/specs/journey/workspace-upload-rag.spec.ts
pnpm exec playwright test --project=journey e2e/specs/journey/chat-session.spec.ts
pnpm exec playwright test --project=journey e2e/specs/journey/session-history.spec.ts
pnpm exec playwright test --project=journey e2e/specs/journey/workspace-chat.spec.ts

# 全量
bash scripts/test-l3-journey.sh
```

---

## 6. 文件触点清单（预期）

| 区域 | 路径 |
|------|------|
| Claim 抬权 | `avrag-rs/crates/storage-pg/.../repository_ingestion_queue.rs`（及 cleanup claim 若同类） |
| 参考实现 | `avrag-rs/bins/worker/src/orphan_object_jobs.rs`（已有 super_admin） |
| E2E helper | `avrag-rs/crates/app-bootstrap/.../e2e_upload_helpers.rs` |
| Journey 测 | `frontend_next/e2e/specs/journey/{chat-session,session-history,workspace-chat,invite-collaboration}.ts` |
| POM | `frontend_next/e2e/pom/workspace-page.ts` |
| API helpers | `frontend_next/e2e/utils/api-helpers.ts` |
| 可选 patho | `avrag-rs/crates/storage-pg` 或 worker 测 `patho_claim_*` |

---

## 7. 与金字塔关系

| 层 | 本计划影响 |
|----|------------|
| L1/L2-patho | 可增加 claim RLS patho，防再回归 |
| L3-thin-ui / thin-llm | **不依赖**本修复即可绿（已验证） |
| L3-journey | **依赖 W1** 才能作为 DR3 UI 门禁 |
| DR2 默认 | 仍不含 journey；本计划不改 DR2 范围 |

---

## 8. 建议拍板

| # | 选项 | 建议 |
|---|------|------|
| 1 | Claim 抬权实现位置 | **队列 repository 内置 super_admin 事务**（推荐） |
| 2 | Invite | **先抬权 helper + 个人账号**；产品缺 invite 则 skip 并登记 |
| 3 | Write | W1–W2 后单独修；不阻塞 upload-rag 关门 |
| 4 | 执行顺序 | **严格 W1 → W2 → W3 → W4** |

---

## 9. 一句话

Journey 红的主因是 **强制 RLS 下 worker 裸 claim 看不见全站队列**；修好抬权后上传类应大面积转绿，再用小补丁对齐 **workspace id / history testid / invite**，最后处理 write 完成态。
