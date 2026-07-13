# 金字塔全量重跑 — 残留失败诊断（2026-07-13）

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-13 |
| 触发 | 全量 L1 / DR2 / journey / L2-integration / L3-quality 系统重跑 |
| 主干结论 | **Journey 12/12 · DR2-full · L1 · L3-ui · L3-llm 已绿** |
| 残留 | R1–R3 修复已落地（2026-07-13 同会话）；见文末 §Fix |
| 证据 | `/tmp/pyramid-run/{l1,dr2,journey,l2-integration,quality,quality2}.log` |

---

## 0. 分层结果（本轮）

| 层 | 结果 | 说明 |
|----|------|------|
| L1（含 avrag-storage-pg） | ✅ | claim patho 单测含在内 |
| DR2 `REQUIRE_L3=1` | ✅ DR2-full | L1 + mechanisms + patho + ui-smoke(18) + llm 四模式 |
| L3-journey 一键 | ✅ 12/12 · 9.2m | 含 write / upload-rag / invite |
| L2-integration | ⚠️ 85 pass / 5 fail → 补丁后 4/5 绿 | 见 §1 |
| L3-quality `rag_quality_prod` | ⚠️ 3/5 | 主闸 golden evaluator 绿；见 §2–3 |

已合入测试对齐：`931c707`（MCP `account_*`、quota/usage 不再读 `users.owner_user_id`、preflight 忽略 shell 误匹配）。

---

## 1. L2 — `empty_document_ingests_with_zero_chunks_and_degrades`

### 1.1 现象

- 文档长期 `status=queued`，120s 超时。
- 诊断：`attempt_count=3`，`last_error`：

```text
document locked by another worker: postgres advisory lock held for <doc_id> (key=…); requeue for retry
```

- 之后 e2e worker 日志反复：`worker ingestion poll completed with no tasks`  
  （任务因 fail 退避 `available_at` 在未来，claim 可见行=0）。

### 1.2 证据链

| 步骤 | 事实 |
|------|------|
| Fixture | `fixtures/empty.txt` **0 字节** |
| 队列 | e2e `queue_group=default`，本测用 **ephemeral PG**（非宿主 `avrag_rs`） |
| 锁 | Worker 无 Redis 时用 `pg_try_advisory_lock`（`processor.rs`） |
| 释放 | `PgAdvisoryLockGuard::Drop` 在 **pool 上 spawn 新连接** 调 `pg_advisory_unlock` |

### 1.3 根因（产品 · P0）

**Postgres session-level advisory lock 跨连接错误释放。**

```text
acquire: connection A  → pg_try_advisory_lock(key)  // 锁绑定 session A
drop:    connection B  → pg_advisory_unlock(key)    // 无法解开 A 上的锁
结果:    A 回池后仍持锁 → 后续 try_acquire 全失败 → 任务反复 requeue → 退避后「no tasks」
```

代码位置：`avrag-rs/bins/worker/src/pipeline/processor.rs`（`PgAdvisoryLockGuard`）。

空文档路径更容易触发「claim → 持锁 → 失败/requeue」循环，因此 **L2 empty 稳定复现**；非空文档若一次成功则不暴露。

次要因素（非主因）：

- 宿主 `avrag-worker` 与 e2e 若 **同库同 queue_group** 会抢任务；本测用 ephemeral PG，主因仍是 **同进程池内锁泄漏**。
- Journey 上传非空 fixture 不走该边角，故 journey 可绿而 L2 empty 红。

### 1.4 修复方向

1. **同一连接**上 acquire + unlock（专用 `PoolConnection` 持有 guard 生命周期）；或  
2. 改用 **transaction-scoped** `pg_advisory_xact_lock`（事务结束自动释放）；或  
3. Redis document lock 为唯一路径（本地 e2e 必须配置或强制单连接 advisory）。

验证：`cargo test -p app --test product_e2e empty_document_ingests --features product-e2e`（E2E_MODE=integration，无宿主 worker 抢同库）。

---

## 2. L3-quality — `realistic_corpus_full_eval`

### 2.1 现象（quality2 重跑）

```text
create notebook failed: HTTP 500
postgres error: new row violates row-level security policy for table "users"
```

卡在 `create_workspace("rag-quality-realistic-corpus")`（`rag_quality_prod.rs` ~482），**尚未进入上传**。

### 2.2 对比

| 用例 | 建 workspace | 结果 |
|------|----------------|------|
| `production_rag_evaluator…` | 共享 / 既有 fixture 路径 | ✅ |
| `rag_system_prompt_smoke_v5` | `shared_smoke_v5_context` + persistent infra | ✅ |
| `realistic_corpus_full_eval` | `TestContext::new_with_real_llm_pdf()` → 立即 `create_workspace` | ❌ users RLS |

### 2.3 根因（产品/E2E 配置 · P1）

`create_workspace` → storage `ensure_user_and_actor`：

- 在 **已 set `app.current_user = owner`** 的事务里 `INSERT users`。
- **WITH CHECK** 要求 `id = current_user` **或** admin 角色。
- 当 `AuthContext` 的 **actor_id ≠ user_id** 时，会对 actor **再 insert 一行**，该行 **不满足** `id = current_user` → **RLS 500**。

与 storage-pg patho 单测曾踩的坑同类（双 UUID owner/actor）。

首轮 quality 曾越过 create、在 **upload thesis… 后 queued 超时**——那是 **worker/锁/队列** 另一面（与 §1 同源或 queue 退避）；重跑时更早死在 create，说明 **create 路径本身也不稳**。

### 2.4 修复方向

1. E2E / JWT：**B2C 个人账号 actor ≡ user**（推荐默认）；或  
2. `ensure_user_and_actor`：插入 actor 时 **临时 super_admin**；或  
3. 注册接口已写 users 后，create_workspace **禁止再 insert 非 self 用户**。

验证：`E2E_MODE=nightly cargo test -p app --test product_e2e realistic_corpus --features product-e2e -- --ignored --test-threads=1`。

---

## 3. L3-quality — `triplet_benchmark_huawei_ipd`

### 3.1 现象

```text
assertion failed: benchmark requires RAG_SMOKE_SINGLE_DOC=huawei_ipd_370_activities.txt
left: ""  right: "huawei_ipd_370_activities.txt"
```

（`TRIPLET_BENCHMARK_MODEL` 已用 env 补齐后仍因 **SINGLE_DOC 未设** 失败。）

### 3.2 根因（配置 / 门禁 · 非产品回归）

该测 **故意** 要求专用 env（见测试注释与 `scripts/benchmark_triplet_models.sh`）：

| 变量 | 用途 |
|------|------|
| `TRIPLET_BENCHMARK_MODEL` | 必需 |
| `RAG_SMOKE_SINGLE_DOC=huawei_ipd_370_activities.txt` | 单文档语料 |
| `RAG_QUALITY_SMOKE_FORCE_INGEST=1` | 强制灌库 |
| `RAG_QUALITY_SMOKE_TRIPLET_ENABLED=1` / triplet LLM | 图/三元组 |

`test-l3-quality.sh` **只**跑 `llm_real::rag_quality_prod` 过滤器，会 **捞到** 该 `#[ignore]` 测，但 **不注入** 上述 env → **必然失败**。

### 3.3 修复方向

1. **脚本**：`test-l3-quality.sh` 默认 **排除** `triplet_benchmark_*`；或  
2. **过滤器**拆成：`production_rag_evaluator|rag_system_prompt|rag_tools` vs 可选 `triplet_*`；或  
3. 文档标明 quality 一键 **不含** triplet benchmark（需 `benchmark_triplet_models.sh`）。

---

## 4. 已关闭的「假残留」（本轮已处理）

| 项 | 处理 |
|----|------|
| MCP `org_key_*` 错误码 | 产品已是 `account_*`；测试对齐 `931c707` |
| quota/usage `users.owner_user_id` | B2C 无该列；测试改 `id` 作 owner |
| quality preflight 误杀 | shell cmdline 含 `avrag-worker` 字样被 `pgrep -af` 命中；preflight 已收紧 |

---

## 5. 严重度与优先级

| ID | 问题 | 严重度 | 阻塞 |
|----|------|--------|------|
| R1 | Advisory lock 跨连接释放 | **P0 产品** | L2 empty；潜在生产并发 ingest 卡死 |
| R2 | ensure_user actor 双写 RLS | **P1 产品/E2E** | realistic_corpus；部分冷启动 create workspace |
| R3 | quality 脚本吞 triplet 专用测 | **P2 门禁** | 一键 quality 假红 |
| R4 | empty 文档业务语义 | P3 | 依赖 R1 后再验 zero-chunk degrade |

**不阻塞**：日常 L1、准部署 DR2-full、L3-journey 主路径。

---

## 6. 建议执行序

```text
1. 修 PgAdvisoryLockGuard 同连接释放（R1）
   → 复跑 empty_document + 抽样多文档并发 ingest
2. 修 ensure_user_and_actor / E2E actor≡user（R2）
   → 复跑 realistic_corpus_full_eval
3. 收窄 test-l3-quality.sh 过滤器（R3）
   → 一键 quality 只含 production + smoke_v5 + tools
4. （可选）triplet 走 benchmark 脚本，不进默认 quality
```

---

## 7. 一句话结论

> 主干金字塔（L1 / DR2-full / journey）已绿。  
> 残留红灯 **不是** journey/claim 回归，而是：  
> **(1) worker advisory lock 释放错误 → 空文档 L2 死锁式 requeue；**  
> **(2) B2C 后 ensure_user 对 actor 二次 insert 撞 RLS → realistic 建 workspace 500；**  
> **(3) quality 脚本误跑需专用 env 的 triplet 基准测。**  
> 修 R1+R2+收窄 quality 过滤即可收口本轮系统债。

---

## §Fix (2026-07-13 execution)

| ID | 改动 |
|----|------|
| R1 | `PgAdvisoryLockGuard` 持 `PoolConnection`，同 session `pg_advisory_unlock`（`block_in_place`） |
| R2 | `ensure_user_and_actor` 对 actor≠owner 插入时本地 `super_admin`，插入后清空 role |
| R3 | `test-l3-quality.sh` 增加 `--skip triplet_benchmark` |

