# TN 代码质量整改交接文档（2026-07-09）

| 字段 | 值 |
|------|-----|
| 状态 | **Done** — Wave 0–6 + P0–P7 + **TN-2**（删 atomic_tools；list_catalog_tools；workspace envelope；disclosure_catalog） |
| 分支 | 本地 `master`（solo trunk） |
| 范围 | `avrag-rs` / `contracts` / `frontend_next` / scripts（**不含** `frontend_rust`） |
| 主方案 | [`TN_CODE_QUALITY_REMEDIATION_2026-07-09.md`](./TN_CODE_QUALITY_REMEDIATION_2026-07-09.md) |
| 产品词 | ADR-0006 **§5a**：Capability / Skill / Tool **三层保留** |

---

## 1. 一句话进度

**整改关闭。** 工具执行单点 `ToolCatalog`；Capability 披露仅 mode `tool_pool`；`app-chat` 仅 orchestrator；Bound 面拆分；体积热点拆分；`/workspaces` 与 `/notebooks` 双挂；analysis IO 并行。

---

## 2. 已完成（摘要）

| 波次 | 状态 |
|------|------|
| Wave 0 文件门禁 | Done |
| Wave 1–2 工具单点 + ExecutePlan 清零 | Done |
| Wave 3 AppState Bound 面 | Done（`bound/` 分文件） |
| Wave 4 幽灵 ports + AgentRequest 强类型 | Done |
| Wave 5 workspace_id + Rag ports | Done |
| Wave 6 agent-tools / agent-loop | Done |
| P0 allowlist | Done |
| P1 C1/C2/C3 | Done（**C4 否定**） |
| P2 e2e_upload_helpers + bound 拆分 | Done |
| P3 document_pipeline / token_budget 拆分 | Done |
| P4 frontend client/admin/tool-result | Done |
| P5 `/workspaces` 双挂 | Done |
| P6 analysis `tokio::join!` | Done |
| P7 本文档收口 | Done |
| **TN-2.1** 删除 `atomic_tools` | Done |
| **TN-2.2** `list_catalog_tools` + rag auto_fallback 披露 | Done |
| **TN-2.3** workspace JSON envelope + typeshare | Done |
| **TN-2.4** progressive `disclosure_catalog` | Done |
| **W2** AppState Bound 纪律（admin_ops / share.check_access） | Done |

---

## 3. 验证命令

```bash
bash scripts/check_file_size_limits.sh
cd avrag-rs
cargo test -p agent-tools --lib
cargo test -p agent-loop --lib
cargo test -p app-chat --lib
cargo test -p app-bootstrap --lib
cargo test -p transport-http --lib
cargo test -p app --lib
# frontend
pnpm -C frontend_next exec tsc --noEmit
```

---

## 4. 产品 / 工程约定（勿回退）

| 主题 | 说明 |
|------|------|
| Capability ≠ Skill ≠ Tool | ADR-0006 §5a；禁止「注册表合并为 1」 |
| 执行单点 | `ToolCatalog` + `dispatch_tool` only（**无** atomic_tools） |
| Capabilities API | 披露 = mode `tool_pool` ∪ `auto_fallback.tool_id`；全表见 `list_catalog_tools` |
| JSON 信封 | 写 `workspace(s)`；读兼容 `notebook(s)` |
| Handler | Bound 面：`docs()` / `admin_api()` / `share()` / `prefs()` / `billing_api()` |
| URL | `/workspaces/*` 产品默认；`/notebooks/*` 兼容同 handler |
| 不恢复 ExecutePlan | |
| Solo | 默认本地 trunk；定向测试 |

---

## 5. 可选未做 / 非目标

| 项 | 说明 |
|----|------|
| C4 | **明确不做**（产品分层） |
| `frontend_rust` | 范围外（契约字段改动时需自行对齐） |
| TN-2 B5–B10 | profile Value、测试巨石、crate 再瘦、admin ops 真共享… |
| CI 挂 file-size gate | solo 默认定向本地 |

---

## 6. 变更记录

| 日期 | 说明 |
|------|------|
| 2026-07-09 | Wave 0–6 主体 + 再收口 |
| 2026-07-09 | 产品补裁 §5a；P0–P7 可选全量落地；handoff **Done** |
| 2026-07-09 | **TN-2**：删 atomic_tools；list_catalog_tools；workspace envelope；disclosure_catalog |
