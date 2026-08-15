# Desktop 云登录门 + 官方 Key 走余额 + 设置抽屉重设计

- **日期**: 2026-08-15
- **状态**: 现行 wave 计划(W1 本文)
- **触发**: 真机发现 —— 桌面端零配置时 `RAG runtime is not configured`(api.log: `enable_rag=true but no embedding client`);客户端设置抽屉全是 dev 栈管道,零用户设置
- **关联**: ADR-0010(商业模式,本计划修正其「模型优先 BYOK」叙事)· `docs/design/PRODUCT_IA.md`(先改)· `docs/desktop/2026-08-10-v0.2.0-free-client-release.md`

## 1. 拍板的决策(2026-08-15,用户)

1. **官方 key 走余额是初始设置**:不自定义 Provider 时,客户端用平台官方 key,计量扣钱包余额(×1.5 代购价,沿用 ADR-0010 `LIST_PRICE_MULTIPLIER`)。
2. **云端登录后下发**:打开客户端 → 云登录页 → 登录成功才下发官方 key;BYOK 是登录之后的高级配置(canonical 仍是 `/settings?tab=providers`)。
3. **抽屉重设计**:移除 dev 栈管道,换成用户视角设置(见 §5)。

## 2. 技术选型:云端 metered relay(而非真实 key 下发)

| 方案 | 选择 | 理由 |
|------|------|------|
| **A. relay**:下发「relay base_url + desktop token」,真实 provider key 不出云 | **采纳** | 计量 = 云端现有 `PgUsageObserver` + `debit_platform_usage`(钱包白名单、幂等、余额不足拒绝全部现成);key 可撤销;无「本地用量回传 + 身份映射」新机制 |
| B. 真实 key 下发到本机,本地计量回传 | 否 | 本地软件 key 必可提取;滥用 = 免费骑我们的 provider 账户;usage 回传/身份映射/防重放全是新工程量 |

桌面端调模型本来就必须联网(BYOK 也调 DeepSeek/SiliconFlow),relay 不新增联网前提。**明确代价**:首启无网 = 无法登录 = 不可用官方 key(v1 接受;BYOK 纯离线模式留作后续)。

## 3. 架构

```text
打开客户端
  → AppShellGate:无 cloud_session → 云登录页(email+password → 云端 POST /api/auth/login)
  → 登录成功 → 云端 POST /api/v1/desktop/tokens 铸 desktop token(长期、可撤销、relay 专用)
  → Tauri 写 cloud_session.json(%APPDATA%\com.contextos.desktop)
  → native_stack 写 client.env:
      AGENT_LLM_BASE_URL / EMBEDDING_BASE_URL / RERANK_BASE_URL = 云 /v1/relay
      *_API_KEY = desktop token
      AVRAG_PLATFORM_KEYS_RELAY=1(标记:平台用量已由云端计量,本地跳过 wallet debit)
  → 重启本机 api/worker(壳已有进程管理)
  → 本机 stack/产品/会话自举(现状不变,本地数据仍属 device-derived 本地账户)
```

- **relay 端点(云)**:`POST /v1/relay/chat/completions`(SSE 透传,注入 `stream_options.include_usage`)+ `POST /v1/relay/embeddings`。认证只接受 desktop token(不放行全 API)。上游 = 平台现有 `AGENT_LLM_*`/`EMBEDDING_*` 池;按 usage 真实 token 走 `PgUsageObserver.record_chat/record_embedding` → `debit_platform_usage`(model 必须在 `wallet_pricing.rs` 白名单,否则 fail-open 不扣 —— 上线前核对白名单覆盖 relay 型号)。
- **desktop token**:`desktop_tokens` 新表(id, user_id, name, token_hash, prefix, created_at, last_used_at, revoked_at);`cos_dt_` 前缀,只存 hash。
- **双扣防护**:本地 `unified/mod.rs:193` 目前仅 BYOK 时 `skip_wallet_debit=true`;relay 模式下平台 env key 的用量云端已扣,本地须同跳 —— bootstrap 读 `AVRAG_PLATFORM_KEYS_RELAY=1` → tenant 同样 skip(usage 行仍落本地库,作本地用量展示)。
- **BYOK 优先序不变**:用户填了 BYOK secret → 每请求 overlay 覆盖平台 relay(`bind_byok_client` 现状),且本地/云端都不计平台价。
- **身份**:本地 RLS 仍用 `uuidv5(device_id)`;cloud user_id/email 只存 cloud_session.json 作账户展示与充值/余额查询(`GET /api/v1/billing/wallet` 用 cloud JWT)。

## 4. 关键缝(侦察结论,实现时按此下刀)

- 登录门挂在 `frontend_next/components/desktop/AppShellGate.tsx:33-42`(`(app)` 路由唯一收口,桌面分支);登录页是桌面壳内页,不是云端 `/login`(该页对 Tauri 强制跳走,`app/(auth)/login/page.tsx:38-55`)。
- WebView → 云端 fetch 走 CORS(非 PNA);`router_core.rs:240` 默认 CORS 已含 `http(s)://tauri.localhost` —— **部署门:VPS `.env` 的 `CORS_ALLOWED_ORIGINS` 不得覆盖掉这两个源**。
- token 存储/client.env 注入放 `desktop/src-tauri/src/commands/` 新 `cloud_session.rs`,复用 `local_session.rs` 的 0600 写盘模式;`native_stack.rs` 生成 client.env 时读 cloud_session 注入 relay 四元组。
- relay 路由挂 `router_core.rs` 新 `routes/desktop.rs`(token 中间件) + `routes/relay.rs`(转发 + 计量)。

## 5. 抽屉重设计(PRODUCT_IA 先行,见本文 §6)

| 区块 | 内容 |
|------|------|
| 账户 | 云账户邮箱、余额 + 充值(外开 `/pricing#topup`)、退出云登录(撤销本机 token) |
| 模型 | 当前来源:官方(走余额)/ 自备;切换入口 → `/settings?tab=providers` |
| 数据 | 数据目录、打开日志目录 |
| 关于 | 版本号、客户端页/定价链接 |

移除:本机数据栈 tab 整页(栈启停/裸 client.env/CLI 提示/Monorepo 提示)、本机产品进程、本机个人账户块;顶栏 `DesktopStatusBadge`「已激活」胶囊一并撤掉(Keygen 残余,`license_allows_chat` 已恒 true)。诊断价值的内容(栈状态/日志路径)折叠为「诊断」区,默认收起。

## 6. PRODUCT_IA.md 变更(先改文档再改代码,§9 流程)

- §2 Object model:Client 行补「默认平台 key 走钱包余额(云登录后下发 relay 凭据);BYOK 为高级选项」。
- §3.2 认证:补「桌面壳内云登录门(非路由,客户端首次打开)」。
- §5 Shell rules:补桌面 shell 行(登录门 → 工作台;抽屉 = 账户/模型/数据/关于)。
- §6 Taxonomy:补「官方模型(走余额)」用语,BYOK 保留「自定义 Provider / 自备 Key」。

## 7. 分片与验证门

| W | 内容 | 验证门 |
|---|------|--------|
| W1 | 本文 + PRODUCT_IA.md | 文档落库,IA 自检(§4 每条入口有 canonical) |
| W2 | 云端:`desktop_tokens` 表 + 铸/列/撤 + relay chat/embeddings + 计量接线 + 白名单核对 | `cargo test -p transport-http -p app-billing`;curl 真云收发 + 钱包扣费行 |
| W3 | 桌面:登录门页 + cloud_session.rs + client.env 注入 + relay 双扣防护 + 登录后重启本机产品 | `pnpm typecheck` + 桌面包冷启动真机:登录 → 不发 BYOK 直接 RAG 问答成功且云端钱包扣费 |
| W4 | 抽屉重设计 + 撤「已激活」badge + i18n | `pnpm test` + `design-baseline.test.ts` + nav-config 测试 |
| W5 | 打包 + l0/l1/l2/u1 全绿 + VPS 部署(CORS 门) | `scripts/desktop-e2e/run.sh` 四模式;`scripts/deploy-*.sh` |

排序原则(layered growth):W2 云端 relay 可独立curl 验收;W3 出最小端到端(登录→relay→扣费);W4 纯前端可插在任何空档。

## 8. 风险 / 明确不做

- relay 流量经过 VPS:带宽与延迟成本;SSE 必须流式透传,不能缓冲。
- desktop token 泄漏 → 用户可在云端设置撤销;v1 不做 token 轮换提醒。
- 不做:离线首启、usage 回传云对账(本地行与云行各记各的)、桌面端充值页(一律外开 `/pricing#topup`,PRODUCT_IA §4 禁止第三 checkout)。
- 旧 `DesktopSettingsDrawer` 栈管道代码在 W4 删除,不留兼容层(no backward compatibility tax)。
