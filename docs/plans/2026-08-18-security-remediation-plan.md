# AI 产品安全修补计划（2026-08-18）

**状态**: W0–W5 代码在工作区；审查闭合见 [`2026-08-18-security-review-fixes.md`](2026-08-18-security-review-fixes.md)（P2-14 仍延期）  
**父文档**: [`docs/engineering/2026-08-18-security-audit-ai-product.md`](../engineering/2026-08-18-security-audit-ai-product.md)  
**范围**: `avrag-rs` + `frontend_next`（W4 cookie / W5 legal+upload）+ `desktop`（W5 CSP/IPC）  
**原则**: 无兼容税（删兜底，不留 shim）；每波是已工作产品上的一层；语言级钩子不视为安全边界。

审查已核验。本计划只编排修补顺序、取舍与验证门。系统提示词里「检索正文是数据」**已经写在** `prompts/system/lead-base.md` 与 `worker-sandbox.md`——审查 P1-5 的「缺声明」项下调，本计划不再补第二份禁令式文案。

---

## 0. 拍板项（开工前确认）

| # | 决策 | 推荐 | 备选 |
|---|---|---|---|
| D1 | 开工范围 | **W0 + W1**（三个 P0，先断致命三角） | 只做 W0（配置/鉴权，当天可合） |
| D2 | 沙箱终点 | **本波：进程级隔离**（`env_clear` + Unix `prlimit`/`pre_exec` + 修 wrapper）。语言钩子只作纵深 | 本波就上容器/microVM（工期大，另开计划） |
| D3 | 上传签名密钥 | **删硬编码兜底**；未配置则签名/校验一律失败。本地把随机值写入 `avrag-rs/.env` | 仅 release panic、debug 仍兜底（否决：本地即现网攻击面） |
| D4 | 代理头 | **`E2E_ENABLED` 不再开代理认证**；生产硬拒。本地/E2E 改设 `TRUST_PROXY_AUTH=true` | 保留 `E2E_ENABLED` 开代理、只加 `NODE_ENV!=production`（仍把两个旗标绑在一起） |
| D5 | 检索 `doc_ids=None` | **数据层 fail-closed**（`None` 与空 vec 一样返回空） | 只加 `workspace_id` 过滤、保留 owner 全库查询（仍是跨 workspace） |

未点名则按「推荐」执行。

---

## 1. 依赖与波次

```
W0 鉴权/密钥 fail-closed  ──独立──► 可单独合入
W1 沙箱隔离              ──独立──► 可与 W0 并行
         │
         ▼
W2 Agent 出入站（SSRF + 证据闸）  依赖 W1 已切断外传，否则中间层复活只减面不闭合三角
         │
         ▼
W3 检索租户（workspace / 非空 doc_ids）
         │
         ▼
W4 会话 cookie + CORS fail-closed
         │
         ▼
W5 计量/metrics、桌面 CSP/IPC、ClamAV fail-closed、摄取隐藏 Unicode
```

P2-14（解析器二进制隔离）仍不在本计划实施清单（需独立执行环境，近 W1 中期）。

---

## 2. W0 — 鉴权与密钥 fail-closed

**目标**: 去掉「知道源码常量 / 拨一个 env」即可接管的路径。产品正常登录/上传在密钥配好后行为不变。

### W0-A · P0-3 代理头与 `E2E_ENABLED` 解耦

**现状**: `proxy_auth_allowed`（`transport-http/src/middleware.rs:499-510`）在 `E2E_ENABLED=true` 时直接 `true`，不看 `NODE_ENV`。`/api/e2e/*` 已有 `NODE_ENV==production` 硬拒（`routes/e2e.rs:54`）。本地 `.env` 现为 `E2E_ENABLED=true`。

**改法**（删耦合，不留「E2E 仍可开代理」的兼容枝）:

```
proxy_auth_allowed:
  NODE_ENV == production  → 永远 false
  TRUST_PROXY_AUTH in {true,1,yes} → true   // 仅非 production
  !postgres_configured() → true             // 无 PG 的内存开发态
  其他 → false
```

`E2E_ENABLED` 只保留：`/api/e2e/*` 路由闸、边缘限流抬升（`middleware.rs:111-115`）。  
测试/本地：`product_e2e` bootstrap 与本地 `.env` 改设 `TRUST_PROXY_AUTH=true`，并保证 `NODE_ENV` 不是 `production`。`.env.example` 注释改成与实现一致。

**文件**: `middleware.rs`、`app/tests/product_e2e/test_context/{config,builder}.rs`、`avrag-rs/.env.example`、本地 `.env`（`TRUST_PROXY_AUTH=true`，不提交）。

**验证**（需同意后再跑，约 1–2 min）: `cargo test -p avrag-transport-http --lib`；加单测：`E2E_ENABLED=true` + `NODE_ENV=production` → 代理头被拒；非 production + `TRUST_PROXY_AUTH=true` → 仍接受。

### W0-B · P0-2 上传签名：删兜底密钥

**现状**: `upload_signing_secret()`（`app-core/src/config_helpers.rs:25-30`）回落 `"context-osv6-local-upload-secret"`。无 object path 且无 PG 时整段跳过校验（`infra_handlers.rs:217-222`）。比较用 `!=`（`storage_context.rs:115`）。

**改法**:

1. 删除常量。`AVRAG_UPLOAD_SIGNING_SECRET` 空/未设 → 签发与校验都返回明确错误（不 panic 整个进程，避免测试/worker 误伤；路由层 503/500）。
2. 无 object path：有 PG 或无 PG **一律拒传**（删 `None => {}` 跳过枝）。
3. 签名比较改 `subtle::ConstantTimeEq`（或 HMAC verify），已有 `hmac` 依赖则走 crate 提供的恒时接口。
4. 本地生成随机密钥写入 `avrag-rs/.env`（不提交）；`.env.example` 增加占位与一句「未设则上传关闭」。

**文件**: `config_helpers.rs`、`storage_context.rs`、`infra_handlers.rs`、`.env.example`、本地 `.env`。

**验证**（约 1 min）: `cargo test -p avrag-app-core --lib` + `cargo test -p avrag-transport-http --lib` 中上传/签名相关；单测：无密钥 → verify 失败；已知源码旧常量 → 失败。

### W0 不做

- 不改 JWT debug 兜底（release 已 panic，属 P2；放 W4 一并收）。
- 不改 512MB body 上限（消耗面，跟签名不是同一根因）。

---

## 3. W1 — 沙箱：切断致命三角的外传腿

**目标**: 间接注入即使骗到模型写逃逸代码，也读不到 API 进程密钥，且 Unix 内存/CPU 上限真正生效。  
**非目标**: 把 import 钩子做成「完整安全边界」。钩子只防呆；边界是 **空环境 + OS rlimit + 墙钟杀进程组**。

**现状**: `CodeInterpreter::execute`（`code-interpreter/src/lib.rs:146-153`）与 bridge spawn（`bridge.rs:813-821`）都不 `env_clear`。wrapper 先装 `_safe_import` 再 `import resource`，而 `resource` 在封锁名单 → rlimit 从未生效。`_original_import` 留在 `exec` 共享 globals。

**改法**（一层做完，不留「先修钩子、以后再清环境」的半成品）:

1. **Rust spawn（`execute` 与 `execute_with_bridge` 同一辅助函数）**  
   - `env_clear()`  
   - 只回填：`PATH`（解析到的 python 所在目录）、`HOME`/`TMPDIR`（指向本次 `TempDir`）、`LANG`/`LC_ALL=C.UTF-8`、`PYTHONDONTWRITEBYTECODE=1`  
   - bridge 额外只设 `BRIDGE_TOKEN_ENV`（已有）  
   - Unix：`pre_exec` / `prlimit` 设 `RLIMIT_AS`、`RLIMIT_CPU`（父进程设，不依赖 Python `resource`）  
   - Windows：保持现有 Job Object（`lib.rs:172` / bridge job）
2. **Wrapper**  
   - 先 `import resource`（若仍走 Python 路径）再装钩子；`importlib` 加入封锁名单  
   - `exec` 用独立 `user_globals`，**不**把 `_original_import` / `_safe_import` 放进去  
   - `sys` 仅 wrapper 使用；用户 globals 不预置 `sys`
3. **测试**（现有 `test_blocked_os_import` 不够）  
   - `_original_import('os')` 失败  
   - `importlib.import_module('subprocess')` 失败  
   - `os.environ` / `os.getenv('JWT_SECRET')` / `os.getenv('DATABASE_URL')` 不可见（父进程可故意注入再断言子进程没有）  
   - Unix：试图分配超 `memory_limit_mb` 被拒或进程被杀（能稳定就写；不能稳定则只测 rlimit 已 `setrlimit` 成功的探针）

**文件**: `crates/code-interpreter/src/lib.rs`、`bridge.rs`。抽一个 `fn spawn_sandboxed_python(...)` 避免两条路径再分叉。

**验证**（需同意，约 1–2 min，会起 python3）: `cargo test -p avrag-code-interpreter --lib`。

**中期（不在本计划）**: 容器/microVM。本波结束后在审查复查清单打勾「语言钩子 ≠ 边界」。

---

## 4. W2 — Agent 出入站

依赖 W1：先没有密钥外传，再收紧「不可信内容进模型」和「模型拉内网」。

### W2-A · P1-4 `web_fetch` 切严格 SSRF

**现状**: `web_fetch.rs:239-273` 字符串前缀；`common/src/ssrf.rs` 已覆盖元数据 IP / ULA / DNS，但只给 URL 导入用。reqwest 默认跟随重定向且不复验。

**改法**:

1. 删除 `validate_url`。入口改 `common::validate_http_url_with_dns(url, true)`。  
2. 直连 reqwest：`redirect(Policy::none())`，若产品需要跟跳，则手动有限次、**每跳再跑同一校验器**（URL 导入已是 limited(5)）。本波采用 none + 不跟跳，更简单；需要跟跳再加，不预留半开自动跟随。  
3. CRW 路径：发出前同样校验目标 URL（CRW 侧内网策略仍是部署问题，代码只保证本进程不把私网 URL 递出去）。

**文件**: `agent-tools/src/skills/builtin/web_fetch.rs`（确认 `agent-tools` 已依赖 `common`）。

**验证**（约 1 min）: `cargo test -p avrag-agent-tools --lib`；单测 `169.254.169.254`、`0.0.0.0`、`fd00::1` 被拒。

### W2-B · P1-5 复活证据闸（接在现行 Lead/Worker 口，不接已死 Agent）

**现状**: `sanitize_tool_results` / `sanitize_search_results` 零调用者；`UntrustedInputProcessor::sanitize_tool_result_data` 零调用者。活的只有用户查询闸与答案输出闸。Lead/Worker 系统提示**已有**「检索正文是数据」第三人称声明——本波不改 prompt 禁令语气，不复制一段。

**改法**（一个 choke point，不双轨）:

1. 在 **host 装配 `evidence_pack_v1`** 处扫描 `EvidenceItem.content`（`lead_workers/evidence_pack.rs` / `run_lead_workers.rs` 装配函数）。有 `GuardPipeline` 则走 `check_content`；没有则走 `UntrustedInputProcessor::sanitize_retrieval`。命中则替换为现有 `[REDACTED: ...]` 占位，并记 degrade trace（已有结构）。  
2. Worker 沙箱 observation 进模型前：检索/web stdout 同样过一次（`iteration_codegen` 已有 `untrusted="true"` 包装——扫描接在包装前）。  
3. 检测器补中文高危子串（与英文列表同级，仍是启发式，不假装语义防火墙），两边（`untrusted_input.rs` + `guardrails/.../prompt_injection.rs`）同一组字面，避免只活一层。  
4. 修 `extract_evidence` 的非 char-boundary 切片（P3-16，顺手，同一文件）。  
5. 旧 `sanitize_tool_results`：若装配口不再经过 `ToolResult.data[].text`，**删除**这两段死函数，只留装配口一个实现。不保留「以备某日」的第二套。

**不做**: 新 LLM 注入分类器；不在用户主气泡加 host 脚注。

**文件**: `agent-loop/src/lead_workers/evidence_pack.rs`、`react_loop/run_lead_workers.rs`、`react_loop/iteration_codegen.rs`、`untrusted_input.rs`、`content_guard.rs`（可能整文件删）、`guardrails/src/input/prompt_injection.rs`。新 observation 文案若要注入模型，只写 `prompts/loop/*.md` 并先登记 `host_markers.rs`。

**验证**（需同意，`agent-loop` 较重，约 3–8 min）: `cargo test -p avrag-agent-loop --lib`；单测：英文 `ignore previous instructions` 与中文「忽略以上指令」在 pack content 上被 redact。

---

## 5. W3 — 检索租户：数据层 fail-closed

**目标**: 向量查询不再出现「`doc_ids=None` → 该 owner 全部 workspace」。workspace 隔离不依赖每位调用方记得传 scope。

**现状**: pgvector / Milvus 只滤 `owner_user_id`；`None` 走全 owner（`storage-pgvector/src/search.rs:50`）。产品 chat 经 `workspace_doc_scope`（`rag_execute.rs:44-51`）通常会带上 id；share 访客重映射为 owner 后，任何漏传 `doc_ids` 的路径都会串库。MCP `expand_external_workspace_rag_scope`（`transport-http/src/mcp/tools/query.rs:86`）会扩展空 scope。

**改法**:

1. 数据层：`doc_ids` 为 `None` 或空 → 与现空 vec 一样 **直接返回空**（pgvector 已对 `Some([])` 如此，把 `None` 收成同一枝）。Milvus 同样。  
2. 需要「该 workspace 全部已完成文档」的调用方必须在应用层解析出 id 列表再传入（chat 已有）。  
3. MCP expand：空 scope **不再**扩成 owner 全库；只扩当前 `workspace_id` 下已完成文档，扩不到就空结果。  
4. 单测：share 身份 + `doc_ids=None` → 0 hits；带本 workspace id → 仅这些文档。

**文件**: `storage-pgvector/src/search.rs`、`storage-milvus/src/schema.rs`（及对应 search）、`transport-http/src/mcp/tools/query.rs`、`retrieval-data-plane` 里传 `None` 的测试夹具。

**验证**（约 2–4 min）: `cargo test -p avrag-storage-pgvector --lib`、`cargo test -p avrag-storage-milvus --lib`（若需 PG/Milvus 按现有测试 skip 规则）、`cargo test -p avrag-transport-http --lib` 中 mcp query。

---

## 6. W4 — 会话与 CORS（前端 + 路由）

### W4-A · P1-7 cookie

`avrag.auth.persisted`：`SameSite=Lax`；`Secure` **仅在 `https:`**（`http://localhost` 带 Secure 则 cookie 设不上）。不在本波做 HttpOnly + 短周期轮换（要改前端读 cookie 的水合路径，单独开）。

**文件**: `frontend_next/lib/auth/server-session.ts`。  
**验证**: `pnpm test` 中 auth 相关（约 1 min）；目视 Set-Cookie。

### W4-B · P2-8 CORS

`build_cors_layer`（`router_core.rs:237-256`）：解析结果为空时 **不要** `AllowOrigin::any()`，回退到现有 localhost/tauri 默认列表。生产 `.env` 设 `CORS_ALLOWED_ORIGINS`（部署脚本/VPS `.env`，本波只改代码 + `.env.example`）。

### W4-C · P2-9 JWT（轻）

release 已无兜底。本波只把 `JWT_SECRET` 写入 `.env.example` 为必填说明；本地 `.env` 补随机值。不改 debug 回落，除非要顺便收紧：debug 也拒绝空密钥（会破一批未设 env 的单测）——默认 **不收紧 debug**，避免测试噪音。

**验证**（约 1–2 min）: `cargo test -p avrag-transport-http --lib` CORS/JWT 相关。

---

## 7. W5 — 计量 / 桌面 / 摄取（P2–P3）

**目标**: 关掉审查里仍 fail-open 或客户端可伪造的一层，不引入容器。  
**非目标**: 解析器二进制进程/容器隔离（P2-14，另开）。

### W5-A · P2-11 ClamAV

`CLAMAV_HOST` 未设或空 → 跳过扫描（未配置）。设了但连不上 / 响应异常 → `Err`，processor 默认 fail-closed（仅 `SECURITY_SCAN_FAIL_OPEN=true` 例外）。**不再默认 localhost:3310**。

**文件**: `ingestion/src/security_scanner.rs`、`.env.example`。

### W5-B · P2-11 Relay 计量

Chat 仍强制 `stream_options.include_usage`。上游无 usage：按请求正文 ~4 chars/token 估 token 并 debit。非流式且既无实际也无估计 → 502 不交付。流式已发出则记估计（无法撤回）。Embeddings / rerank 同样：无实际且估计为 0 → 502。

**文件**: `transport-http/src/routes/relay.rs`。

### W5-C · P2-10 `/metrics` 与 `x-forwarded-for`

- `TRUST_FORWARDED_FOR` 未显式开启时忽略 `x-forwarded-for` / `x-real-ip`（与 `TRUST_PROXY_AUTH` 独立；生产反代需设 true）。
- `METRICS_TOKEN` 设了则要求 `Authorization: Bearer` 或 `x-metrics-token`；生产且未设 token → `/metrics` 404；非生产未设 → 允许本地 scrape。

**文件**: `middleware.rs`、`lib_impl/infra_handlers.rs`、`.env.example`。

### W5-D · P2-12 桌面

1. Tauri CSP 对象（`script-src 'self'`，Tauri 会追加 nonce/hash）。
2. `get_client_runtime_config` 抹掉 URL 密码，保留 `pg_host`/`pg_port`。
3. 本地密码改 `Uuid::new_v4()`（CSPRNG）。
4. Windows `open_with_os` 改 `ShellExecuteW`（`open_in_browser` 已拒非 http(s)/mailto）。
5. `client.env` / `jwt.secret` / `byok.key` / `local_user.json` 写入后 Unix `0o600`；Windows owner-only DACL。

**文件**: `desktop/src-tauri/tauri.conf.json`、`commands/{local_stack,local_session,system,native_stack,secret_fs}.rs`、`Cargo.toml`。

### W5-E · P2-13 摄取隐藏 Unicode

`sanitize_string_field` 默认剥离 ZWSP/ZWNJ/ZWJ/WJ/BOM、软连字符、bidi isolate/embedding（U+202A–202E、U+2066–2069、U+200E/F），警告码 `hidden_unicode_stripped`。

**文件**: `ingestion/src/ir_validator.rs`。

### W5-F · P3-15 / 17 / 18

法律 Markdown `allowDangerousHtml: false`；ConsentCheckbox `rel="noopener noreferrer"`；客户端上传预检 100MB（与 `AVRAG_MAX_UPLOAD_FILE_SIZE_BYTES` 默认一致）。P3-16（CJK 切片）已在 W2-B 修。

**文件**: `frontend_next/lib/legal/render-markdown.ts`、`components/legal/ConsentCheckbox.tsx`、`lib/workspace/client.ts`、`hooks/workspace-context-rail/use-source-actions.ts`。

---

## 8. 延期（有意不做）

| 项 | 原因 |
|---|---|
| P2-14 外部解析二进制 | 供应链，需隔离执行环境（近 W1 中期）；本波不做假包装 |

---

## 9. 每波验证门（通过才能开下一波）

| 波 | 门 | 复查清单对应 |
|---|---|---|
| W0 | 伪造旧上传常量失败；`E2E_ENABLED=true`+`NODE_ENV=production` 代理头 401/403 | 审计 §六 条 2、3 |
| W1 | `_original_import` / `importlib` / 读 `JWT_SECRET` 均失败 | §六 条 1 |
| W2 | `client.fetch("http://169.254.169.254/")` 拒；pack 含注入中英样本被 redact | §六 条 4、5 |
| W3 | share / `doc_ids=None` 不回 owner 其他 workspace | §六 条 6 |
| W4 | Set-Cookie 含 `SameSite=Lax`；`Secure` 仅 `https:`；空 CORS 列表不 `*` | — |
| W5 | ClamAV 未配置跳过、配置失败则 Err；无 usage 的 embeddings 502；未信任的 forwarded-for 忽略；隐藏 Unicode 被剥 | §延期仅余 P2-14 |

不跑 full-149 / 真 LLM E2E。波末最多 `cargo test -p <touched> --lib`（时间见上，**先问再跑**）。结构改动后同会话 `code-review-graph update`。

---

## 10. 建议提交粒度（本地 trunk，不推）

1. `security(w0): fail-closed upload secret and proxy auth`  
2. `security(w1): isolate code-interpreter env and rlimits`  
3. `security(w2): strict web_fetch SSRF and evidence intake guard`  
4. `security(w3): require retrieval doc scope at data plane`  
5. `security(w4): secure auth cookie and cors fail-closed`  
6. `security(w5): clamav/metrics/relay fail-closed, desktop csp, ingest unicode`

每波独立可回滚。P2-14 解析器隔离不塞进同一提交。
