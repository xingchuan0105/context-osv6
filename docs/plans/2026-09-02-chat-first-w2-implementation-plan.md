# Chat-first W2 实施编排（Session 文件闭环：Artifact/Binding、会话级 RAG、Snapshot/Evidence、完整删除）

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-02 |
| 状态 | Plan；未开工。W0–W1 已实现（未提交），本计划接管其后的下一阶段 |
| 前置交接 | [`2026-09-02-chat-first-w0-w1-handoff.md`](2026-09-02-chat-first-w0-w1-handoff.md) |
| 设计真相 | [`2026-09-02-chat-first-conversation-workspace-design.md`](2026-09-02-chat-first-conversation-workspace-design.md)（§4.3、§7、§10.2–10.4、§11、§13.2/13.5） |
| 分支基线 | `master` @ `e3af8c09`；工作树含 W0–W1 未提交改动，禁止 reset/checkout/批量格式化 |
| 下一层 | W3（quick_chat BYOK + 逐调用计费），不在本计划内 |

## 0. 一句话目标

**让 `/chat` 会话拥有自己的文件：上传即解析、ready 后只在该 Conversation 的服务端授权范围内参与 RAG；每轮冻结 Snapshot、落真实 Evidence；删除 binding 即退出未来检索、零 binding 触发既有异步完整清理；历史引用退化为最小墓碑。W2 完成后 P0 仅剩 W3。**

## 1. W2 总验收门（全部满足才算完）

1. 同 owner Session 文件可上传并立即解析；`uploading → parsing → ready | failed` 每文件独立状态、重试、移除（设计 §13.2）。
2. 显式附加但未 ready 的文件阻止发送，不被静默忽略。
3. ready 文件只对当前 Conversation 可见；服务端从 binding 重算 allowed artifact IDs，客户端 doc ID 只做子集选择、不能扩权（§11.2）。
4. 回答实际引用区分 Session 文件 / Workspace 资料 / Web；「可用范围」与「实际引用」分开展示（§13.5）。
5. 删除 Session binding 后不再参与未来检索；双 binding 时删一方不误删 Artifact；零 binding 触发完整异步清理（raw、assets、index、chunks/blocks/parse runs、struct sidecar），部分失败保持可重试。
6. 来源删除后引用显示墓碑状态；历史回答文字与历史轮的范围说明不漂移。
7. P0 无自动 raw TTL；文件/Web 不改变持久化 `model_role`。

## 2. 代码勘察结论（2026-09-02，计划的落地依据）

| 事实 | 含义 |
|---|---|
| `documents` 表（0001，0055 改名出 `workspace_id`，0056 改 `owner_user_id`）目前同时承担 Artifact 身份与 Workspace scope；状态枚举在代码侧（`contracts/src/documents.rs` + `app-documents/src/helpers.rs`：`pending/enqueueing/queued/processing/completed/failed/deleting/deleted/upload_invalid`），无 `parse_state` 列 | W2a 的"Artifact 身份"不必新建表：`documents` 即 artifact，拆的是 scope 耦合 |
| 全仓无任何 binding 表；唯一 scope 链接就是 `documents.workspace_id` | 迁移从 0086 起号，双表 typed binding 全新落地 |
| 派生表现状：`chunks`(0001, CASCADE)、`document_assets`(0014, **无 FK**)、`document_multimodal_chunks`(0015)、`document_parse_runs`(0021, 仍带 workspace 语义列)、`document_blocks`(0022, 同上)、`document_cleanup_tasks`(0027)、`document_toc`(0031)、pgvector 五表(0060, `workspace_id` 为软引用无 FK) | 派生表的 scope 列是冗余真相，授权必须改为只认 binding |
| 旧真相读写路径：`storage-pg/src/documents.rs:29`（list 按 `workspace_id=$1`）、`storage-pg/src/lib_impl/repository_retrieval_lifecycle.rs`（update 含 workspace move、delete、状态机）、`app-documents/src/documents.rs:455-470`（move）、`:482`（delete）、port `app-core/src/document_store.rs`、memory adapter、`app-bootstrap/src/adapters/pg_document_store.rs` | W2a 的删除清单；须逐一改为 binding 视角或删除 |
| 上传管线：`POST /workspaces/{id}/documents`(presign) → PUT 对象存储 → `POST /documents/{id}/complete-upload`(校验+写 `upload_*`+入队 `ingestion_tasks`) → worker `bins/worker` 轮询解析/索引/物化 | W2b 完全复用；session 文件只是"artifact + 新 binding"，**不建第二条解析管线** |
| 完整删除机制已存在：`delete_document` 软删 + `document_cleanup_tasks`(claim/lock/renew/complete/fail/dead-letter 全套) → worker `run_document_cleanup_once`（raw→assets→`delete_document_index`→derived rows→struct store→mark deleted） | W2e 零 binding GC 直接入队复用，**不新造 outbox** |
| chat 授权现状：`ChatRequest.doc_scope` 客户端提供（`contracts/src/chat.rs:19-56`）；`validate_rag_doc_scope`（`app-chat/src/context.rs:183` → `app-documents/src/document_context.rs:28-80`）只验存在+Completed+workspace 匹配，**在 owner 范围内信任客户端清单**；RuntimeExecute 路径已是服务端重算（`app-chat/src/rag_execute.rs` → `completed_workspace_doc_ids`）；`agent-loop/src/react_loop/run_lead_workers.rs:987-1014` 把 doc_scope 注入 tool args；强制点在 `rag-core/src/runtime/scoped_rag_dispatch.rs`（`dispatch_scoped/force_doc_scope/intersect_doc_scope`，fail-closed）与 SQL `document_id = any(...)`（`storage-pg/src/lib_impl/repository_retrieval.rs`） | W2c 只需把 chat 管线路径切到 binding 派生 allowed 集；拦截点全部现成 |
| `chat_messages.turn_metadata` JSONB（0039）已存在，`storage-pg/src/lib_impl/repository_sessions.rs:206-258` 按 user/assistant 行分别写入 | Snapshot/Evidence 落点现成（见 §5 决策 D2） |
| 迁移尾部 0085；HTTP 会话更新当前为 `PUT`（设计 §10.1 定稿是 `PATCH`） | 新迁移 0086/0087；方法名偏差在 W2b 一次性统一为 PATCH |
| 前端：ChatComposer（`components/workspace/chat-composer.tsx`）无上传 UI；上传在 Workspace 右栏 sources pane（`workspace-sources-pane.tsx` → `use-source-actions.ts:107-140`）；`selectedSourceIds → doc_scope`（`hooks/chat-session/use-chat-stream.ts:253`） | W2b 的会话文件托盘是 ChatCanvas/Composer 新增件；Workspace sources pane 是另一 scope，不混用 |

## 3. 切片编排与门

```text
W2a schema → W2b API+UI → W2c ContextScope/RAG → W2d Snapshot/Evidence → W2e 删除/墓碑
   门A           门B             门C                门D                  门E
```

线性推进；任一门失败停在该片修复，不叠下一层。每片开工前 `code-review-graph` 查影响半径；改动落完后同 session `code-review-graph update`。

### W2a — Artifact + typed Binding schema（迁移 0086/0087）

**改动面**

1. 盘点 `documents.workspace_id` 全部读写方（勘察 §2 已列主干；补充审计 `document_parse_runs`/`document_blocks`/`document_assets` 的 workspace 语义列、`rag_*` 五表的 `workspace_id` 软引用、`update_document` 的 workspace move 在 UI/HTTP 层是否有产品入口）。
2. 迁移 **0086**：建
   - `conversation_document_bindings(id, artifact_id → documents, conversation_id → chat_sessions, owner_user_id, created_at)`，`UNIQUE(conversation_id, artifact_id)`，`ON DELETE CASCADE` 随会话；
   - `workspace_document_bindings(id, artifact_id → documents, workspace_id → workspaces, owner_user_id, created_at)`，`UNIQUE(workspace_id, artifact_id)`，`ON DELETE CASCADE` 随工作区；
   - 两表 `FORCE ROW LEVEL SECURITY` 按 `owner_user_id`（沿用 0082/0056 的 policy 形态）；
   - 回填：`documents.workspace_id` 非空且未删除的行 → workspace binding；回填后行数等值校验。
3. 代码切读：workspace documents 列表、`get_document_scope_states`、`completed_workspace_doc_ids` 等 改为 binding JOIN 派生；**授权从此只认 binding**。
4. 迁移 **0087**：删除 `documents.workspace_id` 列与派生表冗余 scope 写入（ rag_* 软引用列处置见 §5 D4），删除旧读写路径（上表清单），不留双读双写。
5. memory adapter（`app-core/src/adapters/memory.rs` / `memory_chat_persistence.rs`）同语义实现，测试双轨对齐。

**关键决策**：`documents` 表保留原名作为 Artifact 物理身份（见 §5 D1）；`update_document` 的"移动 workspace"产品语义到 W4 才有（binding 的增删），W2a 先把该路径改为 binding 操作或按盘点结果删除。

**门 A（验证）**：`cargo test -p contracts`；`cargo test -p avrag-storage-pg --lib`（tenant isolation + 新增 binding 唯一/级联/回填等值/双 binding/零 binding 判定用例，需 source `.env`）；`cargo test -p app-core --lib -- --test-threads=1`；`cargo test -p app-documents`（如存在既有测试面）。新增 owner 越权、同 artifact 双 binding、binding 级联三组反例必测。

**停止条件**：回填等值不等、RLS 用例失败、或旧路径删除后出现编译期残留引用 → 停在 W2a。

### W2b — Session 文件 API 与 UI 状态

**改动面**

1. HTTP（`transport-http/src/routes/` + `handlers/documents.rs` 复用）：
   - `POST /api/v1/chat/sessions/:id/files` — owner 校验后同事务创建 artifact（`documents` 行，无 workspace）+ conversation binding，返回 presign（复用 `create_document_upload` 机制）；
   - `POST /documents/:id/complete-upload` 保持 artifact 级复用（入队 ingestion），不建会话副本端点；
   - `GET /api/v1/chat/sessions/:id/files` — binding JOIN documents + 解析状态；
   - `DELETE /api/v1/chat/sessions/:id/files/:bindingId` — W2b 先做"删 binding + 触发 W2e 清理判定"的领域入口（清理本体 W2e 接）。
2. **幂等建会话**：`/chat` 无 session 时首次上传先 `POST /chat/sessions` 再传文件（设计 §7.1）；前端 ChatCanvas 持有该编排。
3. 前端：ChatCanvas/Composer 新增**会话文件托盘**（上传按钮、每文件 `uploading/parsing/ready/failed` 状态、重试、移除）；显式附加未 ready → 阻断发送并可移除/排除；数据源为 GET files 轮询或复用 status 端点。托盘不进 Workspace context rail，两 scope 不混。
4. **PUT→PATCH 一次性统一**：router/client/测试同波切到 `PATCH /api/v1/chat/sessions/:id`（定稿 §10.1），删 PUT 路径，重新 `pnpm generate:contracts`。

**门 B（验证）**：transport-http 定向契约测试（新路由 owner 越权 404、binding 归属、幂等）；`pnpm typecheck` + `pnpm test`；`pnpm generate:contracts` 通过。

**停止条件**：上传-解析端到端在 staging 管线未走通（文件卡在非 ready）→ 停在 W2b。

### W2c — 服务端 ContextScope 与 RAG

**改动面**

1. `execute_chat_pipeline`（`app-chat/src/chat/service.rs`）：`resolve_request_workspace` 之后，从持久化 session 解出 owner、optional workspace、`model_role`，计算
   `allowed = ready(conversation_document_bindings ∩ Completed) ∪ (workspace_id? ready(workspace_document_bindings ∩ Completed))`。
2. 客户端 `doc_scope` 与 `allowed` 求交集后才是本轮检索范围；`validate_rag_doc_scope`（`app-chat/src/context.rs:183`）的真相源从 `documents.workspace_id` 改为 binding 集合，越权 ID 表现 not found/invalid。
3. `rag_execute.rs` 的 `workspace_doc_scope()` 同步切 binding 派生；`dispatch_scoped/intersect_doc_scope` 拦截点不变（fail-closed 语义保留）。
4. 同一 artifact 双 scope：范围集按 `artifact_id` 去重（版本去重进 W2d Evidence 记录 scope）。文件、Web 组合均不触碰 `session.model_role`；Lead+Workers 边界不动（session 文件走 RAG Worker，Web 走 Web Worker leaf）。

**门 C（验证）**：`cargo test -p app-chat --lib`；新增用例：跨会话 artifact ID 拒绝、workspace 资料 enabled 集合正确、空 allowed + rag → 结构性错误不退化、quick_chat 会话带文件后 role 不变。

**停止条件**：出现任何"客户端 ID 扩权"路径或 role 重算 → 停在 W2c。

### W2d — Snapshot 与 Evidence

**改动面**

1. **Snapshot**（发送时冻结，随 user 行写入）：`turn_metadata.context_snapshot = { workspace_id_at_send?, conversation_history_boundary, session_binding_versions[], workspace_binding_versions[], web_enabled, thinking_enabled?, model_role, effective_provider, effective_model, credential_source }`。
2. **Evidence**（执行后，随 assistant 行一次写入）：`turn_metadata.turn_evidence = { segments: [{ channel: rag|web|base_tool, source_scope, artifact_id?, parse_version?, chunk_id?/asset_id?/page?, url?, fetched_at?, citation_status }] }`。
3. 写入路径：`repository_sessions.rs` 的 turn_metadata 结构体扩展（serde typed，不散 JSON）；每行一次写入、不可变，不满足"反复覆盖 JSON"禁令。
4. SSE 完成事件与 citation 响应带 `source_scope` / 版本 / URL 结构化字段（设计 §10.4）；前端消息列表增加「本对话 · 会话文件 N · 工作区资料 N · 网页 N」可用范围条与按 scope 分组的实际引用；历史消息只读 metadata，不从 Composer 反推。
5. 双 scope 去重在 Evidence 保留"来自哪些可见 scope"。

**门 D（验证）**：`cargo test -p app-chat --lib` + storage-pg sessions 读写往返用例（snapshot/evidence 序列化保真、旧消息缺字段容忍）；前端 Vitest 范围条/引用分组用例。

**停止条件**：历史消息范围说明可被后续操作改变 → 停在 W2d。

### W2e — 完整删除与墓碑

**改动面**

1. `DELETE .../files/:bindingId`：先撤销未来可见（binding 行删除即时生效，检索集来自 binding 故天然生效）；artifact 仍有另一类 binding → 保留全部派生物。
2. 零 binding 判定（两表 COUNT）→ 入队既有 `document_cleanup_tasks`（完整清理 raw/assets/index/chunks/blocks/parse runs/struct sidecar，worker 既有 claim/retry/dead-letter 兜底）；HTTP 请求内不做多存储同步删除。
3. 孤儿 GC 触点审计：**删除 Conversation**（session bindings CASCADE 后）与**删除 Workspace**（workspace bindings CASCADE 后）两条路径必须补"级联后零 binding artifact 入队清理"，否则泄漏。
4. `parse_failed` 保留原件供重试；只有显式删除 binding 才走清理。
5. 墓碑：引用渲染在 artifact 已删除时降级为最小事实（文件名/页码/URL/时间），不保留 excerpt/chunk/asset 副本；Snapshot/CitationRecord 不计入 binding 引用数。

**门 E（验证）**：storage-pg 用例（删 binding 后 allowed 集不含、双 binding 保留、零 binding 入队恰一次）；worker cleanup 定向复跑；app-chat 墓碑渲染用例。

**停止条件**：清理把仍有 binding 的 artifact 删除、或部分失败被伪装成已删除 → 停在 W2e。

## 4. 实现级决策（推荐值，开工前可推翻）

| # | 决策 | 推荐 | 理由 |
|---|---|---|---|
| D1 | Artifact 物理表 | **保留 `documents` 表名**，Rust 领域语义按 DocumentArtifact/Binding 命名新增，不整表 rename | rename 波及 chunks/cleanup tasks/parse runs/worker 全链，无功能收益；拆 scope 耦合才是设计要求 |
| D2 | Snapshot/Evidence 落点 | **扩展 `chat_messages.turn_metadata` 内 typed serde 字段**（user 行 snapshot / assistant 行 evidence，各一次写入） | 设计 §11.1 明示允许"受约束的 turn metadata"；列已存在，避免新表+查询面 |
| D3 | 会话更新方法 | **统一为 PATCH**，同波删除 PUT | 定稿 §10.1 真相；交接已声明不并存两法 |
| D4 | 派生/index 表 scope 列 | **授权永不读取**冗余 scope 列。实施修正（2026-09-02 实际执行）：`documents.workspace_id` 物理删除（唯一真相违规）；`document_blocks/parse_runs/assets/toc` 与 `ingestion_tasks/document_cleanup_tasks` 的 workspace 列改**可空 lineage**（列保留、去掉 `NOT NULL`；两张队列表另摘除 `FK ON DELETE CASCADE`——否则删工作区会连带删掉会话绑定幸存 artifact 的清理/解析任务，正好破坏 W2e）；rag_* 五表本就可空、代码停写（传 `None`） | 勘察发现队列表/TOC 的 NOT NULL FK 会话文件根本插不进去；可空 lineage 比物理删列的 SQL 手术面小得多，且保留 lineage 信息 |
| D5 | 迁移步数 | **0086 建+回填，0087 删旧列/旧路径**，两迁移同波交付 | 0086→0087 之间代码只读 binding，无双读双写窗口 |
| D6 | 文件列表刷新 | GET files 轮询（复用既有状态端点节奏），不新增推送通道 | 最简可工作；SSE 已被流式占用 |

## 5. W2 明确不做

- W3 全部：`ProviderSecretPurpose::QuickChat`、设置 UI、fail-closed resolver、UsageSegment 逐调用计费。
- W4/P1 全部：move conversation、`add-to-workspace` binding 端点、Workspace「本会话文件」区。
- raw purge / TTL / 冷热分层（设计 §7.4 明确出 P0）。
- 新文件格式、新解析器、`chat_attachments` 平行存储、把原文拼 prompt。
- Session 文件跨会话复用、多 Workspace 联合检索、普通会话分享。
- 任何 host 语义 completeness 否决、用户主气泡脚注；session 文件不新增 prompt 注入旁路（若确需模型可见说明，落 `avrag-rs/prompts/**` 第三人称 observation + `host_markers.rs` 先注册）。

## 6. 执行纪律

- **估时同意**：每片验证门前给出命令与预估时长（历史参照：`pnpm test` ≈ 分钟级、`storage-pg --lib` 定向 ≈ 分钟级、`app-chat --lib` ≈ 分钟级），获用户同意后运行；`jobs=2`，app-core 用 `--test-threads=1`。
- contracts 变更后必须 `pnpm generate:contracts`，不手改生成 TS。
- 结构改动后同 session `code-review-graph update`；不提交 `.code-review-graph/`。
- dirty trunk：编辑前 `git status --short` + `git diff -- <path>` 确认；不 reset/checkout/批量格式化。
- 部署不做；commit 随片推进（solo trunk 本地提交），W2 全绿后波尾跑 `scripts/test-l1.sh`（需另行估时同意）。

## 7. 遗留偏差登记（不阻塞 W2，但须有去向）

1. §4 D3 的 PATCH 统一（W2b 内消化）。
2. 真实 `qwen3.8-flash` 对话 + Web 联合路径未跑（W3 上线门前必过，设计 §8.5）。
3. `document_parse_runs.artifact_path` 指向 raw object 的 provenance 问题——只在将来 raw purge 层处理（P0 保留 raw，不受影响）。
4. `rag-core` 两个集成测试（`citation_id_stability`、`vgrag_pgvector_g2`）与 worker `parse_triplet_response_*` 五测在 master 上即已破损/失败（文件与相关结构体均不在本轮改动内），按「不顺手修」纪律登记，另行处理。W2e 契约扩展（`Citation.source_scope/citation_status`）使这两个本已无法编译的文件各多出几个缺字段报错——同一破损文件，未扩散到任何可编译面。
5. Snapshot 的 `effective_provider / effective_model / credential_source` 与 Snapshot `conversation_history_boundary` 细化：W3 BYOK/逐调用计费落地时补齐。
6. 真实 worker 管线下「上传→解析→ready→参与 RAG→删除→墓碑」staging 端到端：波尾验证（需估时同意）。

## 9. W3 执行记录（2026-09-02，完成）

### W3a — quick_chat purpose + 存储约束
- 迁移 `0088_quick_chat_provider_secret`：`user_provider_secrets.purpose` CHECK 扩展 `quick_chat`（动态定位旧约束替换）；新增偏唯一索引 `(owner_user_id) WHERE purpose='quick_chat' AND revoked_at IS NULL`——每账户至多一个活动 Quick Chat 配置，解析永不依赖行序（设计 §8.2）。
- `ProviderSecretPurpose::QuickChat`（`as_str: "quick_chat"`；parse 接受 `quick_chat|quickchat`），upsert/list/resolve 全链随枚举生效。

### W3b — fail-closed resolver + 设置 UI
- `UnifiedAgent::resolve_quick_chat_credential` 三态：活动且完整 → `Byok(secret)`（主模型走 BYOK，chat 用量豁免钱包）；活动但不完整（`to_llm_config()` 缺 base_url/model/key）→ **fail-closed 报 `quick_chat_byok_config_invalid`，绝不静默回官方扣钱包**；secret store 解析失败 → fail-closed 报错（保守保护钱包）；无配置 → 官方 `QUICK_CHAT_LLM_*` 路由。
- 客户端绑定复用 `bind_byok_client`：官方 quick_chat 客户端存在则 overlay 凭据，不存在则从 secret 单路由构建——DirectAnswer / Lead / synthesis 三路主模型同凭据。
- 设置页 `settings-providers-panel` 新增 Quick Chat 行（百炼 compatible-mode 预设 · qwen3.8-flash，可改 base_url/model），行文案即 §8.4 要求的费用边界披露：「仅替代主回答；解析、检索子代理、向量/重排与联网仍按平台计费」。settings client 的 purpose 联合类型同步扩展。
- 协议边界登记：BYOK 客户端构建当前全平台统一 OpenAI 兼容单路由（`ResolvedProviderSecret::to_llm_config`），qwen3.8-flash 官方端点即 compatible-mode，不受限；多协议 profile 选择是平台既有上限，登记至 §7 去向。

### W3c — 逐调用凭据归因
- 迁移 `0089_llm_usage_credential_source`：`llm_usage_events.credential_source TEXT NOT NULL DEFAULT 'official'`——每次模型调用落一条带 `official | byok` 的用量段；feature/stage 继续承载 component role。
- `TenantContext.credential_source` + `MeteringContext.credential_source` 贯通 PG insert；绑定点标注：unified 主模型 BYOK→`byok`、retrieve Worker 重置回 `official`（检索恒走平台 RETRIEVE_LLM）、writer BYOK→`byok`、worker/bootstrap 默认 `official`。
- 钱包豁免本就按请求级 `usage_kind == "chat"` 生效（retrieve/embedding 不豁免）——「整轮 BYOK 跳账」在事实层面不存在，现在每段用量还带凭据归因。

### W3 验证门（已通过）
avrag-billing 63、app-core 41（单线程）、app-chat 88、contracts 全套（15+12+2+6）、api_key_security 16、storage-pg 37、前端 typecheck + 529 passed/2 skipped；`cargo check --workspace --tests` **首次全绿**（顺带修复了 master 上已三代漂移的 `citation_id_stability` / `vgrag_pgvector_g2` 两个测试文件——删除已移除字段、补齐新字段，1+3 用例全过）；`git diff --check` 干净；`code-review-graph update` 完成（720 files）。
- 附带修复（门驱动）：`unified` retrieve tenant 重置时同步重置 `credential_source=official`。

### W3 尚未验证（去向）
- 真实 qwen3.8-flash BYOK 调用契约（§8.5 上线前配置门：价格表、协议/thinking/流式/缓存校验）——staging 波尾验证，与 W2 上传链路端到端合并执行（需估时同意）。

## 10a. P0 真实管线端到端（2026-09-03 凌晨，通过）

本地 dev 栈（`product-dev-up.sh`：API :8080 + worker + MinIO + pgvector PG；`AVRAG_MIGRATION_ROLE_ONLY=true` 入 `.env` 使 role guard 以 migration-role 语境通过——本地 dev 属该语境，`.env.example` 已有注释说明）。旅程脚本（/tmp/p0-journey.sh，HTTP 直驱 127.0.0.1:8080 + psql 断言）：

| 环节 | 结果 |
|---|---|
| 注册 → 个人会话（`model_role=quick_chat`） | ✅ |
| 会话文件 presign → PUT MinIO → complete-upload → worker 解析（14 轮询 ≈28s）→ ready | ✅ |
| 真实 qwen3.8-flash RAG 轮（官方凭据）：答案正常、`citations[0].source_scope=session` | ✅ |
| BYOK 接线：保存 quick_chat secret（dashscope compatible-mode）→ 第二轮走 BYOK、`llm_usage_events.credential_source=byok` | ✅ |
| revoke 自己的 secret → 第三轮自动回官方、`credential_source=official` | ✅ |
| 删除 binding → files 即空 → artifact `deleting` → cleanup task 恰 1 条 → worker 全量清理至 `deleted`（4 轮询 ≈8s） | ✅ |
| 墓碑：3 条引用 `citation_status=source_deleted`、content/preview 残留 0 | ✅ |

过程中抓到并修复一个真实缺陷（commit `ba51a984`）：0089 的 `llm_usage_events` INSERT 绑定位次与列序错位——prompt_tokens 收到 model 文本，逐调用用量段全部写入失败（cost_events 轮级聚合掩盖了它）。修复后逐调用 credential 归因实证如上。

尚未跑：L3-thin-llm / DR2 全量四模式回归套件、Playwright journey（需要时另行估时）。dev 栈 tmux session `context-os-dev` 保持运行（frontend :3000 / api :8080）。

## 10. L1 波门记录（2026-09-02，通过）

`bash scripts/test-l1.sh agent-tools agent-loop app-chat transport-http avrag-storage-pg` → **L1 OK**（文件尺寸门 + 五 crate lib + 前端 tsc；storage-pg 用例静默复用 `.env` 连本地 PG）。门驱动的三项附带修复（均非本波引入，但挡门）：

1. **`llm/src/embedding.rs` 1251 行超 1000 硬限**（master 既有超标，L1 久未跑）：拆为 `embedding/mod.rs`（684 行）+ `embedding/tests.rs`（570 行，纯移动）；133 测试全过；尺寸门 allowlist 路径同步更新（该清单注释即要求模块移动时更新）。
2. **transport-http `password_reset_code_flow` 用例 500**（环境敏感，W0–W1 交接只跑过 api_key 契约未跑全 lib，故此前未暴露）：dev `.env` 的 `SMTP_*` 泄入测试进程使 `smtp_ready()` 为真，重置流程尝试真实外发并连接失败。修复在正确的层——`tests/support.rs::pg_test_app_state` 置 `EMAIL_PROVIDER=debug` 使测试密闭（曾先试 `cfg!(test)` 门，但该宏在依赖 crate 中恒 false，已回退）。
3. 此前波次登记的 rag-core 两个漂移测试文件已在本波修复，不再挡 `--tests` 全检。

## 8. 执行记录

### W2a（2026-09-02，完成）

- 迁移：`0086_document_typed_bindings`（两 binding 表 + UNIQUE + FK CASCADE + FORCE RLS + 一次性回填）、`0087_document_scope_decoupling`（删 `documents.workspace_id`；派生/队列表 workspace 列 DROP NOT NULL；队列表与 `document_toc` 摘除 workspace FK CASCADE）。
- 代码切读：workspace 列表/搜索/sources/共享计数/发布计数全部改 binding JOIN 或 EXISTS；`get_document_task_seed` lateral 取 earliest binding（Option）；`create_document`/`upsert_published_document` 同事务写 binding；`update_document` 移除 workspace 参数（move 路径删除）；cleanup targets/derived 删除改为 owner+document 键；worker 全管线 `workspace_id: Option<Uuid>` 贯通，rag_* 索引停写 workspace，对象键会话段定 `_session`；memory adapter 增加 `workspace_document_bindings` / `conversation_document_bindings` 两张绑定真值表。
- 契约：`Document.workspace_id → Option`（omit-None）；`UpdateDocumentRequest` 移除 `workspace_id`（move 语义删除）；TS 已再生。
- 门 A（已通过）：workspace `cargo check`（lib+tests，除既有 rag-core 测试漂移）零错误；contracts 15、common 18、ingestion 106、app-core 41（单线程）、app-documents 9、storage-pg 34（含新增 5 用例：binding 写入与列表、唯一约束、工作区级联后 artifact 幸存、会话+工作区双 binding 存续、跨 owner RLS 不可见）、app delegate_contract 15、api_key_security_contract 16、app-chat 87；前端 typecheck + 529 passed/2 skipped；`git diff --check` 干净；`code-review-graph update` 完成（608 files）。
- 未提交：工作树同时承载 W0–W1 与更早未提交改动，按 W0–W1 交接同一模式保留在树中，由用户决定整体提交点。

### W2b（2026-09-02，完成）

- 新契约：`SessionFileRow` / `SessionFilesResponse`（typeshare → TS 已再生）。
- 新路由（`authorize_session_access` 守卫，owner-scoped）：
  - `POST /api/v1/chat/sessions/:id/files`（artifact + conversation binding 同事务创建 + presign）；
  - `GET /api/v1/chat/sessions/:id/files`（binding JOIN documents，上传序）；
  - `DELETE /api/v1/chat/sessions/:id/files/:bindingId`（撤销未来可见性；零 binding GC 留待 W2e）。
- 解析复用 artifact 级 `POST /documents/:id/complete-upload` 与 `POST /documents/:id/reindex`（重试）；上传准入抽取为 `app_documents::ensure_document_upload_allowed` 供工作区/会话两路共用。
- Port 新增 `create_session_document / list_session_files / delete_session_file_binding`（PG + memory 双实现；memory 的 binding id 即 artifact id）。
- **PATCH 统一（D3 已消化）**：`PUT /chat/sessions/:id` → `PATCH`，router/client/安全契约测试一次性切换，无并存路由。
- 前端：`SessionFileTray`（ChatCanvas 个人会话挂载，token 化样式遵守 design baseline）；首传幂等 `POST /chat/sessions` 建会话 + `onSessionChange` URL 同步不重挂；每文件 uploading/parsing/ready/failed + 重试/移除；任一文件非 ready 阻断发送并显示提示；`lib/chat/client.ts` 新增 6 个端点函数；`updateWorkspaceSession` 切 PATCH。
- 门 B（已通过）：contracts/common/ingestion/app-core 41/app-documents 9/app-chat 87/delegate_contract 15/api_key_security 16/storage-pg 35（新增 session file roundtrip 用例）；typecheck + 529 passed/2 skipped；`git diff --check` 干净；`code-review-graph update` 完成。
- **附带修复（门 B 失败驱动的既有缺陷）**：`repository_conversation_memory.rs::resegment_chat_message_search_tokens` 以会话级 `set_config(is_local=false)` 设置 `app.current_role=super_admin` 且不恢复，池化连接对后续任意租户事务永久绕过 RLS admin 分支；改为单连接清扫 + 结束时无条件恢复 `''`。
- 尚未验证：真实 worker 管线下的上传→解析→ready 端到端（属波尾/staging 验证，按 AGENTS mid-wave 不强制）。

### W2c（2026-09-02，完成）

- `execute_chat_pipeline` 新增 `recompute_allowed_doc_scope`：allowed = 本会话 `conversation_document_bindings ∩ Completed` ∪（`workspace_id?` 时）`completed_workspace_doc_ids`（W2a 后已 binding 派生）；客户端 `doc_scope` 与 allowed 求交集后写入 `req.doc_scope`，`run_lead_workers` 注入 tool args 的路径随之天然被 `dispatch_scoped` 强制。
- 新失败语义：rag 轮次客户端选择了文件但交集为空 → `invalid_doc_scope` fail-closed，不静默回退全工作区；原「personal + rag + 空 scope → docscope_required」结构门不变。
- 持久化 `session.model_role` 不受文件/Web 影响（未触碰）；RuntimeExecute 路径 W2a 已天然 binding 化。
- 门 C（已通过）：app-chat 88（新增跨会话 doc_scope 丢弃回归测试）、delegate_contract 15、workspace tests check 除既有 rag-core 漂移外零错误、`git diff --check` 干净、`code-review-graph update` 完成。

### W2d（2026-09-02，完成）

- `TurnScopeFacts`（service.rs）：会话 binding (binding_id, artifact_id) + 工作区 binding artifact 集，scope 强制与引用归属共用同一真相。
- **Snapshot**（user 行 `turn_metadata.context_snapshot`，一次写入不可变）：`workspace_id_at_send / session_binding_artifacts / workspace_binding_artifacts / web_enabled / model_role / created_at`。`effective_provider / effective_model / credential_source` 留待 W3 BYOK 落地时补齐（已登记 §7）。
- **Evidence**（assistant 行 `turn_metadata.turn_evidence.segments`）：channel=rag 的 `source_scope / artifact_id / chunk_id / page / asset_id / parse_version`，随 `persist_chat_execution` 一次写入。
- **引用 scope 标注**：`Citation.source_scope`（wire 契约，persist 时标注——persist 先于 SSE Done，故 live 事件与落库同源）；`Citation.citation_status` 为 W2e 墓碑预留。
- 前端：引用徽标 `data-scope` + 悬停说明（本会话文件 / 工作区资料）；「可用范围」由 W2b 文件托盘 + capability chips 承担。
- 门 D（已通过）：contracts crate 15+12+2+6、app-chat 88、前端 typecheck + 529 passed/2 skipped。

### W2e（2026-09-02，完成）

- **会话文件删除 → 零 binding GC**：`delete_session_file_binding` 返回 artifact_id；新 port 方法 `delete_document_if_unbound`（PG 在单事务 FOR UPDATE 下判零 binding → 复用完整 delete_document 流程：软删 + idempotent cleanup task + ingestion dead-letter）；失败伪装不可能（无 binding 才动作）。
- **工作区删除孤儿清扫**：PG `delete_workspace` 同事务先捕获绑定 artifact、级联删 binding 后逐个判定零 binding → 软删 + 入队 cleanup + dead-letter 在途 ingestion；memory adapter 同语义（inline 标 Deleting）。
- **引用墓碑**：worker 清理序列新增 `prune_document_citations_to_tombstones`——按 doc_id 重写 `chat_messages.citations` JSONB，剥离 `content/preview/image_url/asset_id` 并置 `citation_status="source_deleted"`，保留文件名/页码/ID 等不可还原事实；前端渲染禁用态墓碑 chip（`data-citation-status`）。
- 门 E（已通过）：storage-pg 37（新增 GC 幂等、双 binding 保留、工作区级联孤儿清扫用例；W2a 时代「孤儿保持 pending」断言按 W2e 语义升级为 `deleting`）、app-chat 88、delegate 15、api_key 16、app-core 41（单线程）、前端 529/2、`git diff --check` 干净、`code-review-graph update` 完成。
