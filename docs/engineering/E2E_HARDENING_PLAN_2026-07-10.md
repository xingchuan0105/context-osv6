# E2E Hardening 修复计划（审查后）

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-10 |
| 状态 | **Done** — H1/H2/H4/H5 落地；H3 选项 **C**（journey 真 LLM + 文档说明，未接 mock 栈） |
| 上游审查 | 会话审查（交接文档 + product_e2e / journey 代码） |
| 上游交接 | [`E2E_COVERAGE_REMEDIATION_HANDOFF_2026-07-10.md`](./E2E_COVERAGE_REMEDIATION_HANDOFF_2026-07-10.md) |
| 上游覆盖修复（Done） | [`E2E_COVERAGE_REMEDIATION_PLAN_2026-07-10.md`](./E2E_COVERAGE_REMEDIATION_PLAN_2026-07-10.md) |
| 门禁 | [`avrag-rs/docs/e2e-gates.md`](../../avrag-rs/docs/e2e-gates.md) |
| Solo | [`SOLO_DISCIPLINE.md`](./SOLO_DISCIPLINE.md)、根 `AGENTS.md` §7–§8 |

---

## 1. 一句话

**覆盖缺口已关（Write L2+L3、Guardrails 黑盒、registry）。本计划只 hardening：收紧软断言、锁协议行为、降 journey 成本、清小债。**  
不扩能力矩阵、不恢复 PR smoke 必跑、不把 Write 挂进 ToolCatalog。

---

## 2. 审查结论摘要（修复锚点）

| ID | 问题 | 影响 | 本计划 |
|----|------|------|--------|
| R1 | `write_smoke` 阶段门槛 `hit >= 2` 过松 | 假绿：未走齐 skeleton/draft 仍可能过 | **H1** |
| R2 | journey 正文仅 `length > 0`；`getLastMessage` 不限 assistant | UI 假绿 / 偶发选错气泡 | **H1** |
| R3 | OpenAI tool-only 仅 smoke 间接覆盖 | 再动协议易回归 EmptyStream | **H2** |
| R4 | journey 默认真 LLM ~6min | 成本高、flaky、难日常复跑 | **H3** |
| R5 | `write_real` activity 采 `stage` 而非 SSE `phase` | nightly artifact 空，不挡 pass | **H4** |
| R6 | `smoke/mod.rs` 头注释仍写「3 P0」 | 文档漂移 | **H4** |
| R7 | mock 路由靠中文 system 子串 | prompt 改文案则 smoke 红 | **H3 备注 / 不单独开波** |
| R8 | `agent_type=write_refine` 可能落 chat 而非 4xx | 产品缝隙 + 无 smoke | **H5（可选）** |
| R9 | Admin/Prefs 无薄测；registry 全量 regenerate 不全 | 覆盖/登记残留 | **Out / 低优** |
| R10 | 双套层编号（指南 L1–L6 vs `test-l*.sh`） | 新人混淆；§1 已有映射 | **Out（已够用）** |

**不做（明确非目标）**

- 恢复 GitHub PR 必跑 smoke / 扩 CI 剧场  
- Write 质量语料 / persona / fingerprint band 全过进 smoke  
- Guardrails 全入口（rag/search/write）笛卡尔积  
- Admin CRUD / Prefs 全量（原 W4；改产品时再开）  
- Desktop Tauri E2E  

---

## 3. 成功标准（DoD）

| ID | 标准 | 验证 |
|----|------|------|
| D1 | `write_smoke` 阶段断言收紧后仍绿 | `E2E_MODE=smoke … smoke::write_smoke` |
| D2 | `workspace-write` 断言收紧（assistant + 长度阈值）仍可绿 | Playwright list（mock 优先，真 LLM 备选） |
| D3 | `avrag-llm` 有 tool-only finalize/on_halt 单测 | `cargo test -p avrag-llm --lib openai` |
| D4 | （H3 若做）journey 可在 mock 栈下 < 2min 绿 **或** 文档标明真 LLM-only 与入口 | 实测墙钟 + 注释 |
| D5 | `write_real` phase 采集 + smoke 头注释修正 | 代码 diff + 可选 compile |
| D6 | 铁律不回退：T2 Write∉ToolCatalog；Solo 日常仍 L1 only | review 检查清单 |

**最小可关波**：H1+H2（D1–D3）。H3–H5 按优先级可选。

---

## 4. 波次编排（DAG）

```text
H1  断言 hardening（write_smoke + journey）     ──P0──►  D1, D2
        │
        ▼
H2  OpenAI tool-only 协议 unit                    ──P0──►  D3
        │
        ├──────────────────────────────────────────►  最小关波
        │
        ▼
H3  Journey mock 栈 / 降成本（可选，中）         ──P1──►  D4
H4  观测与文档债（write_real phase、mod 注释）   ──P2──►  D5
H5  write_refine agent_type 产品缝 + smoke（可选）──P2──►  产品债
```

**并行规则**

| 规则 | 说明 |
|------|------|
| H1 ∥ H2 | 不同 crate/文件，默认可并行 |
| H3 依赖 H1 | 先收紧断言再换 mock 栈，避免同时改两维难归因 |
| H4 ∥ 任意 | 纯清理，不阻塞 |
| H5 独立 | 含产品行为决策；**勿**与 H1 混在同一 commit 除非用户明确要 |

**预估（solo 人日）**

| Wave | 人日 | 风险 |
|------|------|------|
| H1 | 0.25–0.5 | 低（断言过严导致假红 → 回退阈值） |
| H2 | 0.25–0.5 | 低 |
| H3 | 1–2 | **中高**（Playwright 后端 wiring） |
| H4 | 0.1 | 低 |
| H5 | 0.5–1 | 中（产品语义：reject vs alias） |
| **最小（H1+H2）** | **~0.5–1** | |
| **含 H3** | **~1.5–3** | |

---

## 5. Wave H1 — 断言 hardening（P0）

### 5.1 目标

在**不改变 mock 路由/生产逻辑**的前提下，提高 Write 门禁硬度，降低假绿。

### 5.2 任务

| # | 任务 | 文件 | 动作 |
|---|------|------|------|
| H1.1 | 收紧 stage 断言 | `avrag-rs/crates/app/tests/product_e2e/smoke/write_smoke.rs` | 见 §5.3 |
| H1.2 | journey 长度 + 角色 | `frontend_next/e2e/specs/journey/workspace-write.spec.ts` | 见 §5.4 |
| H1.3 | （可选）POM 助手 | `frontend_next/e2e/pom/chat-panel-page.ts` | `getLastAssistantMessage()`；仅当 ≥2 specs 复用时再抽，避免 YAGNI |

### 5.3 `write_smoke` 断言规格（目标）

**替换** 当前：

```text
expected_stages ∩ phases 的 hit >= 2
```

**改为**（推荐，一次到位）：

```text
phases 归一化后（exact 或 starts_with("{stage}_")）：
  MUST contain "skeleton"
  MUST contain at least one of: "draft", "refine", "validate"
  hit = |{research,skeleton,draft,refine,validate} ∩ phases| 且 hit >= 3
保留：
  agent_type == "write"
  assert_answer_substantive(&resp, 40)
  deadline 180s / max_events 2048
仍不强制：
  citation 非空、fingerprint band 全过、research 不 degrade
```

**假红回退阶梯**（仅当 H1.1 实测红且 root cause 是 activity 命名而非回归）：

1. 放宽 `draft`：section progress 可能发 `draft` 子 phase；已有 `starts_with` 则保持。  
2. 若 mock 下 `validate` 偶发缺失：MUST 集合改为 `skeleton` + (`draft`|`refine`)。  
3. **禁止**回到裸 `hit >= 2` 而不记录原因。

### 5.4 `workspace-write` 断言规格（目标）

```text
// 选气泡：assistant only
const lastMessage = page
  .locator('[data-testid="chat-message"][data-role="assistant"]')
  .last();
// 或 chat.getLastAssistantMessage() 若 H1.3 做了

await expect(lastMessage).toBeVisible();
await expect(lastMessage.locator("[data-testid='mode-indicator']"))
  .toContainText(/write|写作/i);

const answer = await chat.lastAnswerText();
expect(answer.length).toBeGreaterThan(40);  // 对齐 smoke 量级；勿要求 80+（真 LLM 短题也可）

保留：
  write-usage-hint visible（mode 已切 write）
  waitForAnswer(540_000) + test timeout 600_000  // H3 前不改超时
```

**注意**：`getLastMessage()` 全 role 问题在 skills/chat 其他 spec 也存在——**本波只改 write journey**，不扫射重构全部 journey。

### 5.5 验证

```bash
export CARGO_BUILD_JOBS=2
cd avrag-rs
E2E_MODE=smoke cargo test -p app --test product_e2e --features product-e2e \
  smoke::write_smoke -- --test-threads=1 --nocapture

# journey：H3 前仍可能真 LLM
cd frontend_next
pnpm exec playwright test e2e/specs/journey/workspace-write.spec.ts --reporter=list
```

### 5.6 完成定义

- D1、D2；失败信息能区分「缺 skeleton」vs「answer 过短」vs「mode 未切 write」。

---

## 6. Wave H2 — OpenAI tool-only 协议 unit（P0）

### 6.1 目标

把 W1 生产修复从「仅 product smoke 间接覆盖」提升为 **crate 级锁行为**。

### 6.2 任务

| # | 任务 | 文件 | 动作 |
|---|------|------|------|
| H2.1 | finalize 单测 | `avrag-rs/crates/llm/src/protocols/openai_chat/`（既有 `mod tests` 或 `protocol` 旁测） | 空 content + 非空 `tool_calls` → `Ok`，`content` 可空，`tool_calls` 保留 |
| H2.2 | on_halt 单测 | 同上 | 不推 `ProviderError` empty stream；`FinishReason::ToolCalls` |
| H2.3 | 对照负例 | 同上 | 空 content、无 tool_calls、无 reasoning → 仍 `EmptyStream` / ProviderError |

### 6.3 实现提示（不写实现细节死锁）

- 复用现有 `OpenAiChatProtocol` 状态构造 / 测试 fixture（`mod.rs` tests 已有 tool_calls 序列化样例）。  
- **禁止**为测而改生产语义；只钉当前 W1 行为。  
- 包名以 `Cargo.toml` 为准（`avrag-llm` / `llm`）。

### 6.4 验证

```bash
cd avrag-rs
cargo test -p avrag-llm --lib openai -- --nocapture
# 若 package 名不同：
# cargo test -p llm --lib openai
```

### 6.5 完成定义

- D3；以后动 `protocol.rs` finalize/on_halt 时单测先红。

---

## 7. Wave H3 — Journey mock 栈（P1，可选）

### 7.1 目标

Write UI journey **默认可在 mock 后端**复跑，墙钟目标 **< 2 min**（理想 < 60s），与 L2 `write_smoke` 同构。

### 7.2 策略选择（实施前拍板，默认 A）

| 选项 | 做法 | 优点 | 缺点 |
|------|------|------|------|
| **A（推荐）** | 复用/接通 product_e2e mock 后端 launcher 或现有 Playwright `webServer` mock 配置，使 journey 打到 mock LLM | 与 smoke 一致、可重复 | 需理清 frontend_next 全局 setup |
| **B** | journey 保持真 LLM，单独 `write-mock.spec` 或 env `E2E_WRITE_MOCK=1` 分支 | 少动默认路径 | 两套 spec 维护 |
| **C** | 不做 mock，只把超时/文档写清「真 LLM only」 | 零工程 | D4 以文档关闭；成本问题不解决 |

**默认选 A**；若 spike ≤0.5d 发现 wiring 成本 >2d，降级 **C** 并在本计划标注 Cancelled reason。

### 7.3 Spike（H3.0，阻塞实现）

```text
问题：
  1. Playwright 当前 baseURL / API 是否已能指向 mock product_e2e？
  2. backend_launcher（smoke/mod.rs ignore）是否仍为官方入口？
  3. write 在 mock 下 UI 是否仍长时间停在 progress（应否）？
产出：
  - 推荐选项 A/B/C + 预估文件列表
  - 墙钟基线（一次本地 run）
```

### 7.4 实现任务（选项 A 骨架）

| # | 任务 | 说明 |
|---|------|------|
| H3.1 | 文档/注释 | `workspace-write.spec.ts` 顶注释写清 mock vs real 前提 |
| H3.2 | 栈接通 | 按 spike：env / globalSetup / reuse server |
| H3.3 | 超时下调 | mock 下 `setTimeout(120_000)`、`waitForAnswer(90_000)`；真 LLM 用 project 或 env 保留 600s |
| H3.4 | 验证 | 连续 2 次 mock 绿；可选 1 次真 LLM 回归 |

### 7.5 完成定义

- D4；或书面 Cancelled + 选项 C 文档。

### 7.6 相关：mock 指纹脆弱性（R7）

H3 **不**强制改 prompt 指纹策略。若 H3 期间改 Writer system 文案，必须同步：

```bash
cargo test -p app --test product_e2e --features product-e2e mock_routing -- --nocapture
E2E_MODE=smoke cargo test -p app --test product_e2e --features product-e2e smoke::write_smoke -- --test-threads=1
```

长期（Out of scope）：mock 优先认 `write:skeleton` / phase header，而非中文 `contains`。

---

## 8. Wave H4 — 观测与文档债（P2）

### 8.1 任务

| # | 任务 | 文件 | 动作 |
|---|------|------|------|
| H4.1 | activity 字段 | `avrag-rs/crates/app/tests/product_e2e/llm_real/write_real.rs` | artifact 采集：`phase` 优先，兼容 `stage`（与 `write_smoke` 同构） |
| H4.2 | 模块说明 | `avrag-rs/crates/app/tests/product_e2e/smoke/mod.rs` | 更新头注释：模块列表指向 `run-product-smoke-e2e.sh`；删除过时「3 P0 cases」 |
| H4.3 | （可选）registry regenerate | `generate-e2e-test-registry.py` | 仅当 H1–H2 增测 id 后需要；diff 审再提交 |

### 8.2 验证

```bash
# H4.1 无强制跑 nightly；编译即可
cd avrag-rs && cargo test -p app --test product_e2e --features product-e2e write_real -- --list
```

### 8.3 完成定义

- D5；注释与 runner 列表一致。

---

## 9. Wave H5 — `write_refine` agent_type 缝隙（P2，可选）

### 9.1 背景

交接/审查：文档曾称 `write_refine` 不可选为用户 agent_type；当前可能 **落到 chat** 而非专用 4xx。属**产品语义**，不是纯测试债。

### 9.2 决策门（实施前必须二选一）

| 选项 | 产品行为 | 测试 |
|------|----------|------|
| **Reject（推荐）** | `agent_type=write_refine` → 400，稳定 error code（如 `invalid_agent_type` / `write_refine_not_user_mode`） | smoke 1 条 HTTP 黑盒 |
| **Alias** | 显式文档：内部别名 → 按 `write` 跑 | 测试锁 alias 行为；**不推荐**（混淆 T2 边界） |

### 9.3 任务（Reject 路径）

| # | 任务 | 层 |
|---|------|-----|
| H5.1 | 在 conversation / 请求校验处拒绝 `write_refine` | 生产（`app-chat` / transport 校验，**非** AppState 新业务方法） |
| H5.2 | `smoke` 或 `write_mode_contract` 一条 | 400 + error code |
| H5.3 | 文档一句 | full-functional 或 write launch plan |

### 9.4 铁律

- 不把 `write_refine_*` 工具挂回 ToolCatalog。  
- 用户可见模式仍只有 `write`；refine 仅内部 control ring。

---

## 10. 提交与验证策略（Solo）

### 10.1 建议 commit 切片

```text
test(e2e): tighten write_smoke and workspace-write assertions (H1)
test(llm): lock openai tool-only empty content (H2)
test(e2e): mock-backed workspace-write journey (H3, optional)
test(e2e): write_real phase field + smoke module docs (H4)
fix: reject agent_type=write_refine at API boundary (H5, optional)
```

- 本地 `master` trunk；**不**默认 push / 开 PR。  
- 每波只跑**相关**验证；波次末再考虑 `run-product-smoke-e2e.sh` 全量。

### 10.2 验证矩阵

| 变更 | 必跑 | 可选 |
|------|------|------|
| H1 write_smoke | `smoke::write_smoke` | `--check-modules` |
| H1 journey | `workspace-write.spec.ts` | — |
| H2 | `cargo test -p avrag-llm --lib openai` | product `write_smoke` 一次 |
| H3 | mock journey ×2 | 真 LLM ×1 |
| H4 | `--list` / 文档审 | — |
| H5 | 新 smoke + 相关 lib | — |
| 日常开发 | **仍** `scripts/test-l1.sh` only | — |

### 10.3 波次末（可选）

```bash
cd avrag-rs && ./scripts/run-product-smoke-e2e.sh --check-modules
# 若碰 non-RAG 多模块：
# bash ../scripts/test-l2-mechanisms.sh
```

---

## 11. 风险与回退

| 风险 | 缓解 |
|------|------|
| H1 断言过严导致 mock 假红 | 回退阶梯 §5.3；先看 activity dump 再降阈值 |
| H2 测不到私有 finalize 状态 | 经公开 Protocol trait / 现有 test harness 构造；必要时 `#[cfg(test)]` 友元，禁止复制生产逻辑到测 |
| H3 wiring 黑洞 | spike 超时 → 降级选项 C |
| H5 误伤内部调用 | 仅拒 **HTTP/MCP 用户入口** 的 agent_type 字符串；内部 refine 不走该字段 |
| 并行 cargo OOM | `CARGO_BUILD_JOBS=2`；不叠两次 full test |

---

## 12. 铁律（勿回退）

| # | 规则 |
|---|------|
| T1 | 不在 `AppState` 加业务方法 |
| T2 | Write / `write_refine_*` **∉** ReAct ToolCatalog |
| T3 | Chat/RAG/Search 执行经 `dispatch_tool` |
| Solo | 日常 **L1 only**；smoke/journey 波次末或手跑 |
| CI | **勿**擅自恢复 PR smoke 必跑 |
| 范围 | 每 commit 可追溯到本计划 Wave ID |

---

## 13. 执行检查清单（接盘）

- [ ] 读本计划 §2–§4 + 上游 handoff §5–§7  
- [ ] **H1** 改断言 → 跑 write_smoke + workspace-write  
- [ ] **H2** 协议 unit → `avrag-llm` openai 测绿  
- [ ] 最小关波：勾选 D1–D3  
- [ ] （可选）H3 spike → A/B/C 决策 → 实现或 Cancel  
- [ ] （可选）H4 清理  
- [ ] （可选）H5 产品决策后再码  
- [ ] 本地 commit；用户要求时再 push  

---

## 14. 相关链接

| 文档 | 用途 |
|------|------|
| [覆盖修复计划 Done](./E2E_COVERAGE_REMEDIATION_PLAN_2026-07-10.md) | 本波前置：Write/Guard/registry 已落地 |
| [覆盖修复交接](./E2E_COVERAGE_REMEDIATION_HANDOFF_2026-07-10.md) | 代码触点、验证命令、残留表 |
| [e2e-gates](../../avrag-rs/docs/e2e-gates.md) | 金字塔与 smoke 模块列表 |
| [full-functional-e2e-guide](../../avrag-rs/docs/full-functional-e2e-guide.md) | 能力矩阵 + 新旧层映射 |
| [SOLO_DISCIPLINE](./SOLO_DISCIPLINE.md) | 本地 trunk / 验证分层 |

---

## 15. 变更记录

| 日期 | 说明 |
|------|------|
| 2026-07-10 | 初稿：基于覆盖修复完成后的代码审查，编排 H1–H5 hardening 计划 |
| 2026-07-10 | 实施完成：H1 断言收紧；H2 tool-only unit；H3→C 文档；H4 phase/注释；H5 reject `write_refine` + smoke/unit。验证：`avrag-llm` openai 23 ok；`write_smoke` 2 ok；`write_mode_contract` 4 ok；`conversation_rejects_write_refine` ok |
