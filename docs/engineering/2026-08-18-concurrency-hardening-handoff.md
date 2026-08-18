# 抗并发改造交接（W1–W5 已落地，W6 回归挂死未决）

日期：2026-08-18。前置文档：`2026-08-16-retrieve-split-event-log-handoff.md`（§15 = VPS E2E 基建）。

## 0. 当前状态一句话

**W1–W5 仍在工作树未提交。full-149 / E2E RAG 挂死根因已定：W2 `SharedConn` 对 E2E 黑洞 `redis://127.0.0.1:1` 按默认 ConnectionManager 重试（第二次约 100s），卡在限流中间件、打不到 LLM。`SharedConn::get` 已改 fail-fast。W3 SSE 不是这条路。生产仍跑旧版。**

## 1. 已落地的改动（全部已编译验证 + 单测绿）

### W1 止血
- `transport-http/src/middleware.rs`：用户层限流 **429 前置**（原先是请求执行完才判定，超限流量照烧 LLM）。边缘 IP 层、share 层原本就是前置。
- `router_core.rs`：全局 `DefaultBodyLimit` 512MB→**64MB**；`/uploads/{id}`（`routes/infra.rs`）与 `/dev-upload/{id}` 用 `RequestBodyLimitLayer::new(512MB)` 保持大上限（tower-http 0.6 是 `new` 不是 `max`，workspace 加了 `limit` feature）。
- `storage-pg/.../repository_bootstrap.rs`：PG pool `AVRAG_PG_MAX_CONNECTIONS`（默认 20）+ acquire 30s + idle 300s（E2E 分支 25 不动）。
- `deploy/docker/run-avrag-containers.sh`：容器 `--ulimit nofile=65536`。

### W2 连接与资源管理
- `cache-redis/src/conn.rs`（新增）：`SharedConn` — 懒初始化 `redis::aio::ConnectionManager`（自动重连、Clone 共享），替换 `CacheStore`/`DocumentLock`/（app-bootstrap）`RedisRateLimitBackend` 里每操作新建 TCP 连接的做法。`DocumentLockGuard::drop` 仍保留阻塞释放兜底（shutdown 安全网）。workspace redis 加 `connection-manager` feature。
- `llm/src/client/rate_limit.rs`：进程级 `SHARED_LIMITERS` 注册表（key = base_url+model+api_key 哈希+限额），**BYOK 每请求重建客户端不再重置令牌桶**；上限 4096 桶防爆。
- `turnstile.rs`：静态共享 `reqwest::Client`（5s 超时），不再每请求新建。
- SIGTERM：api（`bins/api/src/main.rs`）与 worker（`bins/worker/src/lib.rs`）都接 ctrl_c+SIGTERM；worker 当前 tick 跑完再退。

### W3 SSE 背压（ripple 最大）
- 终端 ChatEvent 通道 `unbounded_channel` → `tokio::sync::mpsc::channel(512)`，链条：`transport-http/handlers/chat.rs` → `app-bootstrap/product_apps/conversation.rs` → `app-chat/chat_streaming.rs` → `chat/pipeline.rs`（`StreamConfig.sender`）。
- `agent-loop/src/sse_sink.rs`：`SseSink.sender: Sender<ChatEvent>`；**生产路径 `emit` 走 `send_async`（await 容量=真背压）**；sync `send` 保留给测试/同步上下文（满则 try_send 丢弃+warn）。`ensure_answer_started` 同步转 async。
- `pipeline.rs::emit_share_cache_hit`、`pipeline_steps.rs::emit_terminal_stream_events` 随之 async 化。
- agent 内部 `UnboundedSender<String>`（delta 流）与 `ChannelSink<AgentEvent>`（仅测试用）**未动**。

### W4 状态外迁（水平扩展前置）
- `app-chat/src/share_cache.rs`：exact 层加 Redis L1（`init_shared_cache`，bootstrap 接线 `app-bootstrap/src/lib.rs`），进程内降为 L2；语义层留在进程内（miss 只多一次 LLM）。app-chat 新增依赖 `avrag-rag-core-ports`。
- share 日限额：`RedisRateLimitBackend::check_window(key, limit, 86400)`（窗口参数化），Redis 失败回落内存。
- 余额通知节流：`chat_private::init_funds_notify_cache`，Redis get/set 6h（get-set 有良性竞争窗口，重复通知代价可接受）；内存兜底保留。

### W5 观测 + worker 吞吐
- 慢查询：`PgConnectOptions::log_slow_statements(Warn, 500ms)`（workspace 新增 `log = "0.4"`，storage-pg 引用）。
- `/metrics`：`avrag_ingestion_queue_depth{status}` gauge（scrape 时查 PG）；handler 改吃 `State<AppState>`（`state.postgres_pool()` 现成）。
- worker 副本：`AVRAG_WORKER_REPLICAS`（run-avrag-containers.sh 循环起 `avrag-worker-N`，超编自动清 stale）；**周期 side job（billing/outbox/usage-export/retention/analytics/memory/audit/orphan）非领取制，锁副本 1**——新闸 `AVRAG_WORKER_SIDE_JOBS=0`（bins/worker/src/lib.rs tick 与 heartbeat 两处）。ingestion + document cleanup 是 SKIP LOCKED 领取制，多副本安全。

### 压测资产（未执行）
- `scripts/loadtest/mock_llm.py`：零依赖 OpenAI 兼容 stub（chat SSE/非流式、responses、embeddings、models；`STUB_DELAY_MS/CHUNKS` 可调），本地已实测。
- `scripts/loadtest/sse_load.py`：httpx 阶梯并发 SSE 压测（P50/95/99、TTFB、错误率；`x-owner-user-id` 头认证）。
- `scripts/loadtest/collect_vps.sh`：VPS 指标采集 CSV（CPU/mem/load/TCP/PG/Redis 连接、inflight、SSE 活跃）。

## 2. 阻塞问题：full-149 回归挂死（未决，嫌疑已重排）

**现象**：新二进制在 VPS 上 8/149 派发后 900s 无输出被看门狗杀；本地 3 题同样挂死（E2E harness，不是 `cargo run` API）。
**评测热路径**：`post_rag_chat` 发 **`stream: false`**（`test_context/http.rs`）。`run_single_question` 先 `eprintln` 题号再 `await` 整段 JSON——8 路全卡住时日志就会「派发 8 题后沉默」，正好撞上 900s 看门狗。

### 2.1 已证实（本地 `avrag-api` :18081）

| 探测 | 结果 | 结论 |
|------|------|------|
| SSE 无 `workspace_id` | HTTP 200、0.2s 关连接、**空 body**；日志 `first SSE event must be start, got "error"` | 出站闸丢掉 Start 前的 Error |
| 同上，修闸后 | `event: error` `notebook_required` | 空 body 根因就是这闸 |
| SSE 有 workspace | TTFB 12ms 即 `start`，可跑到 `done` | **W3 bounded(512) 不卡首包** |
| 8× `stream=false` 无 workspace | 8/8 约 100ms 回 400 `notebook_required` | **W1 Redis 前置限流 + W2 SharedConn 在 8 并发下不卡** |
| 8× `stream=false` 有 workspace chat（真 LLM） | 7/8 在 7–10s 回 200 短答；**1/8 curl 25s 零字节超时** | 卡在 **preflight 之后** 的 execute/LLM |
| 8× 同上，`AGENT_LLM_*` 指到 `mock_llm.py :8399` | **8/8 在 0.23–0.30s 回 200**，答案为 stub 固定句 | **W1–W5 execute/Redis/PG 在 8 并发下不挂**；真 LLM 那 1/8 是上游 |

漏掉的那路（真 LLM）：`request_id=301ed81d-…`，preflight 有、persist 无。`CHAT_LLM_TIMEOUT_MS=180000`。stub 对照后可视为上游挂死，不是我们的锁。

W3 顺手修：`pipeline.rs` OperationGuide `send` 漏了 `.await`（bounded 下丢事件）。

### 2.2 已落地的闸修复（未提交）

- `transport-http/src/sse_order.rs`：允许唯一帧为 `error`（preflight/session 失败必须到客户端）。单测 `patho_stream_tracker_accepts_error_before_start` 绿。
- `app-chat/src/chat/pipeline.rs`：OperationGuide `.await`。

这能解释「直连 stream=true 零字节」；**解释不了** full-149 的 900s 全体挂死（那条路根本不走 SSE）。

### 2.3 根因（full-149 / E2E RAG）

E2E 故意把 Redis 写成 `redis://127.0.0.1:1`（`e2e-gates.md`：黑洞，让 embedding mock 生效）。W2 之前每次操作新建连接，`:1` 立刻 RST。W2 换成默认 `get_connection_manager()` 后，backon `min_delay=1s` × `factor=100`，第二次重试睡约 100 秒；`check_rate_limit_with_fallback` 只在 `Err` 时回落内存，于是 `/api/v1/chat` 一直不写 body。`/health` 不走该中间件所以正常。`cargo run` 连真 `:6379`，所以 8 路 chat 对照是绿的。

**不要再先回退 W3。** 评测心跳已加：`await_with_heartbeat`（30s）包住 `post_rag_chat` 和 judge（`rag_quality_prod.rs`）。900s 看门狗不再把「8 路卡在模型 HTTP」误判成挂死。VPS 上要重编 product_e2e 二进制才生效。

**E2E stub 对照（已跑，2026-08-18）**：`E2E_QUESTIONS=1,2,3 E2E_CONCURRENCY=3 RAG_EVAL_V2=0`，`AGENT_LLM_*` 指 `:8399`。三题同时 `chat qN: start` 后心跳到 240s+ 仍无 `done`；当时 **ss 上没有任何连 8399 的 TCP**。

**根因（2026-08-18 单题对照）**：`E2E_CONCURRENCY=1` 同样挂。`/health` `/metrics` 秒回，`POST /api/v1/chat` 零字节。进程 **没有 Redis/LLM/embedding TCP**。E2E 把 AppConfig Redis 写成 `redis://127.0.0.1:1`（文档里的 embedding-mock 黑洞）。W2 `SharedConn` 调默认 `get_connection_manager()`：backon `min_delay=1s` × `factor=100` → 第二次重试睡 ~100s，超时中间件永远走不到内存回落。`cargo run` API 用真 `:6379` 所以 8 路 chat 没事。

`SharedConn::get` 已改为 1s 连接超时、只试 1 次。`E2E_QUESTIONS=1,2,3 E2E_CONCURRENCY=3` + stub：**3/3 `done` ~353–357s，`test result: ok`，recall@15=100%**。原先「派发后打不到 LLM」的挂死已核销。stub 答案导致 citation/halluc 分数无意义，不挡。

## 3. 遗留事项

- **L1 门槛预存红**：`crates/llm/src/llm/embedding.rs` 1234 行 > 1000 硬限（`scripts/check_file_size_limits.sh`），HEAD 上 `f8430600` 就已超线，与本次无关，需单独立项拆解，否则 L1 永远红。
- **回归过了之后**：`deploy-backend.sh` 上生产（含 worker 副本/ulimit 改动）→ 压测 L0–L4（设计见会话内评估；核心产出 = 单副本 SSE 并发安全水位、PG pool 拐点、Redis 连接表现、worker 排水速率）。
- 压测四条预测待核销：PG 10 连接 50–100 并发饱和（已调到 20，拐点应后移）、Redis 建连风暴（已修）、worker 排水 ~10 篇/分钟、事后限流不省成本（已修）。
- git：W1–W5 + §2.2 闸修复 **未提交**（在工作树）。回归未过，仍不是提交/部署时机。
- 本地诊断 API 可能仍在 `127.0.0.1:18081`（勿动 8080 上的旧 `avrag-api`）。

## 4. VPS 速查（沿 §15）

- 二进制：`~/e2e-vps-target/debug/deps/product_e2e-375c9f81277a2267`（新 hash，W1–W5）；worker `~/e2e-vps-target/debug/avrag-worker` 已同步到 VPS。
- VPS runner：`/opt/avrag-e2e/run-binary.sh`（BIN 已指向新 hash）；日志 `/opt/avrag-e2e/output/runtime-logs/`。
- 构建：`e2e-vps-builder:v1` 容器 + `/tmp/vps-build.sh`（编 test 二进制 + worker，~36s 增量）。
- 注意：容器内编译因 tower-http/redis feature 变化，hash 会从 3cb7f… 变成 375c9…；再改代码 hash 可能再变，rsync 后同步 run-binary.sh 的 BIN 行。
