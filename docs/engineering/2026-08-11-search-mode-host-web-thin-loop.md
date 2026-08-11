# Search 模式瘦身：Host 直调 web + 薄 synthesis（RAG/dual 不变）

> **SUPERSEDED（产品路径，2026-08-11 W3）** — search-only 已切 **LeadWorkers**：host 叶子仍产证据（+CRW），**Lead 合成**写用户气泡。`RetrieveStrategy::HostWeb` 代码可保留作测试/hospice，**assemble 不再选用**。权威：`docs/plans/2026-08-11-lead-rag-web-workers-design.md`。

- **日期**: 2026-08-11  
- **状态**: **产品路径已取代**（W3）；本文保留 host 叶子设计意图  
- **范围**: 历史 search-only host_web  
- **非范围**: 现行 product assemble  

## 1. 问题

当前 search 模式与 rag 共用 **SaC 三环**（retrieve 多轮 codegen → synthesis → verify）。  
底层 `client.web` 已是 DeepSeek Responses **服务端 agentic** 检索（裸 query + SSE）。再叠一层产品 SaC 多轮 fan-out，导致：

- 延迟叠加（单次 web 10–20s × 多 query × 多轮 LLM）  
- 策略冲突（两边都在「决定搜什么」）  
- 沙箱墙钟 / 截断类故障面放大  

**目标**：search-only 把 DeepSeek 当**证据源 API**（一次 host 检索），去掉 retrieve 阶段的 codegen 叠床架屋；**不把** DeepSeek 全文当唯一用户气泡（保留薄 synthesis，可选 verify）。

## 2. 原则（与已拍板边界对齐）

| # | 原则 |
|---|------|
| P1 | **search-only** 可砍 / 绕过 SaC retrieve；DeepSeek 不叠产品 agentic |
| P2 | **web 叶子形状不变**：仍产出 `SearchResponse`（title/url/snippet/…）进证据面 |
| P3 | **rag-only / dual 的 loop 状态机不动**；dual 继续 `client.web` 同一叶子 |
| P4 | dual **合并的是 web hits**，不是 DeepSeek `output_text` 终答管道 |
| P5 | 用户主气泡仍由 **synthesis（模型）** 写；host 不拼脚注（既有 harness 哲学） |
| P6 | 无兼容税：search 旧 SaC 路径直接替换，不留双轨开关长期并存 |

## 3. 目标架构

### 3.1 能力判定（单一条件）

```text
search_host_web_path  ≔  mode_id == "search"
                       ∧  sdk 含 web/fetch
                       ∧  sdk 不含 dense/lexical/grep   // 非 dual
```

- dual = `sdk_primitives_for_caps(true, true)` → **不走**本路径。  
- 判定放在 loop 入口或 retrieve 首轮，读 `ModeConfig.sdk_primitives`（assemble 已填）。

### 3.2 数据流

```text
search-only（新）
  user query
    → Host: SearchProvider.search(query)     // 现 execute_deepseek_web + CRW enrich
    → 写入 tool_results / evidence（与 bridge Ok web 同构）
    → 可选 progress: Searching / SourcesCollected
    → BreakToSynthesis（跳过 codegen 多轮）
    → synthesis（writing skill，基于 observation 中的 sources）
    → verify（默认关或 1 次；见 §5）
    → 用户气泡

dual / rag（不变）
  SaC retrieve（dense ± web）→ synthesis → verify
  client.web 仍走同一 SearchProvider 叶子（裸 query + stream）
```

### 3.3 与「DeepSeek 全文当答案」的区别

| | 本方案 | 不采纳 |
|--|--------|--------|
| 交付 | sources → **我们的** synthesis 成文 | DeepSeek message 原文直出 |
| 引用 | `[[web:n]]` 对齐 SearchResponse | 对方脚注格式 |
| dual | 可共用 hits | 两套答案管道 |

## 4. 配置面（search.yaml）

建议显式字段（避免魔数埋代码）：

```yaml
# modes/search.yaml（示意）
retrieve_strategy: host_web   # 新枚举；缺省/其它模式 = sac_codegen
loop_exit:
  forbid_retrieve_direct_answer: true   # 仍禁止 retrieve 直出终答
  verify: false                         # 首版建议关，降延迟；见 §5
  # 或 verify: true + verify_max_fail_rounds: 1
budget:
  max_iterations: 2                     # synthesis(+verify) 即可；retrieve 无 LLM 轮
```

- **不**给 dual/rag 加 `host_web`。  
- `auto_fallback.web_search` 可保留作 executor 空结果时的 Brave 路径（已在 provider 层 deepseek→brave）。

## 5. Verify 策略（首版建议）

| 选项 | 延迟 | 质量闸 | 建议 |
|------|------|--------|------|
| **verify off** | 最低 | 无 | **首版默认**（search 要快） |
| verify on, max_fail=1 | +1 LLM | 轻 | 若上线后幻觉多再开 |
| 保持 max_fail=3 | 差 | 重 | 与瘦身目标冲突，不做 |

若关 verify：synthesis 一次交付；灾难兜底仍走现有 token/格式闸。

## 6. 实现切片（PR / 本地 commit 顺序）

### Slice A — 配置与判定（小）

1. `ModeConfig` / YAML：`retrieve_strategy: sac_codegen | host_web`（serde 默认 `sac_codegen`）。  
2. `search.yaml` → `host_web`；`rag.yaml` 不写（默认 sac）。  
3. 纯函数 `fn is_search_host_web_path(mode: &ModeConfig) -> bool`（mode id + strategy + primitives 无 dense）。  
4. 单测：search yaml 为 true；rag / dual 组装为 false。

### Slice B — Host retrieve 一步（核心）

1. 在 `run_retrieval` **进入 LLM 循环前**（或 iteration 0 专用分支）：  
   - 若 `is_search_host_web_path`：  
     - `deps.execute_search_fallback(query, Some("web"))` 或直接 `SearchProvider`（与 fallback 同构，优先复用已有 executor 入口，含 CRW enrich）。  
     - 将 `SearchResponse` 落成与 bridge `web` 相同的 `ToolResult`（`tool: web_search`, status Ok, data=…）。  
     - 注入 model 可见 observation（第三人称，**prompts/loop/** 新 asset，如 `host-web-results.tmpl.md`；占位符填 JSON/紧凑列表；**禁止**在 Rust 里写长中文指令体）。  
     - emit progress（Searching / sources）。  
     - **不**跑 codegen；直接 `BreakToSynthesis`（或等价状态转移）。  
2. 空结果：observation 陈述空命中 → synthesis 可澄清/弱答；不强制再开 SaC 多轮（可后续加「一次重试换 query」）。  
3. 错误：同现网 fallback 错误 observation；不 crash 整管道。

### Slice C — Synthesis 侧（薄适配）

1. 确认 search synthesis 已能读 `tool_results` / web 证据（现 unified 合同）；缺则补 **prompt 资产**（clusters/writing 或 search skill 一句环境事实：「本轮检索由宿主一次 web 完成，证据见 observation」）。  
2. **不**改 dual/rag synthesis 分支条件（仅 search_host_web 时少一轮 retrieve 上下文）。  
3. `max_iterations` 下调后确保 synthesis 仍能跑完。

### Slice D — 清理与观测

1. search 模式不再依赖「模型必须写 `client.web`」——capability 文案 `prompts/capabilities/web/SKILL.md` 对 **search-only** 改为环境事实：宿主一次检索；**dual 仍描述 SaC fan-out**（可用 applicable_modes / 分文件，避免 dual 误读）。  
2. Telemetry：`retrieve_strategy=host_web`、latency 分段（deepseek_ms / crw_ms / synthesis_ms）。  
3. 删除或停用 search 上已无意义的 soft-baseline / 空转 early-stop 对 codegen 的依赖（保留 generic 安全）。

### Slice E — 回归

| 用例 | 期望 |
|------|------|
| search-only 简单问答 | 无 sandbox timeout；有 sources；有合成答 |
| search + DeepSeek 慢 | stream 不截断；总时长 ~ 一次 web + 一次 synthesis |
| rag-only | 行为与现网一致（codegen + dense） |
| dual（KB+web） | 仍 SaC；可 `client.web`；合并 `[[web:n]]` + SELECTED |
| 单测 | strategy 判定；host path 注入 ToolResult 形状；不误触发 dual |

**Verify 命令（实现后，需用户同意再跑）**：

```bash
cargo test -p agent-loop --lib host_web -- --nocapture   # 新测过滤名
cargo test -p avrag-search --lib
cargo build -p avrag-api --bin avrag-api
# 本机：新 session agent_type=search 问「什么是 BYOK」
```

## 7. 非目标 / 明确不做

- 不在 dual 上启用 `host_web`（避免「一次 web 替代 dense 规划」）。  
- 不把 DeepSeek 流式 token 直接刷成用户气泡。  
- 不重做引用体系；沿用 SearchResponse → `[[web:n]]`。  
- 不引入长期 feature flag 双轨（P6）；灰度若需要用 **短命 env** 仅 dev，合并前删除。  
- 不在本方案修 CRW CDP（独立工单）；host 路径仍走现有 enrich。

## 8. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 单次 web 源少（裸 query ~2 URL） | CRW 补 snippet；synthesis 基于摘要；后续可「极轻 input 要 3–5 源」A/B |
| 无 fan-out 双语漏检 | 首版接受；热修：host 固定双语两次 search 并行（仍非 LLM SaC） |
| verify 关掉质量降 | 监控用户反馈；再开 verify×1 |
| observation 泄漏进用户气泡 | 既有出站闸；synthesis 合同 prose_only |
| 误判 dual 为 host_web | 单测：primitives 含 dense ⇒ false |

## 9. 工作量粗估

| Slice | 量级 |
|-------|------|
| A 配置+判定 | ~0.5h |
| B host retrieve | ~2–3h |
| C synthesis/prompt | ~1h |
| D 文案/telemetry | ~1h |
| E 测+本机回归 | ~1h |
| **合计** | **约 0.5–1 人日** |

## 10. 验收标准（Done）

1. search-only：日志可见 **一次** `search ok`（或 brave fallback），**无** python sandbox codegen 多轮（或仅 synthesis 无 bridge web）。  
2. 用户气泡为模型合成文 + 可用 `[[web:n]]`；非 offline 免责声明常态。  
3. rag / dual 相关单测与路径无回归。  
4. 本方案文档与 `modes/search.yaml` 一致。

## 11. 建议实现顺序

**A → B → E 烟雾 → C → D → 本机 UI 验收**。  
B 是唯一行为开关；A 可空配先写死 search id 判定，再收拢到 YAML。

---

**一句话**：search-only = host 一次 web 取证 + 薄 synthesis；dual/rag 继续 SaC，共用 web 叶子，合并仍吃 hits。
