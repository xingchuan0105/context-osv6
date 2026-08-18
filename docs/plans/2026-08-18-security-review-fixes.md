# W0–W5 审查修复计划（2026-08-18）

**状态**: F1–F3 已落地（决策按 §0 推荐；`--lib` 验证已过）  
**父计划**: [`2026-08-18-security-remediation-plan.md`](2026-08-18-security-remediation-plan.md)（W0–W5 代码在工作区；P2-14 仍延期）  
**审查**: 两轴（Standards / Spec）对 `stash@{0}` 落地后的工作树  
**原则**: 无兼容税；修缺口不扩范围；不复活已删的 `content_guard.rs`；不抽未要求的第二套 spawn API。

工作区已从 stash 恢复，**不再做 pop**。`desktop/src-tauri/src/commands/secret_fs.rs` 仍是未跟踪文件，必须与本波一起纳入提交。

---

## 0. 拍板项

| # | 决策 | 推荐 | 备选 |
|---|---|---|---|
| D1 | W4 cookie `Secure` | **维持 https-only**（`http://localhost` 否则 cookie 设不上）；改原计划 W4-A 一句与实现一致 | 一律 `Secure`（本地 HTTP 会话坏掉） |
| D2 | W2-B degrade trace | **PackGate `reasons` 加 `intake_redacted`**；不进用户气泡；不写 `DegradeTraceItem` 第二条通道 | 复活 `content_guard.rs`（否决：死代码） |
| D3 | `retrieval-data-plane` `doc_ids: None` 夹具 | **不改**（stub 断言的是 `search_graph` not implemented，不是 SQL） | 改成空 hits（测错语义） |
| D4 | 抽 `spawn_sandboxed_python` | **本波不抽**（Unix fd-bridge 与 Windows TCP+Job 不是同一条 spawn） | 本波合并两条 OS 路径（工期大，缺口不在 spawn 重复） |

未点名则按「推荐」。

---

## 1. 不做

| 项 | 原因 |
|---|---|
| P2-14 解析器隔离 | 父计划延期，需独立执行环境 |
| 抽公共 Python spawn / 合并两份 import hook | D4；F1 只补 bridge `user_ns` |
| 合并两份 `ensure_test_upload_signing_secret` | 两 crate 测试辅助，抽共享会加依赖 |
| 改 `retrieval-data-plane` stub 夹具 | D3 |
| 一律 cookie `Secure` | D1 |
| 再 stash / 与 `f933b609` cache 提交混在一起 | 卫生 |

---

## 2. 波次

```
F0 文件在场确认     ──已完成
         │
         ▼
F1 安全闭合          ──已落地（SQL 死枝 + bridge user_ns + 检测器字面 + PATH）
         │
         ▼
F2 原计划测试门      ──已落地（W1 importlib.subprocess / rlimit 探针 / W3 share+None / PackGate 理由）
         │
         ▼
F3 卫生与条文        ──已落地（回退纯 rustfmt；W4-A https-only Secure）
```

F1 不过不进 F2。F3 可与 F2 同会话，但不挡 F1 验证。

---

## 3. F1 — 安全闭合

**目标**: 审查里仍能走到的旧路径关掉。产品在密钥/doc_ids 配好后行为不变。

### F1-A · 删 `with_doc_ids=false` 死枝

**现状**: `search_bm25_cjk` 已固定传 `true`，但 `build_cjk_bm25_query(..., false)` 仍拼「只滤 owner、无 `doc_id = ANY`」；测试还断言该形状（`storage-pgvector/src/search.rs`）。

**改法**: 去掉 bool。SQL **永远** `AND doc_id = ANY($…)`。测试只保留 `true` 形状。graph 侧若还有平行 bool，同样删。

**文件**: `storage-pgvector/src/search.rs`（及 `graph.rs` 若仍有）。

### F1-B · bridge wrapper 独立 `user_ns`

**现状**: `execute` 已 `exec(..., _user_ns)`；`build_bridge_sandbox_wrapper` 把用户代码缩进进 `__avrag_main`，与 wrapper 同模块 globals（`sys` / `threading` / prelude 可见）。

**改法**: 用户代码在独立 dict 里 `exec`/`eval`；只注入 bridge 需要的名字（如 `client` / shim 导出）。**不**把 `sys`、prelude 模块放进 user_ns。Hook 仍装在 `__builtins__`（纵深）。不抽跨 OS 的 spawn 函数（D4）。

**文件**: `code-interpreter/src/bridge.rs`（wrapper 字符串）；`lib.rs` 的 execute 路径不回退。

### F1-C · 中英高危字面同一组

**现状**: `untrusted_input.rs` 有「你现在是」「系统提示」等；`prompt_injection.rs` jailbreak 正则覆盖「忽略…指令」但缺这两条。

**改法**: `guardrails` 导出一组中英 **substring 常量**（已有 `agent-loop → avrag-guardrails`）。`untrusted_input` 引用该常量做 `contains`；regex 侧用同一组字面拼接或额外 alternation。禁止两边各维护一份差不多的中文。

**文件**: `guardrails/src/input/prompt_injection.rs`、`agent-loop/src/untrusted_input.rs`。

### F1-D · 去掉 `/usr/bin` PATH 兜底

**现状**: `apply_sandbox_env` 在 `python_path` 无 parent 时 `unwrap_or_else(|| "/usr/bin")`。调用方已 `resolve_python_executable`，此枝是死兜底。

**改法**: 无 parent → 设空 `PATH`（或不设），不发明系统目录。

**文件**: `code-interpreter/src/lib.rs`。

**验证**（先问，约 2–4 min）: `cargo test -p avrag-storage-pgvector --lib`；`cargo test -p avrag-code-interpreter --lib`；`cargo test -p avrag-agent-loop --lib` 中 pack/untrusted；`cargo test -p avrag-guardrails --lib`。

---

## 4. F2 — 原计划测试门

补的是父计划已写、落地时漏掉的门，不发明新语义闸。

| 门 | 断言 | 落点 |
|---|---|---|
| W1 `importlib.import_module('subprocess')` | 失败（`import importlib` 不够） | `code-interpreter` `--lib` |
| W1 Unix rlimit | 能稳定则超 `memory_limit_mb` 被拒/被杀；不能稳定则探针 `setrlimit` 已成功，**不要**写 flaky 分配测 | 同上；Windows 不写 Unix 探针 |
| W2-B redact → PackGate | 中英注入样本 redact 后 `reasons` 含 `intake_redacted` | `evidence_pack.rs` 单测（已有中英 redact 则只加 reason） |
| W3 share + `doc_ids=None` | 0 hits；带本 workspace id → 仅这些文档 | `storage-pgvector` 已有 owner/`None` 测的，补 **share 重映射身份** 一条；不改 retrieval-data-plane stub（D3） |

**验证**（先问，约 2–5 min）: 同上 crate `--lib`，外加 pgvector 测（有 PG 才跑，按现 skip 规则）。

---

## 5. F3 — 卫生与条文

### F3-A · 回退纯 rustfmt

`git diff HEAD -- <file>` 若只是换行/import 顺序（如 `pg_auth_store.rs` 的 `ProfileRow` 拆行、`memory.rs` 删空行）→ `git checkout HEAD --` 那些文件。有行为改动的（W0 上传、E2E `TRUST_PROXY_AUTH`）留下。

**已回退**（纯 wrap/import）：`adapters/mod.rs`、`object_store.rs`、`pg_admin_store/mod.rs`、`pg_auth_store.rs`、`pg_billing_store.rs`、`pg_desktop_token_store.rs`、`pg_document_store.rs`、`pg_provider_secret_store.rs`、`pg_referral_store.rs`、`pg_share_store/mod.rs`、`pg_wallet_store.rs`、`app_state/state_methods.rs`、`product_apps/desktop.rs`、`product_apps/prefs.rs`、`services/email_copy.rs`、`services/password_reset.rs`、`app-chat/.../memory.rs`。

**留下**: `app_state/e2e_upload_helpers.rs`（W0-B object_path + HMAC）。

**不要**对整个 `app-bootstrap` 盲 checkout。

### F3-B · 修订原计划 W4-A

改成：`SameSite=Lax`；`Secure` **仅 `https:`**（本地 HTTP 不能带 Secure）。与 `server-session.ts` / 未跟踪的 cookie 测试一致。

### F3-C · 未跟踪文件

`secret_fs.rs`、本文件、父计划、审计文档、`server-session.test.ts` 随安全提交一起纳入；**不**纳入 `.zcode/`、`desktop/runtime/mingw/`、`scripts/fix-blog-canonical.sh`。

**验证**: `git diff --stat` 不再含纯 wrap 文件；`pnpm exec vitest run tests/auth/server-session.test.ts`（约 30s，先问）。

---

## 6. 每波门

| 波 | 过门 |
|---|---|
| F1 | CJK SQL 无「无 doc_id」枝；bridge 用户 ns 看不到 `sys`；两边检测器含同一组中文；PATH 无 `/usr/bin` 兜底 |
| F2 | `import_module('subprocess')` 失败；share+`None` → 0 hits；redact 记 `intake_redacted` |
| F3 | 纯 fmt 已回退；W4-A 条文与 https-only 实现一致 |

不跑 full-149。结构改动后同会话 `code-review-graph update`。

---

## 7. 建议提交（本地 trunk，不推）

父计划 W0–W5 仍未提交。推荐 **一次** `security: fail-closed W0–W5 plus review closures`，或仍按父计划 6 粒 + 本文件 F 波一粒。F 波不单独先合、把死枝留在 W3 提交里。
