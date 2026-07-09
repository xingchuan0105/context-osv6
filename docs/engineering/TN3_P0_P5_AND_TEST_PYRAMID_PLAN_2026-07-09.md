# TN-3 Plan：P0–P5 结构债 + 测试金字塔重整

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-09 |
| 触发 | Thermo-Nuclear 再审查 + 用户测试目标澄清 |
| 状态 | **Locked + executing**（产品拍板 2026-07-09） |
| 约束 | Solo local trunk；默认定向验证；不默认扩 CI 门禁 |
| 相关 | [`TN_REMEDIATION_HANDOFF_2026-07-09.md`](./TN_REMEDIATION_HANDOFF_2026-07-09.md)、[`avrag-rs/docs/e2e-gates.md`](../../avrag-rs/docs/e2e-gates.md)、[`SOLO_DISCIPLINE.md`](./SOLO_DISCIPLINE.md) |

### 产品拍板（锁定）

| # | 选择 | 含义 |
|---|------|------|
| 1 日常 | **A** | 只跑快底座（L1）；真 UI / 真 LLM 不进日常 |
| 2 真 AI | **A** | 抽样通行证（每模式 1–2 条）；质量大考 release/weekly |
| 3 真界面 | **A** | 短旅程波次末；长旅程发版/夜间 |
| 4 性能 | **A** | 独立 job，不进日常红线 |
| 5 结构债 | **S4** | P0–P5 全做 |
| 6 测试改造 | **A** | 先 inventory + 测时，再收敛入口与去重 |

---

## 0. 一句话

结构债按 **P0→P4 小步收口**；测试不再「碎片 + 一锅粥」，而是 **按目的分层、按预算计时、按触发频率分发** 的金字塔。

---

## 1. 测试：需求澄清（先对齐再改套件）

### 1.1 你最初的多重目的（全部保留，但**不能塞进同一条命令**）

| 目的 ID | 业务问题 | 典型手段 | 失败时客户/研发感知 |
|---------|----------|----------|---------------------|
| **A. 界面真实可用** | 注册登录、点击、增删改查、文案/布局/可点性 | Playwright 真浏览器 | 用户「点不动 / 白屏 / 流程断」 |
| **B. LLM 主链路打通** | 真实模型能回答、能引文、不 500 | Rust product_e2e `llm_real` + 少量 skills | 用户「完全不能聊 / 空答案」 |
| **C. 四 Agent Loop 健康** | Chat / RAG / Search / Write 的 loop 策略、工具、退出、预算 | **mock** 下的 loop/unit + mock E2E；**real** 下抽样 | 研发「机制坏了」；用户「一种模式挂」 |
| **D. 性能/时延** | 端到端或机制层是否变慢 | 独立 benchmark / 带阈值的 nightly，**不是**每次 commit | 用户「卡」；研发「回归慢」 |
| **E. 机制正确性** | ingestion、检索、权限、计量 | crate lib 测 + mock integration | 研发「内部契约坏」 |

历史问题不是目的太多，而是：

1. **碎片化**：同一目的散落在 smoke / integration / llm_real / Playwright skills / journey / visual / billing 等多入口，无统一「跑哪一层」心智。  
2. **一锅粥**：一次想覆盖 A+B+C+D，导致 debug 要等完整链路（PG + worker + mock/real LLM + UI）。  
3. **缺时间预算**：没有「本层允许几分钟」的硬约束，慢测挤进日常循环。

### 1.2 金字塔（目标形状）

```
        /\          L3  旅程 / 真 UI / 真 LLM 抽样     少 · 慢 · 波次/夜间
       /  \
      / L2 \        L2  机制 + mock 产品面            中 · 分钟级 · 日/波次末
     /------\
    /  L1    \      L1  单元 / 契约 / 编译 / 端口     多 · 秒级 · 每次提交
   /__________\
```

| 层 | 名称 | 回答的问题 | 目标单次耗时（本地） | 触发 |
|----|------|------------|---------------------|------|
| **L1** | 地基 | 代码编译？类型/端口/契约对吗？纯逻辑对吗？ | **≤ 2–5 min** 定向；全量 L1 **≤ 15 min** | 每次 commit / 改相关 crate |
| **L2** | 机制 | ingestion / agent-loop / ToolCatalog / mock RAG-Search-Chat-Write 协议通吗？ | **≤ 15–25 min**（可并行子套） | 波次末；动机制时定向 L2 子集 |
| **L3** | 旅程 | 真人路径 + 真 LLM 抽样还活着吗？UI 可感问题？ | **≤ 45–90 min** 标准包；完整质量 **夜间** | 波次关闭 / 发版前 / nightly |

**刻意不放进 L1 的**：Docker 全栈冷启动、真 LLM、Playwright、质量 recall 基准。  
**刻意不放进 L3 的**：纯 merge 规则、catalog 单测、auth JWT roundtrip（应下沉 L1）。

### 1.3 目的 → 层 映射（防再碎片）

| 目的 | 主落点 | 禁止 |
|------|--------|------|
| A 界面真实可用 | **L3 Playwright**（smoke 短旅程 + journey 全旅程） | 用 Playwright 测 JSON 协议细节 |
| B LLM 主链路 | **L3 llm_real 薄切片**（每 agent 1–2 条 happy path） | 把 quality corpus 当日常 B |
| C Loop 健康 | **L1** agent-loop/agent-tools lib；**L2** mock product smoke 四模式 | 只用真 LLM 才测 loop 分支 |
| D 性能 | **L3-nightly / release** 独立 job + 基线对比 | 塞进 L1/L2 必过且无预算 |
| E 机制正确 | **L1** crate 测；**L2** mock integration 子集 | 每个小机制单独起一套 E2E 二进制 |

### 1.4 时间预算（验收标准）

实施前先做 **inventory + 实测**（P5-0），再调数字。初值：

| 套件 | 预算 | 超预算处理 |
|------|------|------------|
| `cargo test -p <touched> --lib` | 单包 ≤ 60s 理想 | 超则拆测或 mock 变重 |
| `cargo test -p agent-loop -p agent-tools -p app-chat --lib` | ≤ 5 min | 下沉 fixture |
| L2 mock product smoke（现 `run-product-smoke-e2e.sh` 重整后） | ≤ 20 min | 砍并行争用 / 共享 fixture / 删重复 |
| L2 integration 全量 mock | ≤ 40 min 或拆「core / edge」 | edge 降为 weekly |
| L3 Playwright smoke（auth+短 CRUD） | ≤ 15 min | 旅程全量不进 smoke |
| L3 Playwright journey 全量 | ≤ 45 min | 夜间 |
| L3 llm_real 四模式抽样 | ≤ 30–40 min（成本+时延） | 与 quality 分 job |
| L3 rag_quality_prod / judge | 无日常预算 | release / weekly only |

### 1.5 入口收敛（消灭「一锅粥命令」）

目标：**≤ 5 个正式入口**，名字与层绑定：

| 命令（建议） | 层 | 内容 |
|--------------|-----|------|
| `scripts/test-l1.sh [crate…]` | L1 | check + 定向 `--lib` + file-size gate + 可选 `tsc`/vitest 定向 |
| `scripts/test-l2-mechanisms.sh` | L2 | agent-loop/tools + storage 关键 lib + mock product **core** smoke |
| `scripts/test-l2-integration.sh` | L2 | 现 integration mock（可再拆 core） |
| `scripts/test-l3-journey.sh` | L3 | Playwright smoke 或 journey 子集（env 选） |
| `scripts/test-l3-llm.sh` | L3 | `llm_real` 四模式抽样 + 可选 quality |

现有 `run-product-smoke-e2e.sh` / `E2E_MODE=*` / 十余个 workflow **映射进上表**，不并行发明第 16 种。

### 1.6 现有资产如何归层（不推倒重来）

| 现有 | 建议层 | 动作 |
|------|--------|------|
| `cargo test -p X --lib` | L1 | 保持；扩大覆盖比再加 E2E |
| `transport-http` / contracts 单测 | L1 | 保持 |
| `frontend_next` vitest | L1 | 保持 |
| `product_e2e::smoke::*` mock | L2 | 重整模块列表；删与 L1 重复断言 |
| `product_e2e::integration::*` | L2 | 标 core vs edge；edge 降频 |
| `product_e2e::llm_real::*` 薄路径 | L3 | 每 agent 保留最短路径 |
| `llm_real/rag_quality_prod.rs` 巨石 | L3-release | **不**进日常；可拆文件但不进 L1/L2 |
| Playwright `specs/smoke` | L3-smoke | 日常旅程薄切 |
| Playwright `specs/journey` | L3-journey | 波次末 |
| Playwright `skills` + judge | L3-quality | nightly |
| Playwright billing / visual | L3-specialty | 路径过滤或 weekly |

### 1.7 P5 实施步骤（测试专章）

| 步 | 内容 | 验收 |
|----|------|------|
| **P5-0 Inventory** | 列出所有 test binary / Playwright project / workflow；每项标注目的 A–E、估时、是否 Docker/真 LLM | 一张表进 `e2e-test-registry` 或新 `test-pyramid-inventory.md` |
| **P5-1 测时** | 本地跑各入口，记录 wall-clock（脚本 `scripts/bench-test-suites.sh` 可选） | 有数字才谈砍 |
| **P5-2 分层标签** | 统一：Rust `#[cfg]` / module path / `E2E_MODE`；Playwright project 与目录一致 | 文档一张映射表 |
| **P5-3 入口脚本** | 实现 L1–L3 五个脚本；旧脚本变 thin wrapper | `test-l1` 默认 < 预算 |
| **P5-4 去重** | 同一断言只留最低足够层（例：SSE 顺序只在 transport-http L1） | 删或 `#[ignore]` 重复 E2E |
| **P5-5 巨石拆文件** | `rag_quality_prod` / `llm_real/mod` / `test_context/builder` **按层拆文件**，不改变层级语义 | 文件 < 800 行优先；**不等于**进 L1 |
| **P5-6 文档** | 重写 `e2e-gates.md` 金字塔段；SOLO：日常只 L1+定向 L2 | handoff 链接 |

**不做**：为金字塔强行把真 LLM 放进 PR gate；为「全绿」合并 A+B+C+D 成一条 job。

---

## 2. 结构债 P0–P4（与测试解耦，可并行排期）

来源：Thermo-Nuclear 再审查。**默认顺序 P0→P4**；P5 可与 P2/P3 交错。

### P0 — AppState 停增 + 边界纪律（纪律优先）

| 项 | 内容 |
|----|------|
| **问题** | ~1752 LOC / ~134 方法门面；Bound 是薄委托，概念未减 |
| **做** | 1) 文档钉死：新能力 **禁止** 直接加 `AppState`/`bound` 方法，必须先有 domain service；2) 新代码只通过已有 face 或新 service 注入；3) 可选：Bound 只 `pub fn docs() -> &DocumentApp` 逐步收口 |
| **不做** | 一轮把 86 个方法全搬完（过大） |
| **验收** | ADR/handoff 纪律条款；下一次 feature 不新增 AppState 业务方法；gate soft warn 不恶化 |

### P1 — 前端删掉双 Workspace DTO

| 项 | 内容 |
|----|------|
| **问题** | `RawWorkspace` + `mapWorkspace` 与 typeshare `Workspace` 并行 |
| **做** | client 直接用 generated contracts；UI 统一 `id` 或单点 adapter；删重复 envelope 类型 |
| **验收** | `pnpm tsc` + `vitest` workspace client；无第二套 Workspace 形状 |

### P2 — Profile 存储强类型

| 项 | 内容 |
|----|------|
| **问题** | Delta 强类型，merge 仍 `serde_json::Value` 手术 |
| **做** | `UserProfile` 结构体；`apply_delta: (UserProfile, ProfileDelta) -> UserProfile`；仅 adapter 编解码 jsonb |
| **验收** | chat_private 测全绿；无生产路径 `apply_*_from_value` |

### P3 — workspace 命名漏网收口

| 项 | 内容 |
|----|------|
| **问题** | `share_enabled_for_notebook`、`resolve_share_chat_notebook_scope`、memory `state.notebooks`、FE middleware `/notebooks` |
| **做** | 方法/字段 rename；**删除** 产品已否决的 `/notebooks` middleware（确认无外链后） |
| **验收** | 活跃代码无 `*_notebook_*` 业务 API；middleware 仅 `/workspaces` |

### P4 — 文档口径对齐

| 项 | 内容 |
|----|------|
| **问题** | handoff 仍写双挂 / notebook 读兼容 |
| **做** | 改 TN handoff + e2e-gates 交叉引用；与 WORKSPACE 决策一致 |
| **验收** | 文档无「双挂默认」表述 |

---

## 3. 总排期建议（solo）

| 阶段 | 内容 | 预估 | 验证 |
|------|------|------|------|
| **W-A** | P4 文档 + P0 纪律写入 handoff/ADR 短注 | 0.5d | 读文档 |
| **W-B** | P1 FE DTO | 0.5–1d | tsc + vitest |
| **W-C** | P3 命名漏网 | 0.5–1d | cargo test 定向 |
| **W-D** | P2 Profile 强类型 | 1–2d | chat_private + 相关 |
| **W-E** | P5-0…P5-3 金字塔 inventory + 入口脚本 | 1–2d | 脚本 + 测时表 |
| **W-F** | P5-4…P5-6 去重 + 巨石拆文件 + 文档 | 1–2d | 入口预算达标 |

AppState **大搬迁**（门面变 composition root）单列 **W-G 可选**，不阻塞产品；仅当再出现「Bound 继续膨胀」时启动。

---

## 4. 明确非目标

- C4 合并 Capability/Skill/Tool  
- Write 并入 `ToolCatalog`  
- 为金字塔恢复 PR 强制真 LLM / 全 Playwright  
- 无测时数据前盲目删测  
- archive / 历史 eval jsonl 全量改词  

---

## 5. 决策确认清单（实施前你可勾选）

测试相关（本 plan 已按你的目标起草，实施前建议确认）：

- [ ] L1 日常只跑「秒～数分钟」，接受 **不** 覆盖真 UI / 真 LLM  
- [ ] 真 UI（A）固定在 L3 Playwright，不与 mock smoke 混跑同一命令  
- [ ] 真 LLM（B）固定在 L3 抽样，**质量语料**（recall）单独 release/weekly  
- [ ] 四 Agent loop（C）主靠 L1 unit + L2 mock；L3 每模式 1–2 条真路径即可  
- [ ] 性能（D）单独 job + 基线，不进 L1 必过  
- [ ] 旧 workflow 可保留为 wrapper，但**文档只推 5 个入口**

结构债：

- [ ] P0 先纪律后大拆  
- [ ] P1–P4 按序；P5 可与之交错  

---

## 6. 成功图像

**开发中午**：`test-l1` 绿 → commit。  
**改 loop/ingestion 下午**：`test-l2-mechanisms` 绿。  
**波次末**：`test-l3-journey` + `test-l3-llm` 绿。  
**发版前**：quality / release gate。  

结构上：AppState 不再默认可加方法；FE/Profile/命名无「半迁移」；文档与决策一致。

---

## 7. 变更记录

| 日期 | 说明 |
|------|------|
| 2026-07-09 | 初稿：P0–P5 + 测试金字塔需求澄清 |
