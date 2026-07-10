# Product App 残留清理计划（TN APPROVE 后 P2）

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-10 |
| 状态 | **Done — P2-1–P2-5 complete** |
| 起因 | TN 全量复审 **APPROVE**；剩余空壳类型 / dead 字段 / 双名 / 胖 AppState 延后项 |
| 约束 | Solo L1；行为保持；**优先删死代码**；不扩 CI |
| 上游 | Phase A–C Done：架构迁移、TN R0–R5、wrapper slim S0–S5 |
| 权威入口 | 项目根 [`AGENTS.md`](../../AGENTS.md) §8、[`CLAUDE.md`](../../CLAUDE.md) Product App 节 |

---

## 0. 落地后现状（勿回退）

```text
Transport → conversation().execute[_stream]
              ├ write → execute_pipeline(Write) → run_write_mode
              └ else  → execute_pipeline(Agent) → dispatch_agent_mode

AgentApp  → sessions / search / citations / runtime_tools / usage
（无 WriteApp）
write_refine → tool_specs_for_pool only（∉ SkillRegistry/ToolCatalog）
AppState  → composition root 文档 + face 工厂；禁止新业务方法
```

---

## 1. 残留项 → 结果

| ID | 残留 | 结果 |
|----|------|------|
| **P2-1** | `WriteApp` 空壳 | **Done** — 删除 `write.rs` / `state.write()`；统一 `app_chat::is_write_agent_type` |
| **P2-2** | dead `auth` 字段 | **Done** — ConversationApp / AgentApp 仅持 `chat` |
| **P2-3** | `docs()` 双名 | **Done** — 移除 `docs()`；调用改 `workspace()` |
| **P2-4** | 四入口 pipeline | **Done** — `execute_pipeline` / `execute_pipeline_stream` + `PipelineLane` |
| **P2-5** | 胖 AppState composition | **Done（纪律级）** — `AppState` 文档化为 composition root + face 工厂；**不**做真 `Arc<*App>` 大搬家 |

---

## 2. 非目标（仍适用）

- 再引入新的 Product App 包装层  
- Write 进 ToolCatalog  
- 大爆炸 `Arc<*App>` 重写  
- 强制真 LLM / 全 Playwright  

---

## 3. 验证

```bash
export CARGO_BUILD_JOBS=2
cargo test -p app-bootstrap --lib product_apps
cargo test -p app-chat --lib pipeline
bash scripts/test-l1.sh
# rg: WriteApp 生产零引用；docs() 产品访问器已删
```

---

## 4. 变更记录

| 日期 | 说明 |
|------|------|
| 2026-07-10 | 初稿：TN APPROVE 后 P2 残留；架构纪律写入 AGENTS/CLAUDE |
| 2026-07-10 | **P2-1–P2-5 代码落地**（中断后补提交验证） |
