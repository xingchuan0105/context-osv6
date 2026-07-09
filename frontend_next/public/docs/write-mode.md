# Write 模式（长文写作）/ Write Mode

Stable link: `/docs/write-mode.md`

## 1. 概述 / Overview

**Write 模式** 是 Context-OS 的第四种对话模式，与 `chat` / `rag` / `search` 同级。它围绕一个主题自动撰写长文：调研资料、规划大纲、分段起草、统计指纹精修、校验交付。

**Write mode** is the fourth conversation mode in Context-OS, on par with `chat` / `rag` / `search`. Given a topic, it automatically writes a long-form article: research → skeleton → draft → fingerprint-driven refinement → validate.

| 模式 / Mode | `agent_type` | 典型调用数 / Typical LLM calls | 联网 / Internet | 输出 / Output |
|------|------------|------------|------|------|
| chat | `chat` | 1–2 | 否 / No | 短答 / Short answer |
| rag | `rag` | 2–6 | 否* / No* | grounded 答 / Grounded answer |
| search | `search` | 2–6 | 是 / Yes | 网页综合答 / Web-synthesized answer |
| **write** | **`write`** | **10–20** | **是 / Yes** | **长文 + 引用 / Long-form article + citations** |

\* RAG 本身不强制联网；Write 的 web research worker 需要 `web_search`。

## 2. 流水线阶段 / Pipeline Stages

```
research → skeleton → draft → diagnose → WriteRefine → validate
```

| 阶段 / Phase | 说明 / Description |
|------|------|
| `research` | 双路调研：并行 RAG（知识库）+ Search（网络），压缩为 MaterialCard |
| `skeleton` | 规划文章大纲与分节 |
| `draft` | 分段起草正文 |
| `diagnose` | 分析初稿统计指纹（cv / hapax / zipf / burstiness） |
| `refine` | WriteRefine ReAct 精修 loop（6 轮硬上限，gate 下 3 次有效 revise） |
| `validate` | 校验指纹 band，软结束（未全过仍可交付） |

**Persona**（随机人格）和 **WriteRefine**（指纹精修）是 Write 流水线的内置能力，**不单独暴露为 API**。每篇自动生成随机人格小传，降低跨篇同质腔调；用户不可配置。

Persona and WriteRefine are built-in capabilities of the Write pipeline and are **not exposed as separate APIs**. A random persona is auto-generated per article to reduce cross-article tonal homogenization; users cannot configure it.

## 3. REST / SSE 调用

```
POST /api/v1/chat
```

```json
{
  "query": "<写作主题，非闲聊 / writing topic, not a chat message>",
  "notebook_id": "<workspace uuid>",
  "agent_type": "write",
  "doc_scope": [],
  "stream": true,
  "debug": false
}
```

| 字段 / Field | 说明 / Description |
|------|------|
| `query` | 主题或写作任务描述 / Topic or writing task description |
| `notebook_id` | 工作区 ID / Workspace ID |
| `doc_scope` | 可选；限制知识库调研范围（传给 RAG worker）/ Optional; limit knowledge-base research scope |
| `stream` | 建议 `true`（耗时长）/ Recommended `true` (long-running) |
| `debug` | `true` 时 `done` 含 `write_result`（指纹、revise 轮次、token 等）/ When `true`, `done` includes `write_result` |

> **禁止 / Forbidden**: `agent_type: "write_refine"` — 返回 `400 write_refine_not_user_selectable`。WriteRefine 是内部子循环，不可作为顶层模式。

## 4. SSE 事件 / SSE Events

| event | 写作模式要点 / Write-mode notes |
|-------|----------------|
| `activity` | `phase`: `research` / `skeleton` / `draft` / `diagnose` / `refine` / `validate`；`title` 为英文阶段说明 / English stage description |
| `token` | 文章正文流式输出（节间可能有停顿）/ Article body streaming (may pause between sections) |
| `citations` | 调研来源引用 / Research source citations |
| `done` | `agent_type=write`；可选 `degrade_trace` / Optional `degrade_trace` |
| `trace` | `debug=true` 时含 `tool_result.write_refine_*` / When `debug=true`, includes `write_refine_*` tool results |

## 5. 特点 / Features

- **随机 Persona / Random Persona**: 每篇自动生成人格小传，降低跨篇同质腔调（用户不可配）/ Auto-generated per article to reduce tonal homogenization (not user-configurable)。
- **指纹精修 / Fingerprint Refinement**: 统计 band（cv / hapax / zipf / burstiness）驱动 WriteRefine；软结束，band 未全过仍可交付 / Statistical bands drive WriteRefine; soft exit, article is delivered even if not all bands pass。
- **双路调研 / Dual-path Research**: 并行 RAG（知识库）+ Search（网络），压缩为 MaterialCard / Parallel RAG + web search, compressed into MaterialCards。
- **需联网 / Requires Internet**: `web_search` 用于调研与精修内补检索 / `web_search` used for research and in-refine supplementary retrieval。

## 6. 用量与预期 / Usage & Expectations

> 数据来自 gate 跑批 `heavytail-out/1783430709`（10 topic persona gate）。

| 指标 / Metric | 典型范围 / Typical range | 备注 / Notes |
|------|----------|------|
| LLM 调用 / LLM calls | 10–20 次/篇 / per article | 含 2 路调研 worker / Including 2 research workers |
| Token（全文）/ Token (full) | 约 10万–20万/篇 / ~100k–200k per article | 精修段 alone 约 3.5万–11万 / Refine segment alone ~35k–110k |
| 墙钟 / Wall clock | 2–5 分钟 / 2–5 minutes | 视主题与网络 / Depends on topic and network |
| 相对 chat/RAG | 约 8–15× 单次问答 token | 量级参考 / Order-of-magnitude reference |
| Band 过关率 / Band pass rate | gate 8/10（4/4 band） | 未全过时有 `validation_warning` / `validation_warning` when not all pass |

## 7. 降级与 trace / Degradation & Trace

| stage | 含义 / Meaning |
|-------|-------|
| `write:research` | `research_degraded` — 单路调研失败 / One research path failed |
| `write:refine` / `write:validate` | 指纹 band 未全过 / Fingerprint bands not fully satisfied |
| `write:persona` | `persona:leak` — 人格术语泄漏 / Persona terminology leaked |

## 8. 高级参数（内部 / 后续）/ Advanced Parameters (Internal / Future)

`persona_seed` / `persona_replay` / `no_persona` 经 `AgentRequest.metadata` 解析，**当前 `ChatRequest` 无 metadata 字段**。默认自动生成人格；回放仅用于实验脚本。不在公开 API 承诺 v1 字段。

`persona_seed` / `persona_replay` / `no_persona` are parsed from `AgentRequest.metadata`. **The current `ChatRequest` has no `metadata` field**, so these are not exposed in the public API v1. Default: auto-generate persona; replay is for experiment scripts only.

## 9. 参考 / References

| 资源 / Resource | 路径 / Path |
|------|------|
| Writer orchestrator | `avrag-rs/crates/app-chat/src/writer/mod.rs` |
| WriteRefine loop | `avrag-rs/crates/app-chat/src/writer/refine_loop.rs` |
| 模式路由 / Mode routing | `avrag-rs/crates/app-chat/src/chat/pipeline_steps.rs` |
| 设计规格 / Design spec | `avrag-rs/docs/superpowers/specs/2026-07-06-heavytail-writer-v2-design.md` |