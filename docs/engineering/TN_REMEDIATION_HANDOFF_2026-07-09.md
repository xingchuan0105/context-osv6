# TN 代码质量整改交接文档（2026-07-09）

| 字段 | 值 |
|------|-----|
| 状态 | **TN-3 S4 Done**（P0–P5 产品拍板项已落地） |
| 分支 | 本地 `master`（solo trunk） |
| 范围 | `avrag-rs` / `contracts` / `frontend_next` / scripts（**不含** `frontend_rust`） |
| 主方案 | [`TN_CODE_QUALITY_REMEDIATION_2026-07-09.md`](./TN_CODE_QUALITY_REMEDIATION_2026-07-09.md) |
| TN-3 plan | [`TN3_P0_P5_AND_TEST_PYRAMID_PLAN_2026-07-09.md`](./TN3_P0_P5_AND_TEST_PYRAMID_PLAN_2026-07-09.md) |
| 产品词 | ADR-0006 **§5a**：Capability / Skill / Tool **三层保留** |

---

## 1. 一句话进度

**TN 主线 + TN-3 S4 已关。** 工具执行单点 `ToolCatalog`；Bound 面已拆；workspace 命名主体完成；UserProfile 强类型 merge；测试金字塔入口就绪（日常 L1）。

---

## 2. 已完成（摘要）

| 波次 | 状态 |
|------|------|
| Wave 0–6 + P0–P7 + TN-2 | Done |
| W1–W6 结构债 | Done |
| R1–R3 扫尾 | Done |
| **TN-3** | **Done** — P0–P4 结构 + P5 金字塔（入口/盘点/去重/L1 测时） |

---

## 3. 日常验证（Solo / L1）

```bash
bash scripts/check_file_size_limits.sh
# 推荐入口（落地后）：
# bash scripts/test-l1.sh
cd avrag-rs
cargo test -p agent-tools --lib
cargo test -p agent-loop --lib
cargo test -p app-chat --lib
# 改到的 crate 再定向加测
pnpm -C frontend_next exec tsc --noEmit
```

**不进日常默认**：真 LLM、Playwright 全旅程、rag_quality、性能基线（L3 / nightly）。

---

## 4. 产品 / 工程约定（勿回退）

| 主题 | 说明 |
|------|------|
| Capability ≠ Skill ≠ Tool | ADR-0006 §5a |
| 执行单点 | `ToolCatalog` + `dispatch_tool` only |
| Capabilities API | mode `tool_pool` ∪ `auto_fallback.tool_id` |
| JSON / URL | **workspace** 用语；**仅** `/workspaces/*`（无 `/notebooks` 双挂、无长期 notebook alias） |
| Handler | Bound 面：`docs` / `chat` / `admin_*` / `share` / `prefs` / `billing_api` |
| **AppState 停增（P0）** | **禁止**向 `AppState` / `bound/*` 新增业务方法。新能力：先 domain service（或已有 crate service），再由既有 face 调用或注入。Bound 是 thin face，不是堆逻辑的地方。 |
| Write | 不走 UnifiedAgent `ToolCatalog`；`write-core` 自有 refine dispatch |
| 不恢复 ExecutePlan | |
| Solo | 本地 trunk；定向测试；CI smoke 非默认阻塞 |

### 测试金字塔（产品拍板）

| 层 | 何时 | 内容 |
|----|------|------|
| L1 | 每次提交 | 编译、契约、crate lib、file-size |
| L2 | 动机制 / 波次 | mock 入库与四模式 smoke |
| L3 | 波次末 / 发版 | 短 UI 旅程；真 LLM 每模式 1–2 条；质量/性能分 job |

---

## 5. 非目标

| 项 | 说明 |
|----|------|
| C4 | 不做 |
| `frontend_rust` | 范围外 |
| 日常 PR 强绑真 LLM / 全 Playwright | 不做 |
| 性能进日常红线 | 不做（独立观测） |

---

## 6. 变更记录

| 日期 | 说明 |
|------|------|
| 2026-07-09 | Wave 0–6 / TN-2 / W1–W6 / R1–R3 |
| 2026-07-09 | **TN-3 拍板**：日常 A、真 AI A、真界面 A、性能 A、结构 S4、测试先量再砍 |
| 2026-07-09 | **TN-3 完成**：P2 UserProfile；P5 入口脚本 + inventory + dedup + L1 bench + 巨石部分拆分 |
