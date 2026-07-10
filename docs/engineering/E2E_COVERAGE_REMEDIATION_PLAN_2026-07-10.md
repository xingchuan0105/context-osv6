# E2E 覆盖修复计划（产品功能 × 分层）

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-10 |
| 状态 | **In progress — W1 Done (write_smoke mock green)** |
| 起因 | 审查 E2E 脚本 vs 产品能力：分层齐全，但 **Write 缺 mock/UI 门禁**；Admin/Prefs/Guardrails 偏浅；文档/registry 与金字塔入口漂移 |
| 约束 | Solo local trunk；**不扩 CI 剧场**；Write **不进** ReAct ToolCatalog（T2）；日常默认仍只 L1 |
| 上游 | 会话审查结论；[`TEST_PYRAMID_INVENTORY_2026-07-09.md`](./TEST_PYRAMID_INVENTORY_2026-07-09.md)；[`avrag-rs/docs/e2e-gates.md`](../../avrag-rs/docs/e2e-gates.md)；[`avrag-rs/docs/full-functional-e2e-guide.md`](../../avrag-rs/docs/full-functional-e2e-guide.md)；[`avrag-rs/docs/plans/2026-07-08-write-mode-launch-plan.md`](../../avrag-rs/docs/plans/2026-07-08-write-mode-launch-plan.md) |
| 执行入口 | 本计划 Wave 顺序；Agent 可用 subagent-driven / 本会话 inline |

---

## 0. 目标与非目标

### 0.1 目标

1. **四模式对称的最小门禁**：Chat / RAG / Search / **Write** 在 L2 mock 各至少 1 条可自动跑的 HTTP 黑盒。
2. **Write 用户可感路径**：Playwright 至少 1 条 journey（模式切换 → 有进度/正文 → 完成）。
3. **登记表与层语义统一**：`e2e-test-registry` + 指南层编号 + smoke 模块列表 + `scripts/test-l*.sh` 交叉一致。
4. **P1 薄补**：Guardrails 黑盒、Admin/Prefs 择一薄测（不追求全 CRUD）。

### 0.2 非目标

- 每个功能点 × 每一层的笛卡尔积覆盖  
- Write 质量语料 / persona gate 10/10 进日常  
- 恢复 PR 必跑 smoke（Solo：波次末/手动）  
- Write 进 ToolCatalog / 再引入 WriteApp 空壳  
- Desktop Tauri 全 UI E2E  

### 0.3 成功标准（DoD）

| ID | 标准 | 验证 |
|----|------|------|
| D1 | `smoke::write_smoke` 在 `E2E_MODE=smoke` 下绿（mock，无真 LLM） | `run-product-smoke-e2e.sh` 含该模块 |
| D2 | Playwright journey write 1 条绿（本地栈） | `pnpm exec playwright test …write…` |
| D3 | registry 有 `CAP-WRITE`；smoke lists 含 `workspace_crud` + `write_smoke` | `generate-e2e-test-registry.py` 再生后 diff 合理 |
| D4 | `full-functional-e2e-guide` 层表与 `test-l1…l3` 入口对齐一句说明 | 文档 diff |
| D5 | P1 至少落地 **一项**：guardrails 黑盒 **或** prefs/admin 薄测 | 对应 cargo/playwright 绿 |

---

## 1. 现状摘要（修复锚点）

| 能力 | L2 mock | L3 llm_real | L3 UI | 问题 |
|------|---------|-------------|-------|------|
| Chat/RAG/Search | ✅ | ✅ | ✅ | 可保持 |
| **Write** | ❌ | ✅ `write_real` | ❌（POM 已支持 mode） | **P0** |
| Guardrails | crate 单测 | — | — | P1 |
| Admin | — | — | 导航 only | P1 |
| Prefs | — | — | — | P1 |
| Analyze | — | — | journey+skills | 可接受 UI-only，登记补 CAP |
| 文档/registry | 漂移 | | | P2 |

**Write 技术难点（实施前必须认清）**

```text
run_write_mode:
  research (Search worker, mock search 可撑)
  → skeleton (JSON schema，需 mock 可解析 JSON)
  → draft (多节 prose)
  → WriteRefine ReAct (多轮 tool，最难 mock)
  → validate (本地指纹，可不依赖 LLM)
```

前端 **已有** `CHAT_MODE_ORDER` 含 `write` 与 `data-testid="workspace-chat-mode-write"`；缺口是 **测试与 mock 路由**，不是先做 UI（与 07-08 launch plan 部分重叠已消化）。

---

## 2. 波次编排（DAG）

```text
W0 文档/registry 对齐（可并行起）
        │
        ▼
W1 Write mock 基建 + smoke     ──P0──►  D1
        │
        ▼
W2 Write Playwright journey    ──P0──►  D2
        │
        ├──────────────────────────────►  D3/D4（W0 收尾）
        │
        ▼
W3 Guardrails 黑盒             ──P1──►  D5a
W4 Admin 或 Prefs 薄测         ──P1──►  D5b（与 W3 二选一优先即可）
W5 入口脚本注释 / L1 可选扩展  ──P2──  文档级
```

**并行规则**

- W0 可与 W1 并行（不同文件）。  
- W2 **依赖** W1（若 journey 走 mock 后端）或依赖真 LLM 栈（若走 skills/nightly 路径——本计划 **优先 mock 可重复**）。  
- W3/W4 互不阻塞，波次末二选一落地即可关 D5。  

**预估（solo 人日）**

| Wave | 人日 | 风险 |
|------|------|------|
| W0 | 0.5 | 低 |
| W1 | 1.5–2.5 | **高**（refine mock / 预算） |
| W2 | 0.5–1 | 中（超时） |
| W3 | 0.5 | 低 |
| W4 | 0.5 | 低 |
| W5 | 0.25 | 低 |
| **合计** | **~3.5–5** | |

---

## 3. Wave 0 — 文档与登记表对齐（P2，先清噪音）

### 3.1 任务

| # | 任务 | 文件 | 动作 |
|---|------|------|------|
| W0.1 | 层编号对照表 | `avrag-rs/docs/full-functional-e2e-guide.md` §1 | 增加「旧 L1–L6 ↔ 新金字塔 L1–L3」映射；指向 `scripts/test-l*.sh` |
| W0.2 | smoke 模块列表 | `e2e-test-registry.yaml` / `generate-e2e-test-registry.py` | `non_rag` 补 `workspace_crud`；为后续 `write_smoke` 预留 |
| W0.3 | Playwright 索引 | registry `playwright_specs` | 补 `api-access`、主要 journey（至少 write 占位 note） |
| W0.4 | 过时路径 | full-functional §2.6 | `notebook-crud` → `workspace-crud.spec.ts` |
| W0.5 | 检查清单 | `FUNCTIONAL_ACCEPTANCE_CHECKLIST.md` §12 | 注明 `release-e2e-gate` + `rag_quality_prod` 已接（避免误导「未强制」） |

### 3.2 层编号映射（写入指南）

| 新金字塔（权威） | 旧指南 ID | 入口 |
|------------------|-----------|------|
| L1 日常 | 部分 L6 unit | `scripts/test-l1.sh` |
| L2 mock smoke | 旧 L1 PR smoke | `test-l2-mechanisms.sh` → `run-product-smoke-e2e.sh` |
| L2 integration | 旧 L2 | `scripts/test-l2-integration.sh` |
| L3 UI | 旧 L4/L5 | `scripts/test-l3-journey.sh` |
| L3 LLM 薄路径 | 旧 L3 | `scripts/test-l3-llm.sh` |
| L3-release quality | release gate | `rag_quality_prod` |

### 3.3 验证

```bash
# 仅文档时可跳过 cargo；若改 generator：
cd avrag-rs && python3 scripts/generate-e2e-test-registry.py  # 若脚本要求参数则按其 --help
```

### 3.4 完成定义

- 新人/Agent 打开 `e2e-gates.md` + 本计划后不会把「L1」理解成 product smoke。  
- registry smoke lists 与 `run-product-smoke-e2e.sh` 差集 ≤ 待实施的 `write_smoke`。

---

## 4. Wave 1 — Write L2 mock smoke（P0）

### 4.1 策略（两阶段，避免一口吞 refine）

| 阶段 | 名称 | 断言 | 是否必须整链路 |
|------|------|------|----------------|
| **W1-A** | 路由 + 阶段可见 | SSE/HTTP：`agent_type=write`；至少 activity 含 `research`；不 5xx 死循环 | 可允许 research 后失败但须 **可诊断** |
| **W1-B** | 端到端 mock 成文 | 非空 `answer`（≥阈值）；activity 含 `research|skeleton|draft` 中 ≥2；`write_refine` 作 agent_type 仍 4xx | **目标门禁 D1** |

**优先 W1-B**；若 spike 发现 refine 不可 mock，则：

1. 引入 **仅测试可见** 的 refine 预算收紧（`WriterBudget` / `RefineLoopBudget` 在 `E2E_ENABLED=true` 时 `hard_react_cap=1` 且 mock 返回 finish）  
2. **禁止** 生产默认改行为；用 env 门闸，与现有 E2E throttle bypass 同模式  

**禁止**：为绿测而把 Write 挂进 ToolCatalog。

### 4.2 Spike（W1.0，0.5d，阻塞 W1-B）

**目的**：摸清 mock LLM 最少要识别哪些 system/user 提示，以及 refine 退出条件。

```bash
cd avrag-rs
# 手工：起 smoke context，打 write stream，抓 mock 收到的 system 前缀
E2E_MODE=smoke cargo test -p app --test product_e2e --features product-e2e \
  write_smoke -- --nocapture  # spike 期间临时测试亦可放 tests 草稿
```

**Spike 产出（写回本计划 §4.5）**

- [ ] 列出 skeleton / draft / persona / refine 的 prompt 指纹（可 `contains` 匹配）  
- [ ] 确认 mock search 是否足以让 research 产出 cards  
- [ ] refine：mock 返回何种 tool_calls / 文本可 1 轮结束  

### 4.3 实现任务

| # | 任务 | 文件（预期） | 说明 |
|---|------|--------------|------|
| W1.1 | Mock 路由枚举 | `product_e2e/mock_llm_server.rs` | 增加 Write 相关 route：至少 `WriteSkeletonJson`、`WriteDraftProse`、`WriteRefineFinish`（命名以实现为准） |
| W1.2 | 提示匹配 | 同上 `from_system_prompt` | 匹配「大纲编辑」/ skeleton JSON 指令、draft system、refine 工具轮 |
| W1.3 | Canned JSON | 同上 | skeleton 最小合法 JSON（1–2 section、`rhythm` 枚举合法、`target_chars` 小） |
| W1.4 | Draft 罐头正文 | 同上 | 足够长中文段落，便于 validate 不 panic（允许 validation_warning） |
| W1.5 | Refine 快退 | mock 或 `E2E` 预算 | 1 轮 finish / 或 mock 直接无 tool 纯文本结束——**选成本最低且不改生产语义** |
| W1.6 | smoke 用例 | `product_e2e/smoke/write_smoke.rs` + `smoke/mod.rs` | 见 §4.4 验收断言 |
| W1.7 | 注册脚本 | `avrag-rs/scripts/run-product-smoke-e2e.sh` | `NON_RAG_MODULES` 增加 `write_smoke`（Write 无 Milvus 冷启动则可并行 non-rag） |
| W1.8 | unit mock_routing | `product_e2e` mock_routing 测试 | header/from_prompt 新 route 单测 |
| W1.9 | registry | generator + yaml | `CAP-WRITE` + 新 test id |

### 4.4 `write_smoke` 验收断言（最小集）

```text
require_smoke_suite()
TestContext::new_smoke()
create_workspace("write-smoke")
chat_stream_with_params(agent_type=write, debug=true, 短 topic)

assert:
  - 流能结束（deadline 内，建议 120s smoke 上限）
  - parse done → ChatResponse.agent_type == "write"
  - answer 实质性长度（如 ≥ 40 或项目 assert_answer_substantive 阈值）
  - activity 事件 stage 集合 与 {"research","skeleton","draft","refine","validate"} 相交 ≥ 2
  - agent_type=write_refine 的对照请求 → 4xx（若已有 contract 可 skip 重复）
```

**不强制**：fingerprint band 全过、citations 非空、research 不 degrade。

### 4.5 验证命令

```bash
export CARGO_BUILD_JOBS=2
cd avrag-rs
./scripts/run-product-smoke-e2e.sh --check-modules
E2E_MODE=smoke cargo test -p app --test product_e2e --features product-e2e \
  smoke::write_smoke -- --test-threads=1 --nocapture
# 波次末：
bash ../scripts/test-l2-mechanisms.sh   # 含 product smoke
```

### 4.6 风险与回退

| 风险 | 缓解 |
|------|------|
| Refine 多轮导致 smoke > 20min | E2E cap + mock finish；超时则拆 W1-A 先合，W1-B 跟进 |
| JSON parse 双次 retry 仍失败 | canned 与 `heavytail::skeleton` 字段严格对齐；加 golden fixture 文件 |
| Search worker 慢/flaky | 沿用 mock search；research_degraded 可接受 |
| 改生产预算被误开 | 仅 `E2E_ENABLED=true` 或测试 profile；单测锁行为 |

---

## 5. Wave 2 — Write Playwright journey（P0）

### 5.1 前提

- W1-B 绿 **或** 明确改用真 LLM journey（不推荐作默认 CI 路径）。  
- UI 已具备 mode 按钮 testid（无需再做 launch plan Phase 2 主体）。

### 5.2 任务

| # | 任务 | 文件 | 说明 |
|---|------|------|------|
| W2.1 | journey spec | `frontend_next/e2e/specs/journey/workspace-write.spec.ts` | 新建；模式对齐 `workspace-chat.spec.ts` |
| W2.2 | 超时 | 同上 | `test.setTimeout(180_000)` 起；mock 后端短，真 LLM 更长 |
| W2.3 | 断言 | 同上 | 见下 |
| W2.4 | POM（仅必要时） | `chat-panel-page.ts` | 已有 `setMode("write")`；可补 `expectWriteUsageHint()` 若有 `workspace-chat-write-usage-hint` |
| W2.5 | 入口 | `scripts/test-l3-journey.sh` | `JOURNEY=1` 已跑 `e2e/specs/journey`；无需改除非要单独 project |
| W2.6 | registry | playwright_specs | 登记 CAP-WRITE / L5 |

### 5.3 断言（用户可感，非契约复读）

```text
create workspace
chat.ask(shortTopic, "write")
waitForAnswer(long timeout)
last assistant message visible + non-empty
mode indicator 含 write / 写作（若 UI 有 mode-indicator）
可选：progress card 曾出现或 activity 区可见
不要求 citation 硬门
```

### 5.4 验证

```bash
cd frontend_next
# 需 avrag-api/worker + 与 product smoke 一致的 mock 或真配置（见 playwright webServer）
pnpm exec playwright test e2e/specs/journey/workspace-write.spec.ts --reporter=list
# 或
JOURNEY=1 bash ../scripts/test-l3-journey.sh
```

### 5.5 完成定义

- D2 满足；失败时日志能区分「模式没切到 write」vs「后端超时」。

---

## 6. Wave 3 — Guardrails 黑盒（P1）

### 6.1 范围（最小）

| 用例 | 层 | 断言 |
|------|-----|------|
| 输入注入样例被拦或降级 | L2 product_e2e **或** transport-http contract | 非裸 500；有 guard / 拒绝语义 |
| 输出 PII 脱敏（若 mock 可诱导） | 同上 | 响应不含明显电话/邮箱明文 **或** 单测已覆盖则 E2E skip |

**优先放 L1/L2 契约**，避免 Playwright 脆性。

### 6.2 任务

| # | 任务 | 文件线索 |
|---|------|----------|
| W3.1 | 盘点现有 `guardrails` crate 测 | `crates/guardrails` |
| W3.2 | 补 1 条 HTTP 级 | `product_e2e/smoke` 或 `integration` 或 `transport-http/tests` |
| W3.3 | registry CAP（可选 `CAP-GUARD`） | yaml |

### 6.3 验证

```bash
cargo test -p avrag-guardrails --lib
# + 新增用例的 package filter
```

---

## 7. Wave 4 — Admin / Prefs 薄测（P1，与 W3 可择优）

### 7.1 决策规则

| 若近期改动… | 优先 |
|-------------|------|
| admin 路由 / 运维面 | Admin：`smoke/admin-navigation` 扩展 1 个真实页面断言（非仅可达） |
| 用户设置 / prefs App | Prefs：HTTP 或 1 条 Playwright settings |

**不做**：Admin 10 个导航面全量 E2E。

### 7.2 任务（模板）

| # | 任务 |
|---|------|
| W4.1 | 选 1 个高频只读面（如 Usage 或 Feature Flags 列表） |
| W4.2 | 断言 200 + 关键 testid 可见 |
| W4.3 | 登记 registry |

---

## 8. Wave 5 — 入口与 Solo 纪律（P2）

| # | 任务 | 说明 |
|---|------|------|
| W5.1 | `test-l1.sh` 注释 | 写明默认 packages；改 `transport-http` 时需 `bash scripts/test-l1.sh transport-http`（或文档表） |
| W5.2 | `e2e-gates.md` | 交叉链到本计划；标注 PR smoke 仍为 dispatch-only |
| W5.3 | `SOLO_DISCIPLINE` | 波次末验收勾选：L2 smoke（含 write）+ L3 journey 短集 |

**不**把 W1/W2 绑进每个 commit。

---

## 9. 提交切片建议（本地 trunk）

| Commit | 内容 |
|--------|------|
| 1 | docs: E2E layer map + checklist/registry drift (W0) |
| 2 | test(e2e): mock write routes + write_smoke (W1) |
| 3 | test(e2e): playwright workspace-write journey (W2) |
| 4 | test: guardrails HTTP blackbox (W3) and/or admin/prefs (W4) |
| 5 | docs: close plan status → Done |

每提交后：相关 targeted 测试绿；**不**要求全量 integration。

---

## 10. 与既有 Write 上线计划的关系

| 文档 | 关系 |
|------|------|
| [`2026-07-08-write-mode-launch-plan.md`](../../avrag-rs/docs/plans/2026-07-08-write-mode-launch-plan.md) | 产品上线（文档/UI/API）；UI 模式按钮 **代码侧多已存在** |
| **本计划** | **测试门禁与覆盖对称**；不重复做 Persona/Pro gating |
| `write_real` | 保持 nightly `#[ignore]`；W1 **不替代** 真 LLM |

若 launch plan Phase 1 文档仍缺，可与 W0 **并行** 但不阻塞 D1/D2。

---

## 11. 执行检查表（Agent / 人工）

### 开始前

- [ ] 读本计划 §0–§2  
- [ ] Milvus/PG 策略：Write smoke 默认 **不依赖** 冷 RAG；非 RAG 并行组  
- [ ] 不向用户索要已在 `.env` 的 key  

### W0

- [ ] 层映射写入 full-functional 或 e2e-gates  
- [ ] registry non_rag 含 `workspace_crud`  

### W1

- [x] Spike：skeleton=`大纲编辑`/`只返回 JSON`；draft=`中文长文写作助手`；refine=tool pool `write_refine_finish` 首轮收工；顺带修 OpenAI non-stream tool-only `EmptyStream`  
- [x] `write_smoke` 绿（mock，~17s）  
- [x] `--check-modules` 绿（15 modules）

### W2

- [ ] `workspace-write.spec.ts` 绿  

### W3/W4

- [ ] 至少一项 P1 落地  

### 收尾

- [ ] 本文件状态 → **Done**  
- [ ] 更新 `TEST_PYRAMID_INVENTORY` 资产表一行 Write  

---

## 12. 明确不做清单（防范围膨胀）

- 全量 office staging 进每日  
- MCP 真 LLM 抽检  
- Analyze Rust 双端全覆盖  
- 视觉回归扩面  
- 恢复 GitHub PR 必跑 product smoke  

---

## 13. 变更记录

| 日期 | 说明 |
|------|------|
| 2026-07-10 | 初稿：基于 E2E 覆盖审查的分波修复编排 |
| 2026-07-10 | **W1 Done**：mock Write routes + `smoke::write_smoke`；LLM tool-only finalize 修复；product_e2e `WorkspaceResponse.workspace` 对齐 |
