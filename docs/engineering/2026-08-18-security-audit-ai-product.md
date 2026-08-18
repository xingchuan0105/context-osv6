# AI 产品安全审查 — 框架与发现（2026-08-18）

> 范围：`avrag-rs` 后端、`frontend_next` 前端、`desktop`（Tauri）客户端。
> 方法：先以外部最佳实践（OWASP LLM Top 10 2025、OWASP RAG Security Cheat Sheet、Agentic 防御栈/致命三角）建立分层审查框架，再逐层对照代码实测。所有发现均经人工核验并给出 `file:line`。
> 本文是审查报告，不是修复决定。修补编排见 [`docs/plans/2026-08-18-security-remediation-plan.md`](../plans/2026-08-18-security-remediation-plan.md)（W0–W5 已落地；P2-14 解析器隔离仍延期）。

## 一、外部依据

| 来源 | 用途 |
|---|---|
| OWASP Top 10 for LLM Applications 2025（LLM01–LLM10） | 风险分类骨架：提示词注入、敏感信息泄漏、供应链、投毒、输出处理不当、过度代理、系统提示词泄漏、向量/嵌入弱点、幻觉、无限制消耗 |
| OWASP RAG Security Cheat Sheet | 管道各阶段控制：摄取哈希与对抗性扫描、租户隔离（查询时过滤而非事后过滤）、检索内容包装为数据、输出校验、fail-closed |
| Agentic AI 防御栈（Zylos 等 2026） | 七层纵深：输入过滤、沙箱执行、最小权限、HITL、工具供应链、记忆完整性、审计日志 |
| 「致命三角」（Airia 等） | 判定注入的实际危害 = 私有数据 + 不可信内容 + 外传通道三者交集 |

## 二、审查框架（L0–L6）

每层含：审查项 → OWASP 映射 → 本项目对应面（实测入口）。

| 层 | 审查项 | OWASP | 本项目对应面 |
|---|---|---|---|
| **L0 边界与认证** | 公开路由枚举；认证机制与密钥兜底；代理头信任；限流键不可伪造 | LLM10 | `routes/infra.rs`、`middleware.rs`、`router_core.rs`、`routes/relay.rs` |
| **L1 租户与数据隔离** | RLS；向量库过滤层级（owner vs workspace）；share 访客重映射后的检索边界 | LLM02/08 | `storage-pg`、`storage-pgvector/search.rs`、`storage-milvus/schema.rs`、share 中间件 |
| **L2 提示词信道** | 直接注入（输入闸）；间接注入（检索块/Web 正文/工具 stdout 的扫描、包装、声明）；检测器语言覆盖；系统提示词泄漏 | LLM01/07 | `guardrails/*`、`agent-loop/content_guard.rs`、`untrusted_input.rs`、`react_loop/*`、`prompts/` |
| **L3 工具与执行边界** | 工具面封闭性；Default-Deny 策略；沙箱隔离强度（OS 级 vs 语言级）；SSRF；过度代理 | LLM01/06 | `agent-tools/*`、`code-interpreter/*`、`web_fetch.rs`、`common/ssrf.rs` |
| **L4 输出与前端渲染** | 输出闸（PII/prompt 泄漏）；LLM 输出→HTML 的 XSS 链；链接/URL 安全；token 存储 | LLM02/05 | `guardrails/output`、`citation-renderer.tsx`、`workspace-html-sanitize.ts`、`lib/auth/*` |
| **L5 桌面客户端** | Tauri CSP/能力面；IPC 越权；本地栈凭据落盘；外部打开 | LLM05/06 | `desktop/src-tauri/*` |
| **L6 供应链与消耗** | 依赖与外部二进制；密钥管理；计量 fail-open/closed；DoS 面 | LLM03/10 | `ingestion/*`、`bins/worker`、relay 计量、上传 body 限制 |

**致命三角判定**：本项目三角三要素在 `code_interpreter` + `client.fetch` 上齐聚——bridge 可拉取 owner 私有检索数据（私有数据）、检索块/Web 正文未扫描即入上下文（不可信内容）、逃逸后可用 urllib 外传（外传通道）。这是本次审查的最高危链路。

## 三、发现（按严重度）

### P0 — 严重

**P0-1 `code_interpreter` 沙箱可一步逃逸，且子进程继承全部环境密钥**
`crates/code-interpreter/src/lib.rs:266-353`

1. wrapper 先安装 `_safe_import` 钩子**之后**才执行 `import resource`，而 `resource` 在封锁名单里 → `ImportError` 被 `except: pass` 吞掉 → **Unix 上 RLIMIT_AS / RLIMIT_CPU 从未生效**（Windows job object 有效，lib.rs:172）。
2. `_original_import` 作为全局名留在 wrapper 命名空间，用户代码经 `exec` 共享同一 globals，直接 `_original_import('os')` 即还原完整 import。
3. `importlib` 不在封锁名单，`importlib.import_module('subprocess')` 直接绕过钩子。
4. wrapper 顶部 `import sys, io, json, traceback`（lib.rs:298）发生在装钩子之前，`sys` 对用户代码可见。
5. 子进程默认继承 API 进程环境 → `JWT_SECRET`、`DATABASE_URL`、各家 `*_API_KEY`、`SMTP_PASS` 全部可读。

攻击链：间接提示词注入（用户文档/网页）→ 模型在沙箱里生成逃逸代码 → 任意代码执行（API 服务用户权限）→ 读 env 密钥 → 外传。墙钟超时击杀（lib.rs:211-224）是唯一稳定生效的限制。

**P0-2 无鉴权 `PUT /uploads/{document_id}` + 硬编码签名兜底密钥**
`crates/app-core/src/config_helpers.rs:25-30`、`routes/infra.rs:19-23`、`lib_impl/infra_handlers.rs:192-243`

- 签名密钥兜底常量 `"context-osv6-local-upload-secret"`；本地 `.env` **未设置** `AVRAG_UPLOAD_SIGNING_SECRET`。
- 知道源码常量即可对任意**已存在**的 document_id 伪造 `expires`+`signature`（文档查找走 super_admin，`app_state/e2e_upload_helpers.rs:278-319`）→ **跨用户覆盖文档字节 → RAG 投毒 → 间接注入受害者问答**。
- PG 未配置时签名校验整体跳过（infra_handlers.rs:217-222）。
- 512MB body 无鉴权路由 = 消耗型 DoS 面。
- 次要：HMAC 用 `!=` 字符串比较，非恒时（`storage_context.rs:115`）。

**P0-3 `E2E_ENABLED=true` 即开启代理头认证，且无 NODE_ENV 闸**
`crates/transport-http/src/middleware.rs:499-540`

- `proxy_auth_allowed` 只看 `E2E_ENABLED`/`TRUST_PROXY_AUTH`，**不看 `NODE_ENV`**；伪造 `x-owner-user-id` + `x-permissions` 即可成为任意用户、持任意权限。
- 本地 `.env:202` 就是 `E2E_ENABLED=true`（故本地一直开着代理认证）。VPS 据记忆当前关闭，但距任意账号接管只差一个 env 值，且同旗标会把边缘限流抬到 10,000 RPM（middleware.rs:111-115）。
- 对照：`/api/e2e/*` 路由本身有 `NODE_ENV != production` + `x-e2e-secret` + 邮箱格式三重闸（`routes/e2e.rs:51-105`）——同旗标两条路径防护不一致。

### P1 — 高

**P1-4 `web_fetch` / `client.fetch` SSRF 校验为字符串前缀级**
`crates/agent-tools/src/skills/builtin/web_fetch.rs:239-273`

- 不拦：`169.254.169.254`（云元数据）、`0.0.0.0`、IPv6 ULA/link-local（`fd00::/8`、`fe80::`）、十进制/八进制 IP 形式；不做 DNS 解析校验（rebinding）；reqwest 默认跟随重定向且不复验（web_fetch.rs:170-175）。
- 严格校验器 `crates/common/src/ssrf.rs` 已存在（覆盖以上全部场景），但唯一调用方是 URL 导入（`app-documents/src/url_fetch.rs:29`）——把 agent 路径切换到它即可，修复成本低。
- CRW 配置时实际出网方是 CRW 服务，内部网段的拦截取决于 CRW 部署，代码库内无保障。

**P1-5 检索/Web 证据进入模型上下文前的注入防护是死代码**
`crates/agent-loop/src/content_guard.rs`、`crates/agent-loop/src/untrusted_input.rs`

- `sanitize_tool_results` / `sanitize_search_results`（content_guard.rs:9-75）**全仓零调用者**；`UntrustedInputProcessor::sanitize_tool_result_data`（untrusted_input.rs:80-104）零调用者；`<ExternalEvidence>` 包装只在 Write 模式 cards 生效（`app-chat/src/writer/cards.rs:97`）。
- 已接线的只有两端：用户查询输入闸（`app-chat/src/chat/service.rs:209,251`）和答案输出闸 prompt-leak+PII（`app-chat/src/chat/service_postprocess.rs:26`）。**主 ReAct/RAG 通道中间的证据环节没有活的防线。**
- 系统提示词中没有「外部内容视为数据」的声明/分隔约定（grep 仅命中 format reference）——OWASP RAG CS 建议的 delimiters+重述系统指令缺失。
- 检测器本身是英文子串启发式（untrusted_input.rs:142-183），对中文注入零覆盖——中文产品场景的明显盲区。

**P1-6 向量检索租户边界为 owner 级而非 workspace 级**
`crates/storage-pgvector/src/search.rs:32,50,102,123`、`crates/storage-milvus/src/schema.rs:342-356`

- pgvector/Milvus 查询只按 `owner_user_id` 过滤；`doc_ids = None` 分支会跨该 owner 全部 workspace。Milvus 的 `workspace_id` 字段存在但从不作为过滤条件。
- workspace 隔离完全依赖调用方（`ChatContext::workspace_doc_scope`，`app-chat/src/rag_execute.rs:44-51`）。share 访客被重映射为 owner 身份（middleware.rs:257-268）后，任何以 `None` doc_ids 调用检索的路径都会让访客触达 owner 其他 workspace 的内容。
- MCP 兼容路径 `expand_external_workspace_rag_scope`（`mcp/tools/query.rs:86-115`）自动扩展空 scope，属同类需逐路核实的面。

**P1-7 前端 JWT 双存 localStorage 与非 HttpOnly/非 Secure cookie**
`frontend_next/lib/auth/context.tsx:28,56`、`frontend_next/lib/auth/server-session.ts:17-44`

- `avrag.auth.persisted` cookie：JS 可读、无 `Secure`、`Max-Age=1y`；无 refresh/轮换。XSS 一旦成立即长期账户接管。
- 缓解项：主渲染链路有 DOMPurify 白名单（见「良好控制」），当前无已知 XSS 注入点。

### P2 — 中

| # | 发现 | 位置 |
|---|---|---|
| P2-8 | CORS 配置解析失败回退 `AllowOrigin::any()` + `AllowHeaders::any()`（fail-open 解析）；`.env` 未设 `CORS_ALLOWED_ORIGINS` | `router_core.rs:247-255` |
| P2-9 | JWT debug 兜底密钥 `"change-me-in-production"`；`.env` 未设 `JWT_SECRET`，debug 构建公网暴露即接管（release 会 panic，正确） | `router_core.rs:44,75-81` |
| P2-10 | `x-forwarded-for` 直取作限流键（可伪造转嫁）；`/metrics` 无鉴权暴露内部指标 | `middleware.rs:72-85`、`routes/infra.rs:13` |
| P2-11 | ClamAV 不可达时 fail-open；relay 用量计量 fail-open | `ingestion/src/security_scanner.rs:54-66`、`routes/relay.rs:548` |
| P2-12 | Desktop 一组：Tauri `csp: null`；IPC `get_client_runtime_config` 向 webview 返回带密码的 DB/Redis URL；local 账户密码用非 CSPRNG 哈希；Windows `cmd /C start "" <url>` 参数注入面；云 relay token 明文落 `client.env`；`byok.key` Windows 无权限限制 | `tauri.conf.json:26-28`、`commands/local_stack.rs:225-286`、`commands/local_session.rs:74-88`、`commands/system.rs:28-39`、`commands/native_stack.rs:384-475` |
| P2-13 | 摄取侧无对抗性内容预扫描（零宽字符、隐形 Unicode、隐藏指令标记）——OWASP RAG CS §1 建议项 | `bins/worker/src/pipeline/` |
| P2-14 | 上传解析依赖同主机外部二进制（LITEPARSE `lit`、`anydoc-extract`）直接处理不可信字节 | `ingestion/src/parser/liteparse_pdf.rs:42`、`parser/anydoc.rs:31` |

### P3 — 低

| # | 发现 | 位置 |
|---|---|---|
| P3-15 | legal 渲染管线 `allowDangerousHtml: true` + 无 DOMPurify（当前输入仅为仓内文件） | `lib/legal/render-markdown.ts:28,36` |
| P3-16 | `extract_evidence` 的 `&trimmed[..max_chars]` 非 char-boundary 切片，多字节文本可 panic（当前无调用者） | `untrusted_input.rs:117` |
| P3-17 | `ConsentCheckbox` `target="_blank"` 无 rel（现代浏览器隐含 noopener） | `components/legal/ConsentCheckbox.tsx:39,43` |
| P3-18 | 上传无客户端大小预检；MIME 由客户端扩展名自报 | `lib/workspace/client.ts:302-358` |

## 四、良好控制（应保持）

- **BYOK**：AES-256-GCM + nonce、RLS 强制（migration 0067）、API 只回指纹不回明文、Debug 脱敏（`app-core/src/byok_crypto.rs`、`billing/src/provider_secrets.rs:36-88`）。
- **工具治理**：native LLM 工具面封闭（`tool_registry.rs:76-89`）、Default-Deny `PolicyEnforcer`（`capability/policy.rs:63-120`）、`write_refine_*` 只经专门池。
- **前端渲染**：DOMPurify 白名单 + `ALLOW_DATA_ATTR:false`（`workspace-html-sanitize.ts:3-48`）；`isSafeHttpUrl` 拦 `javascript:/data:/blob:/file:`（`lib/url/isSafeHttpUrl.ts`）；外链普遍带 `noopener noreferrer`；全仓无 postMessage 监听、无 iframe。
- **URL 导入**走严格 SSRF 校验器（`common/ssrf.rs` 经 `url_fetch.rs:29`，limited(5) 重定向）。
- **输出闸**已接线：prompt-leak 指纹（含启动时扫描 prompts 目录的动态指纹）+ PII scrub（`guardrails`、`service_postprocess.rs:26`）。
- **E2E 路由**三重闸（NODE_ENV + secret + 邮箱格式）。
- **ZIP 炸弹检测 fail-closed**（`security_scanner.rs:22-39`）。
- **RLS 逐事务**设置 `app.current_user`（`storage-pg/src/lib_impl/core.rs:36-58`）。

## 五、修复优先级建议（待拍板，未实施）

1. **P0-1 沙箱**：最低成本的正确修是「进程级隔离 + 最小环境」——spawn 时清空环境（只传必要路径变量）、wrapper 内先设 rlimit 再装钩子、`exec` 前清空 `__builtins__` 之外的 wrapper 全局名、封锁 `importlib`；中期换 OS 级隔离（容器/microVM），当前语言级钩子不应视为安全边界。
2. **P0-2 上传**：删除硬编码兜底密钥，未显式配置时 release panic / 全环境 fail-closed；签名比较改恒时；考虑对无 PG 模式直接关闭该路由。
3. **P0-3 代理认证**：`proxy_auth_allowed` 增加 `NODE_ENV != production` 硬闸，并将 `E2E_ENABLED` 与代理认证解耦（它只应影响 e2e 路由与限额）。
4. **P1-4**：`web_fetch` 切换到 `common::validate_http_url_with_dns`，重定向改 `Policy::none` 手动逐跳复验。
5. **P1-5**：把 `content_guard::sanitize_tool_results` 接进 evidence intake（一行接线即可让死代码复活）；补中文注入模式；在 `prompts/system/` 增加外部内容视为数据的第三人称观察声明。
6. **P1-6**：检索层断言 `doc_ids` 必须非空（或查询生成时强制带 workspace 过滤），share 路径逐一核实。
7. **P1-7**：cookie 至少加 `Secure`；规划短期 token + 轮换。

## 六、复查清单（修复后回归）

- [ ] 沙箱逃逸回归：`_original_import` / `importlib.import_module('os')` / 读 env 全部失败
- [ ] 伪造上传签名（默认密钥路径）失败
- [ ] `E2E_ENABLED=true` + `NODE_ENV=production` 下 `x-owner-user-id` 被拒
- [ ] `client.fetch("http://169.254.169.254/")` 被拒；重定向到内网被拒
- [ ] 检索块含注入文本时被 redact（含中文样本）
- [ ] share 访客检索不触达 owner 其他 workspace chunk
