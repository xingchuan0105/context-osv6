# 交接：Search 延迟诊断 + Agent Loop 减轮次优化（进行中）

- **日期**: 2026-08-11  
- **状态**: 桥接真并发**已收口**（红测修清、生产并行验证）；软基线**已 A/B 后关闭**（-50s）；**新瓶颈 = synthesis/verify 尾段（证据薄→先验补→verify fail→重写）**，下一刀 = 首块宽扇出提示  
- **范围**: 本机 only；VPS 未部署  
- **后续窗口**: 从本文 §8.4 继续（首块宽扇出）  

---

> **2026-08-11 第二窗更新见 §8**：WIP 收口、三轮同题对比、去基线实验、新瓶颈根因。§4–§7 保留为第一窗原始记录。

---

## 1. 用户可见问题回顾

| Session | 墙钟 | 现象 |
|---------|------|------|
| `dde25a1e…`（旧 API） | ~1min | search 仍成功调 **dense**（binary 早于 `e0fc7055`） |
| `3e7105de…`（dense 门禁后） | **~79s** | 仅 web×2；verify **fail+ceiling** 吃 ~40s |
| `9212f0e4…`（软基线后） | **~73s** | **web×4 + fetch×2(empty)**；verify **pass**；retrieve **更重** |

**结论（9212 相对 3e71）**  
- dense 误调：已好  
- 总墙钟：几乎无改善（−6s）  
- retrieve 效率：**未改善**（更多圈、更多 DeepSeek web）  
- verify：有改善（一次 pass）  

DeepSeek Anthropic `web_search` 单次约 **8–14s**；两次串行 ≈ 25s 级。

---

## 2. 根因分层

### 2.1 为何 `asyncio.gather` 不加速（P0 真因）

| 层 | 历史行为 | 影响 |
|----|----------|------|
| Python shim `_rpc` | **同步** write → **阻塞** readline | gather 也无法重叠 |
| Host `run_bridge_pump_sync` | **一问一答** `block_on(call)` 再读下一行 | 墙钟 = 延迟求和 |

日志印证：两次 `search ok` 时间戳相差 ~10–14s，不是并行完成。

### 2.2 为何软基线 3/2 没压住轮次

- 仅 **观察 + 提示**，无硬停、无「已有命中 / 近似 query」熔断  
- 模型仍开第 2 波近似 web + 空 fetch  
- product_rounds=4（retrieve 多圈）  

### 2.3 为何整问仍 ~70s+

```text
题卡+规划 ~5–10s
+ 多次 DeepSeek web（串行时 ×N×10s）
+ 合成 ~10s
+ verify ~5–20s（fail 时再 +收束）
```

---

## 3. 已落地（相对完整、可保留）

### 3.1 Search / dense 策略（先前 commit + 本机重启）

- `dense` 仅 `SdkCapability::RAG`；search-only 不挂 dense  
- search 极简题卡 `query-card-search.system.md`  
- chat L0：跳过题卡、iter cap 2  

### 3.2 B2 DeepSeek web（本机测过 live smoke）

- `SEARCH_PROVIDER=deepseek_web_brave` 默认  
- Anthropic `web_search_20250305` → `SearchResponse`；空/错 → Brave  
- 文档：`docs/engineering/2026-08-11-deepseek-web-search-b2.md`  
- **未 VPS 部署**  

### 3.3 软基线预算（可用）

- `BudgetConfig.baseline_iterations`（默认 2）  
- `<loop_budget round baseline_rounds max_rounds …>`  
- 超基线：`prompts/loop/budget-pace-over-baseline.tmpl.md`（如 **3/2**）  
- 近硬顶：`prompts/loop/budget-pace-near-ceiling.tmpl.md`  
- 提示：`agent-base` / `web/SKILL` / `web/contract` / KB `api-detail`  
- 单测：`cargo test -p agent-loop --lib budget_hint` → **绿**  
- 设计：`docs/engineering/2026-08-11-retrieve-pace-baseline-and-fanout.md`  

### 3.4 最佳实践方案文档（产品对照）

- `docs/engineering/2026-08-11-agent-loop-fewer-rounds-best-practices.md`  
- 方向：ReWOO 快路径语义 + 真并行 + 少步骤提示 + 软/硬防护；**不**单砍 max_iter  

---

## 4. WIP：桥接真并发（**已于第二窗收口**，根因与修法见 §8.1）

### 4.1 意图

让 `asyncio.gather(client.web(...), client.web(...))` 墙钟 ≈ **max(latency)** 而非 **sum**。

### 4.2 已改代码（`avrag-rs/crates/code-interpreter/src/bridge.rs`）

| 改动 | 说明 |
|------|------|
| Python `_rpc` | 独立 **reader 线程** + `id` 解复用 + `threading.Event` |
| 方法包装 | `await asyncio.to_thread(_rpc, …)` 再 `return _data{py_return}` |
| Host pump | 每请求 **OS 线程** + `runtime.handle().block_on(bridge.call)`，写回可乱序 |
| 沙箱 import | 包装器预加载 `threading` / `concurrent.futures`；`_safe_import` 允许 **已在 sys.modules** 的 blocked 名再导入（否则 `to_thread` 拉 thread 失败） |

### 4.3 已绿

```bash
cargo test -p avrag-code-interpreter --lib bridge_gather
# bridge_gather_runs_rpcs_concurrently  ~0.32s（2×250ms sleep → 并发）
cargo test -p agent-loop --lib budget_hint  # 4 passed
```

### 4.4 仍红（接手时优先修）

```bash
cargo test -p avrag-code-interpreter --lib bridge_dense
# bridge_dense_returns_chunks_in_stdout
#   success=true exit=0 但 stdout=""（print 未进 cap 或主路径未真正 print）

cargo test -p avrag-code-interpreter --lib bridge_blocks
# bridge_blocks_socket_import
#   期望 import socket 失败进 stderr；现 stderr 空且 success=true
```

`lib.rs` 的 dense 测试里可能仍留有调试行：

```text
eprintln!("DUMP success=...");
```

**请删掉再提交。**

### 4.5 排障线索（未闭环）

1. **dense 空 stdout + success**：JSON 包装路径认为成功，但 `print(json.dumps(chunks))` 未进入 `ExecutionResult.stdout`。优先核对：  
   - 生成 shim 中 `dense` 的 `to_thread` + `["chunks"]` 下标是否正确  
   - 主协程是否在 print 前异常被静默（正常应有 stderr）  
   - 对比「仅 calculator + print」与 dense 路径差异  
2. **socket 拦截**：`_safe_import` 改成「已加载可再 import」后，确认 `socket` 不在 `sys.modules` 时仍 raise；测例 `import socket` 在 `async def __avrag_main` 内，异常应进 `traceback.print_exc` → stderr。  
3. 若并发方案继续难收，**降级切片**：保留串行 RPC，另加宿主 `web_batch(queries)` + `join_all`（提示改用 batch；gather 仍串行但产品可控）。

---

## 5. 建议下一窗口工作序

1. **清 WIP 测试**  
   - 删 `DUMP` 调试  
   - 修 `bridge_dense` / `bridge_blocks` 至绿  
   - 全量：`cargo test -p avrag-code-interpreter --lib`  
2. **重编重启本机 API**（仅当桥接绿）  
   ```bash
   cd avrag-rs && cargo build -p avrag-api
   # 停旧 pid（.dev-logs/avrag-api.pid），nohup target/debug/avrag-api
   curl -s http://127.0.0.1:8080/health
   ```  
3. **同一问「什么是 BYOK？」回归**  
   - 期望：首块 2×web 日志时间戳接近；`web` 次数 ≤2（除非 fetch 有依赖）  
   - product_rounds 降；墙钟目标先看是否进入 **40–50s** 量级（DeepSeek 仍贵）  
4. **P1 未做（可选）**  
   - 已有 web Ok 时超基线观察：写明「已有 N 条命中，近似 query 边际低」  
   - 相似 query 哈希熔断（P2）  
   - verify 策略另议（用户曾要求 search 保留 verify）  
5. **VPS**：仍等用户通知；B2 + 并发勿擅自 deploy  

---

## 6. 关键路径索引

| 类型 | 路径 |
|------|------|
| 最佳实践对照 | `docs/engineering/2026-08-11-agent-loop-fewer-rounds-best-practices.md` |
| 软基线设计 | `docs/engineering/2026-08-11-retrieve-pace-baseline-and-fanout.md` |
| B2 DeepSeek | `docs/engineering/2026-08-11-deepseek-web-search-b2.md` |
| B1 探针 | `docs/engineering/2026-08-11-deepseek-native-web-search-b1-probe.md` |
| 桥接实现 | `avrag-rs/crates/code-interpreter/src/bridge.rs` |
| 预算 hint | `avrag-rs/crates/agent-loop/src/react_loop/assembler.rs` |
| BudgetConfig | `…/policy/config/config_types.rs` |
| modes | `avrag-rs/modes/search.yaml`（`baseline_iterations: 2`） |
| API 日志 | `avrag-rs/.dev-logs/api.log` |
| 会话查库 | `chat_sessions` / `chat_messages` / `llm_usage_events`（RLS：`app.current_role=super_admin`） |

---

## 7. 一句话交接（第一窗）

**诊断清楚：总时长 ≈ 串行 DeepSeek web × 次数 + multi-retrieve +（可选）verify；软基线只是观察，挡不住多搜。**  
**已做：dense 门禁、B2、软基线提示、方案文档。**  
**未收口：桥接真并发半成品 — gather 单测绿，dense/socket 两测红，勿当生产可用；下一窗先修红测再重启 API 回归 BYOK。**

---

## 8. 第二窗验收（2026-08-11 凌晨）：并发收口 + 去基线实验

### 8.1 桥接并发收口（§4 WIP 已清）

两红测根因与修法（`crates/code-interpreter/src/bridge.rs`）：

| 红测 | 根因 | 修法 |
|------|------|------|
| `bridge_dense_returns_chunks_in_stdout`（success/exit0 但 stdout 空） | wrapper 把 `sys.stdout` 换成 StringIO 后不还原，结尾 `_real_stdout.write()` 只进缓冲区；带 reader/executor 线程退出时 finalize 不 flush，载荷丢失 | 写后显式 `_real_stdout.flush()` |
| `bridge_blocks_socket_import`（stderr 空且 success） | WIP 的「blocked 模块已在 sys.modules 则放行」把 asyncio 传递引入的 os/subprocess/socket/signal/posix/fcntl/sys **全部放开（沙箱逃逸级）**；且 `to_thread` 运行期 lazy 的 blocked 名（threading/os）来自 `concurrent/futures/thread.py` 顶层导入 | 预载 `concurrent.futures.thread`（lazy 导入在 hook 安装前完成）+ 恢复**严格**拦截 |

- `cargo test -p avrag-code-interpreter --lib`：**16 passed + 1 ignored**（gather 并发单测仍绿）
- 调试残留已清（lib.rs `DUMP`、bridge.rs 临时 dump）；API 已含全部修复重启

### 8.2 三轮同题「什么是 BYOK？」对比（本机，deepseek-v4-flash）

| | 旧 9212（基线在，串行） | 基线在+并发 | **去基线+并发** |
|---|---|---|---|
| 墙钟 | 73s | 130s | **80s** |
| retrieve | 4 web+2 fetch 串行 ~50s | 2 web 并行 ~15s | 2 web 并行 ~16s |
| synthesis 首稿 | 655 tok | 3571 tok | 2302 tok |
| verify | **一次 pass**（5s） | fail（35s）+ceiling 重写（33s） | fail（17s）+ceiling 重写（21s） |
| 尾段合计 | ~18s | ~105s | ~60s |

并行生产路径实证：用户手动 SAC+BYOK 双实体问（110s）首波 **4 个 web 在 1.3s 窗口齐回**（旧串行约 40s+）。

### 8.3 结论

1. **去基线 -50s**（130→80）：少了基线压力下那轮「要不要继续搜」的 codegen 犹豫，synthesis/verify 输出整体瘦身。已保留关闭状态：`modes/search.yaml` `baseline_iterations: 0`；agent-base / web SKILL / web contract 的基线提示已清（fan-out 并行指导保留）。**未清**：`modes/rag.yaml` baseline、KB `api-detail.md` 提示（rag 侧，本次未测）。
2. **verify-fail-rewrite 与基线无关**：去基线后 verify 照样 fail——根因是 **2 条 web 证据太薄 → synthesis 拿先验知识补细节（30 分钟撤销时限 / 腾讯云 KMS / HYOK / Entrust 起源）→ verify 证据闸 fail → ceiling 重写**，尾段 +40-70s。旧 run 证据厚（4 web+2 fetch）→ 草稿贴证据 → 一次 pass。
3. **题卡（search 模式）**：对 `other` 类查询无信息价值（~2.5-3s 纯开销），仅 `calculation/chitchat` 快路径分类 + `required_actions` 结构闸有实际作用；本轮未动。

### 8.4 下一刀（建议，未动）

**首块宽扇出提示**进 `prompts/capabilities/web/SKILL.md`：定义/背景类查询首块直接 3-4 路 query（定义、起源/提出者、变体如 HYOK、厂商支持）——证据厚 → synthesis 贴证据 → verify 一次过 → 墙钟目标 **35-40s**。只改提示，不动结构。

回归口径（可复用）：账号 `byok-regression@local.dev`（token 在 `/tmp/byok_token.txt`）、workspace `0d33b3e9-027b-4a08-8065-4175711d20c1`：

```bash
curl -X POST http://127.0.0.1:8080/api/v1/chat -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"query":"什么是 BYOK？","agent_type":"search","workspace_id":"0d33b3e9-…","stream":false}'
```

判定点：`llm_usage_events` 里 synthesis 首稿 out tok（目标 <1000）、verify verdict（目标 pass 一次过）、墙钟。

### 8.5 未做

- §8.4 未动刀；VPS 未部署（B2 + 并发 + 去基线均未上）；题卡未删；rag 侧基线残留未清；`verify` 策略未改（用户要求 search 保留 verify）。
