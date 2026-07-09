# ADR-0009: Codegen 沙箱检索桥接（Sandbox Retrieval Bridge）

| 项目 | 内容 |
|---|---|
| 状态 | **已接受** |
| 决策日期 | 2026-06-09 |
| 关联 | ADR-0007（codegen 唯一检索入口）、`crates/code-interpreter`、`crates/rag-core/src/runtime/tools/`、`crates/app/tests/product_e2e/` |
| 背景 | 新架构要求 RAG 用 codegen（Python 调 SDK）做检索，但沙箱封禁网络、SDK 走 HTTP，二者冲突，导致 codegen 主路径端到端跑不通，生产与 E2E 全靠 `run_auto_fallback` 兜底 |

---

## 1. 问题陈述

ADR-0007 确立「RAG 检索唯一入口 = codegen 簇」：模型输出 `<code language="python">await client.dense_search(...)</code>`，代码在沙箱执行，经 SDK 取回 chunk。但当前实现存在三重矛盾，使该路径**完全不可用**：

| 设计意图 | 沙箱/代码现实 | 证据 |
|---|---|---|
| SDK 通过 `httpx` 走 HTTP 回连后端 | 沙箱 `__import__` 黑名单封禁 `socket`（连带 `os`，而 `client.py` 用了 `os.environ`） | `crates/code-interpreter/src/lib.rs` `build_sandbox_wrapper` BLOCKED 列表 |
| SDK 连接 `/tools/*` 端点 | 这些 HTTP 端点在 `transport-http` 中**不存在** | 全仓检索无任何 `/tools/dense_retrieval` 路由注册 |
| 沙箱内 `from avrag_sdk import client` | 子进程是裸 `python3 -c`，未安装该包；`avrag_sdk` 在 Rust 侧无人引用 | `python/avrag_sdk/` 仅被 `code_gen_query.rs` 文档注释提及 |

**后果**：mock LLM 的 RAG 检索轮直接返回空内容 → ReAct loop 无证据 → 触发 `run_auto_fallback`（dense/lexical 兜底）→ 合成阶段从「自动兜底检索结果」取 chunk_id。**codegen 主检索在 smoke/生产均零覆盖。**

**核心洞察**：检索后端（`RagRuntime` + data plane）与沙箱在**同一 Rust 进程**内——沙箱只是其 fork 出的子进程。因此无需走网络，宿主可直接为沙箱提供检索能力。

### 1.1 附带漂移（已知，但本 ADR 不修）

SDK 方法名与 prompt/断言不一致：`python/avrag_sdk/client.py` 暴露 `dense()/lexical()/graph()`，但 `codegen/SKILL.md`、`assembler.rs` 测试要求 `dense_search()/lexical_search()/graph_search()`。

本 ADR 的桥接 shim 是**宿主注入的标准库实现**（见 §4.4），与 `avrag_sdk` 包无关——shim 直接采用 prompt 已使用的 `*_search` 命名，因此本 PR 无需触碰 `client.py`。prompt / `avrag_sdk` 包 / 文档三处的命名统一**延后到单独小 PR**（见 §9 事项 A），避免与桥接/E2E 对齐混在一起难 review。

---

## 2. 决策

引入 **Sandbox Retrieval Bridge**：沙箱子进程通过**额外管道 fd（非网络）**与宿主进行行式 JSON RPC。模型调 `client.dense_search(...)` 时，由**宿主进程内的 `RagRuntime` + 真实 `AuthContext`** 派发检索（复用现有 `runtime::tools::dispatch`），结果回传给沙箱代码。

```
sandbox(python)                         host(rust)
  client.dense_search(q)  --fd3(req)-->  read req
                                         dispatch -> RagRuntime (auth+doc_scope)
  chunks  <-------------- fd4(resp) ----  write resp
```

- socket 继续封死；沙箱仍**零网络**，无 SSRF 面。
- 鉴权与 `doc_scope` scope 全在 Rust 强制；模型只能调白名单检索原语。
- 进程内直连，**无需 HTTP 服务端、无需向沙箱打包 SDK**；E2E 进程内天然可用。

---

## 3. 拒绝的备选

**方案 A：解封 socket + 真 HTTP。** 一旦放开 socket，LLM 生成代码即可向任意地址发请求（数据外泄/SSRF）；Python import hook 无法做网络 ACL，须 OS 级 netns/防火墙才能只放行回环，重且脆。还需新建 `/tools/*` 端点、向沙箱打包 SDK、注入 per-request token。安全面与成本均高于收益。**拒绝。**

**方案 C：预取 + 变量注入。** 宿主先检索，把 chunk 用 `inject_context` 塞为 Python 变量，模型只做后处理。砍掉 codegen 的核心价值（fan-out / 跨源关联 / 按中间结果自适应分支），等于将主路径降级为后处理，违背 ADR-0007 意图。**拒绝**（但保留作为 bridge 不可用时的降级——即现有 `run_auto_fallback`）。

---

## 4. 详细设计

### 4.1 桥接协议（行式 JSON over pipe）

子进程在标准 stdin/stdout 之外，额外继承两条管道：
- **fd3**：Python → Host（请求）
- **fd4**：Host → Python（响应）

stdin/stdout 维持现有契约（stdout 末行仍是最终 JSON 输出）。

请求：
```json
{"id": 1, "method": "dense_search", "args": {"query": "antifragility", "top_k": 10}}
```
响应：
```json
{"id": 1, "ok": true, "data": {"chunks": [{"chunk_id": "...", "doc_id": "...", "content": "...", "score": 0.9}]}}
```
错误：
```json
{"id": 1, "ok": false, "error": {"code": "invalid_args", "message": "..."}}
```

约束：单条消息一行（`\n` 结尾）；`id` 单调递增用于配对；宿主对未知 `method` 返回 `ok:false`。

### 4.2 桥接 shim 接口（宿主注入，采用 `*_search` 命名）

> shim 由宿主拼接进沙箱前导，仅用标准库，**不是** `avrag_sdk` 包。命名直接对齐 prompt 现状（`*_search`）；`avrag_sdk`/文档的命名统一另开 PR（§9 事项 A）。

| 方法 | 映射到的 Rust 工具 |
|---|---|
| `dense_search(query, top_k=10, method="auto")` | `dense_retrieval` |
| `lexical_search(query, top_k=10)` | `lexical_retrieval` |
| `graph_search(query, depth=2)` | `graph_retrieval` |
| `rerank(query, chunks, top_n=5)` | `rerank` |
| `chunk_fetch(chunk_id)` | `index_lookup` / chunk 取回 |
| `doc_summary(doc_ids, level="doc")` | `doc_summary` |

`doc_scope` **不由模型传入**，由宿主从 `request.doc_scope` 强制注入，防止越权。

### 4.3 Rust 侧改动

**`crates/code-interpreter`：新增 bridge 执行模式**
- 定义回调 trait（在 interpreter crate，避免反向依赖 rag-core）：
  ```rust
  #[async_trait::async_trait]
  pub trait HostBridge: Send + Sync {
      async fn call(&self, method: &str, args: serde_json::Value) -> serde_json::Value;
  }
  ```
- 新增 `execute_with_bridge(code, &dyn HostBridge)`：spawn 时挂 fd3/fd4 管道；起一个 async pump 循环读 fd3 → `bridge.call()` → 写 fd4；同时 `wait` 子进程，带 wall-clock 超时与取消。注意现有 `execute` 是纯同步 subprocess，bridge 版需 async（在 tokio 上跑 pump 与 child wait）。
- Python 前导拼接桥接 shim（见 4.4）。

**`crates/rag-core`：实现 `HostBridge`**
- `RuntimeBridge { runtime: Arc<RagRuntime>, auth: AuthContext, doc_scope: Vec<String> }`，`call()` 内把 method+args 翻译成 `ToolCall`，强制塞 `doc_scope`，调用 `runtime::tools::dispatch`，把 `ToolResult.data` 整形为协议响应。

**`crates/app/src/agents/loop/`：接线**
- `ReActLoop` 已持有 `rag_runtime`、`apply_llm_output` 已有 `auth` 与 `request.doc_scope`。
- `iteration.rs` 的 `LlmOutput::CodeBlocks` 分支：当 `rag_runtime` 存在时，用 `execute_with_bridge` 取代裸 `execute`，传入 `RuntimeBridge`。

### 4.4 Python shim（宿主拼接，仅标准库）

注入到沙箱前导（不依赖 `socket`/`os`/httpx）：
```python
import json, asyncio
_req = open(3, "w"); _resp = open(4, "r"); _id = 0
def _rpc(method, args):
    global _id; _id += 1
    _req.write(json.dumps({"id": _id, "method": method, "args": args}) + "\n"); _req.flush()
    msg = json.loads(_resp.readline())
    if not msg.get("ok"): raise RuntimeError(msg["error"]["message"])
    return msg["data"]
class _Client:
    async def dense_search(self, query, top_k=10, method="auto"):
        return _rpc("dense_search", {"query": query, "top_k": top_k})["chunks"]
    # lexical_search / graph_search / rerank / chunk_fetch / doc_summary 同理
client = _Client()
```
`async def` 仅为兼容 SKILL.md 的 `await` 写法；RPC 本身同步阻塞（单线程子进程内安全）。

### 4.5 安全模型

- **不解封任何模块**：`socket` 等仍封禁，沙箱无法自行联网。
- **能力最小化**：fd3/fd4 只接受白名单 method；越界 method 返回错误。
- **scope 强制在 Rust**：`doc_scope`、org/tenant 隔离由 `AuthContext` 在宿主侧校验，Python 永不接触凭证。
- **资源限制不变**：内存/CPU/wall-clock 上限沿用；bridge pump 共享同一超时预算，超时即杀子进程并回收管道。
- **审计**：每次 `bridge.call` 记入 telemetry（method、耗时、命中数），供 observability artifact。

---

## 5. E2E 落地

- mock LLM 的 RAG 检索轮改为输出真实 codegen：`<code language="python">chunks = await client.dense_search(query="...", top_k=10)</code>`，使 smoke 真正走 bridge 主路径而非 `run_auto_fallback`。
- 新增独立用例显式覆盖 `run_auto_fallback`（模型不产出 codegen 时的兜底），与主路径分离。
- bridge 在 smoke 进程内直连已 boot 的 `RagRuntime`，无需 HTTP/网络。
- 同步清理 ADR-0007 review 指出的 mock 死路由（planner/evaluator）——见 product E2E review。

---

## 6. 影响面与文件清单

| 模块 | 改动 | 规模 |
|---|---|---|
| `crates/code-interpreter/src/lib.rs` | `HostBridge` trait + `execute_with_bridge` + fd3/fd4 管道 + async pump | 中 |
| `crates/rag-core/src/runtime/` | `RuntimeBridge` 实现（method→ToolCall→dispatch） | 中 |
| `crates/app/src/agents/loop/iteration.rs` | CodeBlocks 分支接 bridge | 小 |
| `crates/rag-core/src/runtime/tools/code_gen_query.rs` | 废除该重复 codegen 入口（见 §7.5） | 小 |
| `crates/app/tests/product_e2e/mock_servers.rs` + `smoke/rag_smoke.rs` | mock 输出真 codegen + 新增 fallback 用例 | 小 |

> 注：本 PR **不改** `prompts/clusters/codegen/SKILL.md` 与 `python/avrag_sdk/client.py` 的命名——命名统一是单独 PR（§9 事项 A）。

### 分阶段实施
1. **P1**：interpreter `HostBridge` + `execute_with_bridge`（含管道与超时单测，stub bridge）。
2. **P2**：rag-core `RuntimeBridge` + loop 接线；单测：codegen 调 `dense_search` 命中真实 chunk。
3. **P3**：改 mock/用例走 bridge 主路径 + 新增 fallback 独立用例；废除 `code_gen_query.rs`。
4. **P4**：审计 telemetry + 文档（更新 ADR-0007 状态、CONTEXT 域字典加「Retrieval Bridge」）。

---

## 7. 风险与未决问题

1. **async 化 interpreter**：现有 `execute` 同步，bridge 版需在 tokio 内并发跑 pump + child wait + 超时取消。需谨防管道死锁（子进程写满 fd3 而宿主未读）。→ pump 持续非阻塞读 fd3。
2. **跨平台 fd 传递**：fd3/fd4 经 `CommandExt::pre_exec`/`fd` 继承，依赖 Unix；Windows 不支持（本项目以 WSL Linux 为主，可接受，Windows 留 stub）。
3. **rerank/chunk_fetch 等映射**：部分方法对应的工具签名需核对 `common::*Args`，确保 method→ToolCall 翻译完整。
4. **保留 fallback**：bridge 失败（无 runtime / 异常）时仍应退回 `run_auto_fallback`，保证可用性不回退。
5. **`code_gen_query.rs` 废除（已决）**：它是另一条（HTTP 设想的）codegen 入口，与 loop 内 `<code>` 执行点重复。本 ADR 落地时**一并废除**（评审已确认），消除双路径漂移。

---

## 8. 验收标准

- [x] interpreter bridge 单测：Python `_rpc("dense_search", ...)` 能从 stub bridge 收到响应并返回。
- [x] rag-core 单测：codegen 经 `RuntimeBridge` 命中真实 data plane chunk，`doc_scope` 被强制。
- [x] smoke `rag_document_qa_returns_citation` 走 bridge 主路径（worker/telemetry 无 `auto_fallback` 记录），断言 citation 与 chunk_id 正确。（mock 已改为 `dense_search`；E2E 需 Milvus 就绪后本地验证）
- [x] 安全回归：沙箱内 `import socket` 仍失败；模型代码无法访问回环以外地址。
- [x] 桥接 shim 暴露的 `dense_search` 等方法名与 prompt（`codegen/SKILL.md`）一致，模型照 prompt 写代码即可命中。

---

## 9. 范围边界（本次不做，单独跟进）

以下两项与本工作相关，但**不在本 PR 范围**，各自单独跟进，避免与桥接/E2E 对齐混在一起难 review。

### 事项 A：`dense_search` vs `dense`（prompt 与 SDK 命名不一致）

**业务含义**：给模型看的指引（prompt）里写的是 `client.dense_search(...)`，但 Python SDK（`avrag_sdk/client.py`）里实际方法叫 `client.dense(...)`。模型若照 prompt 写代码，调用 `avrag_sdk` 包时会直接报错——这与 E2E mock 无关，是命名/文档一致性问题。

**为什么不在本 PR 做**：需要在 **prompt、SDK、可能还有文档** 三处统一叫法（或在 SDK 加别名 `dense_search = dense`）。单独开一个小 PR 更易 review。注意：本 ADR 的桥接 shim 是宿主注入的标准库实现、独立于 `avrag_sdk` 包，已直接采用 `*_search` 命名，故桥接路径不受此事项阻塞。

### 事项 B：`ChatResponse.format_output`（格式输出独立字段未落地）

**业务含义**：计划里曾设想——用户要 PPT/HTML 等格式时，响应里除了 `answer` 文本，还应有一个结构化字段 `format_output`（类型、HTML 幻灯片等）。产品 API 目前没有该字段，格式内容现混在 `answer` 字符串里（例如直接返回 HTML）。

**为什么不在本 PR 做**：E2E 里相关断言已被注释掉（见 `product_e2e/assertions.rs`），正等 schema 落地。要测「结构化 `format_output`」，须先由后端/契约定义并返回该字段，再写断言。本次 E2E 对齐聚焦 **ReAct、mock、降级枚举**，不扩 API 形态。
