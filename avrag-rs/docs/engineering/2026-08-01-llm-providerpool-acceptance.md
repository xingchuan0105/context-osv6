# LLM ProviderPool 路由策略层 — 验收指示

> 生成窗口:2026-08-01(本窗口开发工作)
> 目标:将本窗口的 LLM 多 provider 路由开发工作移交到新窗口进行验收
> 主提交:`55a6696a` — feat(llm): add ProviderPool routing layer with multi-key rotation and cross-provider failover

---

## 1. 背景与目标

产品部署形态为 **VPS + 多租户公用**,LLM 凭据采用**平台统一 key(租户按订阅/配额计量)**,可用性要求**单 provider 故障自动切换 + 多 key 池**,并明确**保持纯 Rust(不引入 LiteLLM / AI SDK 网关)**。

为此在现有 `avrag-llm` 上新增**路由策略层(ProviderPool)**,补齐缺失的多 key 轮询与跨 provider 故障切换,同时完全保留原有单 route 行为(未配置 pool 时行为不变)。

## 2. 改动清单

| 文件 | 变更 |
|---|---|
| `crates/llm/src/routing/mod.rs` | **新增**。ProviderPool 状态机:多 key round-robin、per-key / per-member 冷却、`try_each` 重试循环、错误分类 `failure_kind` |
| `crates/llm/src/lib.rs` | 导出 `routing` 模块与类型 |
| `crates/llm/src/client/mod.rs` | `LlmClient` 增加 `pool` 字段与 `new_with_pool`;`complete`/`complete_stream` 入口分流;`consume_stream_events` 重构;`record_completion_success` 增加 `track_local_limits` |
| `crates/app-core/src/config_helpers.rs` | **新增** `llm_pool_config_from_env`(`{PREFIX}_API_KEYS` CSV + `{PREFIX}_FALLBACKS` JSON + `{PREFIX}_FAILOVER_COOLDOWN_SECS`)+ 4 个 env 解析测试 |
| `crates/app-core/src/config.rs` | `AppConfig` 增加 `agent_llm_pool: Option<avrag_llm::LlmPoolConfig>`,`from_env` 接线 |
| `crates/app-bootstrap/src/config_helpers.rs` | `make_llm_client` 增加 pool 参数,内部用 `LlmClient::new_with_pool` |
| `crates/app-bootstrap/src/lib.rs` | 两处 `make_llm_client(&config.agent_llm, config.agent_llm_pool.clone())`(仅 agent LLM;memory 传 `None`) |
| `.env.example` | 文档化三个新配置项 |

## 3. 行为规格(验收对照)

### 3.1 多 key 轮询
- 同一 provider 配置多把 key,请求按 **round-robin** 轮流落到各 key(每把 key 独立 RPM/TPM 限流)。
- 某 key 限流(容量不足)时自动跳过,选下一把;同 provider 全限流则落到下一家 provider。

### 3.2 跨 provider 故障切换(failover)
按成员顺序(主 provider 优先)依次尝试,直到成功或候选耗尽:
- `429 / 401 / 403` → **仅冷却该 key**(其他 key 仍可用);
- `5xx / 网络错误 / 超时 / 解析失败 / 协议错误 / 空流` → **冷却整家 provider**(默认 30s,`AGENT_LLM_FAILOVER_COOLDOWN_SECS` 可配);
- `Cancelled / 配置错误 / 其他 4xx` → 不重试。
- 成功会清除该 provider/key 的冷却;失败会退还 TPM 预扣。

### 3.3 流式 failover 边界
- **首个内容 delta(`TextDelta`/`ReasoningDelta`)交付前**失败 → 按 3.2 规则切下一家;
- **已开始交付后**失败 → 不切换,直接报错,并给该 key 加冷却(不退款)。
- 裸 `Finish`(无内容)或首事件即 `ProviderError` → 视为未交付,继续 failover。

### 3.4 记账
- 成功且响应带 usage → 按实际 token 结算;
- 成功但响应无 usage(如 provider 流式不报 usage)→ **保留预扣不退款**(防止 per-key TPM 门失效);
- 失败 → 退还 TPM 预扣。

## 4. 配置方式(全部可选,缺省=原单 route 行为)

```env
AGENT_LLM_BASE_URL=https://api.deepseek.com
AGENT_LLM_API_KEY=sk-xxx
# 主 provider 额外 key(主 key 自动保留在轮换首位,去重):
AGENT_LLM_API_KEYS=key1,key2
# fallback provider(JSON 数组;api_key 或 api_keys 二选一,其余字段缺省继承主配置):
AGENT_LLM_FALLBACKS=[{"base_url":"https://open.bigmodel.cn/api/paas/v4","api_key":"","model":"glm-4.6"}]
# provider 级冷却秒数(默认 30):
AGENT_LLM_FAILOVER_COOLDOWN_SECS=30
```

要点:`API_KEYS` 是"额外 key"(会与 `API_KEY` 合并);`FALLBACKS` 条目缺 `model`/`timeout_ms`/限流时继承主配置;非法 JSON 会 `tracing::warn!` 并忽略 fallback(主 provider 仍进 pool)。

## 5. 验收步骤

### 5.1 编译与单元测试(WSL 下 jobs=2,勿并发堆叠全量测试)

```bash
cd avrag-rs
cargo test -p avrag-llm --lib        # 期望 107 passed(含 routing 8 个新测试)
cargo test -p app-core --lib         # 期望 22 passed(含 config_helpers 4 个新测试)
cargo test -p app-bootstrap --lib    # 期望 11 passed
cargo check -p avrag-worker          # 期望编译通过(调用点无破坏)
```

routing 新测试覆盖:`pick_round_robins_across_keys`、`pick_skips_ratelimited_key_and_moves_to_second`、`fallback_moves_to_second_member_after_provider_failure`、`key_only_failure_keeps_member_alive`、`all_cooldown_yields_no_capacity`、`success_clears_member_cooldown`、`try_each_switches_member_on_failure_and_reports_usage`、`failure_kind_classifies_http_codes`。

### 5.2 跨 crate 兼容(可选,较耗时)

```bash
cargo test -p agent-loop --lib -- --test-threads=1   # 期望 276 passed
cargo test -p agent-tools --lib -- --test-threads=2  # 期望 152 passed
```

### 5.3 行为冒烟(可选,需真实 key)

配置 `AGENT_LLM_API_KEYS`(两把 key)与 `AGENT_LLM_FALLBACKS`(一个备用 provider),启动服务后:
1. 正常对话:验证请求在 key 间轮换(可用 key 维度日志/用量观察);
2. 模拟主 provider 故障(临时改错主 base_url):验证请求自动切到 fallback,响应仍正常;
3. 流式对话:验证内容正常输出。

## 6. 已知事项(验收者必读)

1. **工作区存在大量 pre-existing 未提交改动**(agent-loop / agent-tools / worker / section_index / summary 等,约 54 个文件,非本次窗口产物)。验收时**不要**将这些改动纳入本工作范围的判断。
2. **`app-chat` 有 1 个 pre-existing 测试失败**:`chat::pipeline_tests::tests::inject_assembled_metadata_dual_roundtrips_mode_config`(capabilities 长度断言 2 vs 3),由用户未提交的 mode/capability 改动导致,**与 ProviderPool 无关**,不构成本工作回归。
3. **提交范围**:`55a6696a` 只含本工作 8 个文件。`app-bootstrap/src/lib.rs` 中用户 pre-existing 的 `search_executor` 重构(hunk 3/4)被**选择性暂存排除**,未提交。
4. **仅 agent LLM 接入 pool**:`make_llm_client(&config.agent_llm, ...)`;`memory_llm` 及 worker 侧 summary/section_index 等角色仍走单 route(未接 pool),符合本窗口范围。
5. 本窗口已跑过 `graphify update`,知识图与代码一致(未提交 `graphify-out/`)。

## 7. 验收通过标准(Checklist)

- [ ] `cargo test -p avrag-llm --lib` 全绿(含 routing 8 个新测试)
- [ ] `cargo test -p app-core --lib` 全绿(含配置解析 4 个新测试)
- [ ] `cargo check -p avrag-worker` 编译通过
- [ ] 单 route 路径(不配 `API_KEYS`/`FALLBACKS`)行为与改动前一致(新测试已保证,可抽查)
- [ ] 3.1–3.4 行为规格逐项与代码/测试对应
- [ ] 确认 `55a6696a` diff 不含用户 pre-existing 改动
- [ ] 已知事项 1–5 无遗漏处理
