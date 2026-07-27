# context-osv6 运营与商业化指标体系设计

> Date: 2026-04-08
> Status: Draft for review
> Scope: `context-osv6` 生产运维指标、用户行为指标、商业准备指标
> Audience: product owner, backend, frontend, ops
> 2026-04-26 update: dependency names that include `qdrant` should be read as the historical vector-store dependency. Current retrieval data plane target is Milvus; see [Current Product Architecture](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md).

## 1. 文档目的

本文定义 `context-osv6` 的统一指标体系，用于回答三类问题：

- 系统现在是否稳定，是否需要扩容或告警。
- 用户是否真的在使用产品，在哪些流程流失。
- 虽然产品当前免费，但每个用户消耗了多少资源与潜在成本，未来如何支撑商业化定价。

本设计不直接实现代码。它提供第一期上线所需的指标边界、字段标准、存储位置、聚合逻辑和实施优先级。

## 2. 设计前提

### 2.1 产品前提

- 产品定位为 ToC。
- 商业分析主口径以 `user_id` 为核心。
- `org_id` 仅保留为现有系统兼容字段，不作为经营分析主维度。
- 当前不包含 billing 上线范围，但需要保留未来与成本/计费模型衔接的指标基础。

### 2.2 运维前提

- 云厂商和主机层资源指标由外部平台提供，不在本项目内重复建设。
- 本文不覆盖底层主机/容器/节点监控。
- 项目内只设计应用层、业务层、成本层的指标。

### 2.3 技术前提

- 实时运行指标通过 `/metrics` 暴露给 `Prometheus/Grafana`。
- 用户行为与商业准备指标主要进入 `Postgres` 事件表与聚合表。
- 不把高基数字段如 `user_id` 直接暴露为 Prometheus label。

## 3. 设计目标

### 3.1 必须回答的问题

第一期体系必须能够回答：

- API 错误率是否上升。
- Chat / Search / Upload / Share 的延迟是否异常。
- SSE 长连接是否异常堆积。
- Worker 是否出现 backlog。
- 当前是否应扩容 API 或 Worker。
- 日活、新增、激活用户是多少。
- 哪些核心功能被使用。
- 每个用户每天消耗了多少 LLM / embedding / storage / upload 资源。
- 哪些用户潜在商业价值高，哪些用户潜在成本高。
- 是否存在持续调用 API 的异常循环或无效重试。

### 3.2 第一阶段不追求的目标

- 完整 BI/OLAP 数仓体系。
- 每个指标都做到跨月财务级精确结算。
- 复杂的用户分群模型或机器学习式异常检测。
- 全量 org 级经营看板。

## 4. 指标架构

本体系只保留三层：

### 4.1 L1 服务运行层

目的：回答“系统稳不稳、慢不慢、会不会炸、是否要扩容”。

特点：

- 指标低基数。
- 进入 `/metrics`。
- 由 `Prometheus/Grafana` 消费。
- 支持告警和分钟级排障。

### 4.2 L2 用户行为层

目的：回答“用户是否真正在用产品、在哪些步骤流失、哪些功能被使用”。

特点：

- 以事件明细为核心。
- 进入 `Postgres` 事件表。
- 主要面向日报、周报、行为分析。

### 4.3 L3 商业准备层

目的：回答“免费期每个用户的资源消耗、潜在成本、未来商业化计费价值”。

特点：

- 也进入 `Postgres`。
- 与 `usage-limit`、worker、search、upload 等消耗环节打通。
- 用于未来订阅、按量或混合收费模型的定价前分析。

## 5. 双轨存储与消费模型

### 5.1 Prometheus / Grafana

负责：

- 实时可用性
- 延迟
- 错误率
- 并发/堆积
- 外部依赖健康
- 是否需要扩容

不负责：

- 用户级细分报表
- 留存、激活、成本归因
- DAU/新增的权威口径

### 5.2 Postgres 事件与聚合

负责：

- 用户行为事件
- 成本事件
- 日聚合报表
- 异常用户分析
- 免费期商业准备指标

不负责：

- 秒级/分钟级报警
- 高并发低延迟的 scraping 场景

## 6. 第一阶段指标清单

### 6.1 L1 服务运行层指标

这些进入 `/metrics`：

- `http_requests_total`
  - labels: `route`, `method`, `status_class`
- `http_request_duration_ms`
  - labels: `route`, `method`
- `http_inflight_requests`
  - labels: `route`
- `sse_streams_open`
  - labels: `surface`
- `sse_events_sent_total`
  - labels: `surface`, `event_type`
- `upload_requests_total`
  - labels: `kind`
- `upload_bytes_total`
  - labels: `kind`
- `worker_tasks_started_total`
  - labels: `task_kind`
- `worker_tasks_completed_total`
  - labels: `task_kind`, `result`
- `worker_task_duration_ms`
  - labels: `task_kind`
- `llm_calls_total`
  - labels: `feature`, `provider`, `model`, `result`
- `llm_call_duration_ms`
  - labels: `feature`, `provider`, `model`
- `retrieval_requests_total`
  - labels: `mode`, `stage`
- `retrieval_zero_result_total`
  - labels: `mode`
- `guardrail_blocks_total`
  - labels: `guard_type`, `action`
- `usage_limit_blocks_total`
  - labels: `window`
- `dependency_failures_total`
  - labels: `dependency`

### 6.2 L2 用户行为事件

这些进入 `product_events`：

- `user_registered`
- `user_logged_in`
- `password_reset_requested`
- `password_reset_verified`
- `password_reset_completed`
- `notebook_created`
- `notebook_opened`
- `session_created`
- `session_renamed`
- `session_pinned`
- `session_deleted`
- `document_upload_started`
- `document_upload_completed`
- `document_upload_failed`
- `url_source_added`
- `document_reindexed`
- `chat_started`
- `chat_completed`
- `chat_failed`
- `search_started`
- `search_completed`
- `search_failed`
- `shared_kb_opened`
- `shared_kb_chat_started`
- `shared_kb_chat_completed`
- `citation_opened`
- `source_focused`
- `note_edited`
- `note_synced`
- `share_link_created`
- `share_link_disabled`

### 6.3 L3 商业准备事件

这些进入 `cost_events`：

- `llm_usage_metered`
- `embedding_usage_metered`
- `summary_usage_metered`
- `upload_bytes_metered`
- `storage_snapshot_recorded`
- `external_search_usage_metered`

## 7. 标准字段

### 7.1 `product_events`

必备字段：

- `event_id`
- `event_time`
- `event_date`
- `user_id`
- `session_id` nullable
- `notebook_id` nullable
- `surface`
- `event_name`
- `result`
- `request_id` nullable
- `trace_id` nullable
- `client_platform`
- `metadata` jsonb

约束：

- `surface` 统一枚举，避免自由字符串漂移。
- `result` 统一为 `success | failure | cancelled | degraded`。

### 7.2 `cost_events`

必备字段：

- `event_id`
- `event_time`
- `event_date`
- `user_id`
- `session_id` nullable
- `notebook_id` nullable
- `feature`
- `provider`
- `model`
- `prompt_tokens`
- `completion_tokens`
- `embedding_tokens`
- `usage_units`
- `storage_bytes_delta`
- `external_call_count`
- `source`
- `metadata` jsonb

说明：

- `usage_units` 是统一计费/成本抽象，不替代原始 token 字段。
- 原始 token 和抽象 units 必须同时保留。

## 8. 聚合表

### 8.1 `daily_user_metrics`

用途：

- 单用户日活跃与资源消耗
- 成本估算
- 高价值/高成本用户识别

第一期字段建议：

- `event_date`
- `user_id`
- `is_dau`
- `is_new_user`
- `is_activated`
- `chat_count`
- `search_count`
- `upload_count`
- `shared_kb_open_count`
- `llm_prompt_tokens`
- `llm_completion_tokens`
- `embedding_tokens`
- `storage_bytes`
- `usage_units`
- `estimated_cost_cents`

### 8.2 `daily_product_metrics`

用途：

- 日报、周报、月报
- 产品运营总览

第一期字段建议：

- `event_date`
- `dau`
- `new_users`
- `activated_users`
- `daily_chat_users`
- `daily_search_users`
- `daily_upload_users`
- `daily_shared_kb_users`
- `total_llm_prompt_tokens`
- `total_llm_completion_tokens`
- `total_embedding_tokens`
- `total_upload_bytes`
- `total_estimated_cost_cents`
- `cost_per_dau_cents`
- `cost_per_activated_user_cents`

## 9. 扩容判断

### 9.1 API 扩容信号

同时满足以下信号时触发扩容评估：

- `http_request_duration_ms` 的 p95/p99 持续升高
- `http_inflight_requests` 持续高位
- `http_requests_total` 增长伴随 `5xx` 比例上升或 timeout 增加
- `sse_streams_open` 长时间不回落

### 9.2 Worker 扩容信号

- `worker_tasks_started_total` 增速持续高于 `worker_tasks_completed_total`
- `worker_task_duration_ms` p95 持续升高
- 文档状态 `queued/processing` 数量持续增长且不回落

### 9.3 依赖侧扩容/调优信号

- retrieval 阶段延迟升高且集中在单一依赖
- dependency failures 集中出现在 `postgres`, `qdrant`, `redis`, `smtp`, `llm_provider`

## 10. 异常循环识别

### 10.1 需要检测的异常

- 同一 `user_id + route` 在短窗口内异常重复调用
- 同一 `session_id` 高频创建或高频 chat 提交
- 同一请求模式连续失败后持续重试
- SSE 连接异常重连导致长时间高并发占用

### 10.2 第一阶段实现方式

不在 Prometheus 中直接按 `user_id` 建 label。

而是：

- 事件明细写入 `product_events`
- 周期性聚合出异常用户/异常签名表
- `/metrics` 只暴露异常总量指标，例如：
  - `anomalous_users_total`
  - `suspected_client_loop_total`

## 11. 第一阶段模块边界

建议拆成 4 个模块：

- `crates/telemetry`
  - 负责 Prometheus registry、低基数 runtime metrics、export text format
- `crates/analytics`
  - 负责 `product_events`
  - 负责 `cost_events`
- `crates/analytics_rollups`
  - 负责日聚合与异常检测
- 业务模块侧埋点适配
  - `transport-http`: 请求/延迟/SSE/upload/auth
  - `app`: chat/search/share/use-case
  - `worker`: ingestion/summary/embedding

## 12. 第一阶段上线范围

第一阶段必须完成：

- 最小 `/metrics`
- `product_events`
- `cost_events`
- `daily_user_metrics`
- `daily_product_metrics`
- 异常检测第一版

第二阶段再做：

- 更细漏斗
- 更细成本模型
- 高级异常识别
- 经营分层评分
- 更复杂 BI 看板

## 13. 验证要求

第一阶段完成后，应至少验证：

- `/metrics` 暴露的低基数指标可被抓取
- 核心用户流能写入 `product_events`
- LLM / embedding / summary / upload 能写入 `cost_events`
- 日聚合作业可生成 `daily_user_metrics` 与 `daily_product_metrics`
- 至少一条异常循环规则能产生稳定输出
- 从指标可回答：
  - 今日 DAU
  - 今日新增
  - 今日总 LLM 成本估算
  - Top 20 高成本用户
  - 当前是否出现 SSE/worker 堆积

## 14. 推荐结论

对当前阶段，推荐采用：

- `Prometheus/Grafana` 负责 L1
- `Postgres 事件与聚合` 负责 L2-L3
- `user_id` 作为商业分析唯一主口径
- 免费期先做“商业准备指标”，不提前做收入口径

这是当前最符合 `context-osv6` 上线阶段的取舍。
