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
  → AppShellGate:无 cloud_session → 云登录页(email+password,**Rust 侧 reqwest HTTPS**,不经 WebView fetch)
  → 登录成功 → 云端 POST /api/v1/desktop/tokens 铸 desktop token(长期、可撤销、relay 专用)
  → 云端 GET /api/v1/desktop/relay-config 取 relay base + pin 住型号(壳不硬编码型号)
  → Tauri 写 cloud_session.json(%APPDATA%\com.contextos.desktop)
  → native_stack 写 client.env:
      AGENT_LLM_* / EMBEDDING_* / INGESTION_LLM_* = 云 relay base_url + desktop token + pin 型号
      AVRAG_PLATFORM_KEYS_RELAY=1(标记:平台用量已由云端计量,本地跳过 wallet debit)
      (rerank / memory / triplet LLM v1 不注入 —— 无 relay 路由,见 §8)
  → 重启本机 api/worker(壳已有进程管理)
  → 本机 stack/产品/会话自举(现状不变,本地数据仍属 device-derived 本地账户)
```

- **relay 端点(云)**:`POST /v1/relay/chat/completions`(SSE 透传,注入 `stream_options.include_usage`)+ `POST /v1/relay/embeddings`。认证只接受 desktop token(不放行全 API)。上游 = 平台现有 `AGENT_LLM_*`/`EMBEDDING_*` 池;按 usage 真实 token 走 `PgUsageObserver.record_chat/record_embedding` → `debit_platform_usage`(model 必须在 `wallet_pricing.rs` 白名单,否则 fail-open 不扣 —— 上线前核对白名单覆盖 relay 型号)。
- **relay-config 端点(云,W3 新增)**:`GET /api/v1/desktop/relay-config`(session JWT)→ `{relay_base_url, chat_model, embedding_model}`;`relay_base_url = <AVRAG_PUBLIC_BASE_URL>/v1/relay`,型号取平台 `AGENT_LLM_MODEL` / `EMBEDDING_MODEL` 配置 —— 壳不硬编码型号。
- **desktop token**:`desktop_tokens` 新表(id, user_id, name, token_hash, prefix, created_at, last_used_at, revoked_at);`cos_dt_` 前缀,只存 hash。
- **双扣防护**:relay 模式下平台用量的钱包扣费已在云端发生,本地 api/worker 不得再扣。实现缝(W3 落地):`app-billing` `PgUsageObserver::maybe_debit_wallet` 读 `AVRAG_PLATFORM_KEYS_RELAY=1`(进程级 OnceLock)整体 early-return —— chat/embedding 与 TaskTenantUsageObserver 全部收口于该点;usage 行仍落本地库(作本地用量展示),仅跳过 ledger debit。
- **BYOK 优先序不变**:用户填了 BYOK secret → 每请求 overlay 覆盖平台 relay(`bind_byok_client` 现状),且本地/云端都不计平台价。
- **身份**:本地 RLS 仍用 `uuidv5(device_id)`;cloud user_id/email 只存 cloud_session.json 作账户展示与充值/余额查询(`GET /api/v1/billing/wallet` 用 cloud JWT)。

## 4. 关键缝(侦察结论,实现时按此下刀)

- 登录门挂在 `frontend_next/components/desktop/AppShellGate.tsx:33-42`(`(app)` 路由唯一收口,桌面分支);登录页是桌面壳内页,不是云端 `/login`(该页对 Tauri 强制跳走,`app/(auth)/login/page.tsx:38-55`)。
- **登录走 Rust 侧 HTTPS(reqwest),不经 WebView fetch(W3 偏差,已定)**:CORS 依赖整体消失 —— 原计划的「VPS `.env` `CORS_ALLOWED_ORIGINS` 部署门」不再需要,`router_core.rs` 默认 CORS 与本 wave 无关。结构化错误(401 凭证错误 / 网络不可达)由 `IpcApiError` 透传到登录卡片。
- token 存储/client.env 注入放 `desktop/src-tauri/src/commands/` 新 `cloud_session.rs`,复用 `local_session.rs` 的 0600 写盘模式;`native_stack.rs` 生成 client.env 时读 cloud_session 注入 relay 三元组(agent/embedding/ingestion)+ `AVRAG_PLATFORM_KEYS_RELAY=1`。`native_stack` 无 AppHandle,经 `dirs::data_dir()` + bundle identifier 定位 cloud_session.json(与 tauri PathResolver 同规则)。
- relay 路由挂 `router_core.rs` 新 `routes/desktop.rs`(token 中间件) + `routes/relay.rs`(转发 + 计量)。

## 5. 抽屉重设计(PRODUCT_IA 先行,见本文 §6)

| 区块 | 内容 |
|------|------|
| 账户 | 云账户邮箱、余额 + 充值(外开 `/pricing#topup`)、退出云登录(撤销本机 token) |
| 模型 | 当前来源:官方(走余额)/ 自备;切换入口 → `/settings?tab=providers` |
| 数据 | 数据目录、打开日志目录 |
| 关于 | 版本号、客户端页/定价链接 |

移除:本机数据栈 tab 整页(栈启停/裸 client.env/CLI 提示/Monorepo 提示)、本机产品进程、本机个人账户块;顶栏 `DesktopStatusBadge`「已激活」胶囊一并撤掉(Keygen 残余,`license_allows_chat` 已恒 true)。诊断价值的内容(栈状态/日志路径)折叠为「诊断」区,默认收起。

**W4 落地备注(2026-08-15)**:抽屉壳内用 `NavRail` 左导航(账户/模型/数据/关于/诊断)替代旧 tab;「诊断」作为最后一个 rail 项即默认收起,栈状态在选中时才探测,产品状态(日志目录来源)在选中「数据」或「诊断」时才探测(均懒加载)。新增 IPC:`cloud_wallet_balance`(会话 token → `GET /api/v1/billing/wallet` → `{logged_in, balance_fen}`(分,钱包为 CNY 计价,前端渲染 ¥);401/403 → 结构化 `cloud_session_expired`;未登录 → `logged_in:false` 结果)、`open_data_dir` / `open_logs_dir`(系统文件管理器直开,与 `open_in_browser` 共用 `system::open_with_os` 直启 helper,不引插件)。模型区「当前来源」按 §3 BYOK 优先序显示:存在任一未撤销的 `provider-secrets`(经 `listProviderSecrets`,复用 settings seam)即显示「自定义 Provider(自备 Key)」+ 已配置 provider 条目;否则已登录显示「官方模型(走余额)」+ relay 钉住型号。退出云登录 / 未登录点「登录」→ 关抽屉并整壳 reload,由 `CloudLoginGate` 重新接管显示登录卡。数据区路径:数据目录 = `get_app_data_dir`,日志目录 = `get_local_product_status().log_dir`。`desktop.status.*` i18n 键与 badge 专用 CSS 随组件一并删除。

## 6. PRODUCT_IA.md 变更(先改文档再改代码,§9 流程)

- §2 Object model:Client 行补「默认平台 key 走钱包余额(云登录后下发 relay 凭据);BYOK 为高级选项」。
- §3.2 认证:补「桌面壳内云登录门(非路由,客户端首次打开)」。
- §5 Shell rules:补桌面 shell 行(登录门 → 工作台;抽屉 = 账户/模型/数据/关于)。
- §6 Taxonomy:补「官方模型(走余额)」用语,BYOK 保留「自定义 Provider / 自备 Key」。

## 7. 分片与验证门

| W | 内容 | 验证门 |
|---|------|--------|
| W1 | 本文 + PRODUCT_IA.md | 文档落库,IA 自检(§4 每条入口有 canonical) |
| W2 ✅(312d5e01) | 云端:`desktop_tokens` 表 + 铸/列/撤 + relay chat/embeddings + 计量接线 + 白名单核对 | `cargo test -p transport-http -p app-billing`;curl 真云收发 + 钱包扣费行 |
| W3 | 桌面:登录门页 + cloud_session.rs + client.env 注入 + relay 双扣防护 + 登录后重启本机产品;云端补 `relay-config` 端点 | `pnpm typecheck` + 桌面包冷启动真机:登录 → 不发 BYOK 直接 RAG 问答成功且云端钱包扣费 |
| W4 ✅ | 抽屉重设计 + 撤「已激活」badge + i18n(落地备注见 §5) | `pnpm typecheck` + `tests/desktop` 全绿 + nav-config 测试;`design-baseline` 仅剩既有 WIP 违规(home-client / api-access / help 页,非本片) |
| W5 ✅ | 打包 + l0/l1/l2/u1 全绿 + VPS 部署 + l3 真机门(落地备注见 §8) | `scripts/desktop-e2e/run.sh` 五模式;`scripts/deploy-*.sh` |

排序原则(layered growth):W2 云端 relay 可独立curl 验收;W3 出最小端到端(登录→relay→扣费);W4 纯前端可插在任何空档。

## 8. 风险 / 明确不做

- relay 流量经过 VPS:带宽与延迟成本;SSE 必须流式透传,不能缓冲。
- desktop token 泄漏 → 用户可在云端设置撤销;v1 不做 token 轮换提醒。
- **follow-up:rerank relay**。v1 client.env 不注入 `RERANK_*` / memory / triplet LLM(relay 只有 chat + embeddings 两条路由);rerank 走余额需先加 `POST /v1/relay/rerank` 路由 + 白名单价目,再补注入。本地无 rerank 配置时检索退化为无 rerank 路径(现状行为)。
- 不做:离线首启、usage 回传云对账(本地行与云行各记各的)、桌面端充值页(一律外开 `/pricing#topup`,PRODUCT_IA §4 禁止第三 checkout)。
- 旧 `DesktopSettingsDrawer` 栈管道代码在 W4 删除,不留兼容层(no backward compatibility tax)。
- **E2E 绕行(W5)**:`scripts/desktop-e2e/l0.ps1` 在 KeepRunning(playwright)模式以 `CONTEXT_OS_SKIP_CLOUD_GATE=1` 启动壳;`cloud_gate_bypassed` IPC 为 true 时登录门直接放行——E2E 环境没有真实云账户,预置假 session 还会把 relay 块写进 client.env 改变 chat-unconf 等 spec 的语义。生产安装永不设置该变量。登录门本身由 `tests/desktop/cloud-login-gate.test.tsx` 单测 + l3 真机验收覆盖。

**W5 落地备注(2026-08-16)**:

- **门在栈自举之前是硬设计**:无云会话冷启动不开栈端口。l0/l1/l2/u1 一律注入 `CONTEXT_OS_SKIP_CLOUD_GATE=1`(不只 KeepRunning),只有 **l3**(`run.sh l3`,grep `cloud-login`)用 `-NoCloudGateBypass` 保留真门:keep 阶段只验窗口标题 + CDP(端口/健康/client.env/会话都是登录后的事),spec 走 登录卡 → 填 `.env` 的 `DESKTOP_E2E_CLOUD_*` → 等门释放(不得提前导航——重挂载的门不重查会话)→ 入库 → RAG → 引用。云凭据经 run.sh 从 `avrag-rs/.env` 映射;`cloud_session.json` 已纳入 backup/restore(可重复跑)。
- **云端部署缺口(已补)**:nginx `app-contextlm.conf` 缺 `/v1/relay/` location(SSE 透传配置同 `/api/`),relay 公开路径 404。已补并实部署;`deploy-public-sites.sh` 的 nginx 清单改为 `NGINX_CONFS`(默认含 app-contextlm.conf)+ `ONLY_NGINX=1` 外科手术模式,防再漂移。
- **ensure_native 并发竞态(产品修复,l3 揪出)**:门释放后 bootstrap 会并发触发多次 `ensure_native`;两遍 ensure 交错时,后者在前者 initdb 半途重跑 initdb(code=1),或探到 TCP 开但 SQL 未就绪 → createdb 失败 → **旧代码仍写 `.avrag_inited`**,之后所有 pass 跳过 createdb,api 崩在 `database "avrag_client" does not exist`。修复(`native_stack.rs`):进程级互斥锁串行化 ensure + TCP 开后等 SQL 就绪再 probe + **createdb 验证成功才写标记**。真实用户首启登录同样可能踩中,属本 wave 最重要的产品修复。
- **代理**:E2E 那台 Windows 的系统代理(127.0.0.1:20000)对云端主机不稳定;reqwest 0.13 默认吃系统代理 → l3 登录失败。harness 在 l3 给壳注入 `NO_PROXY=app.contextlm.top` 直连。生产保持默认(尊重系统代理;代理不可用时门给出明确的「云端不可达」错误)。
- **E2E 前置**:`run.sh` 的 l0/l1/l2/l3 用**当前安装**的客户端,不自动装包;换包先 `C:\temp` 本地拷再 `/S` 静默装(从 Z: 网络盘直接跑 NSIS 会挂起)。u1 跨版本(发布版 v0.2.0)被旧包 fresh-bootstrap 不应用迁移阻塞(旧包自身缺陷,现行代码已修),u1 沿用 self 语义(旧=新=当前包)。
- **验收数**:l0/l1/l2 链全绿(l1 6 过 2 跳,l2 D-rag-full 56s),u1-self 绿,l3 4 分钟过(门→登录→relay RAG→引用);云端钱包 2000 → 1998(冒烟)→ **1980**(l3 扣 18 分);本地侧 `AVRAG_PLATFORM_KEYS_RELAY=1` 双扣防护(W3 cargo 测试覆盖)。
- **杂项**:l0.ps1 `Close-AppProcesses` 对 `MainWindowHandle==0` 加 5s 重试(旧包观察到的窗口句柄瞬态);测试号 `e2e-desktop-20260816@contextlm.top` 凭据在 `avrag-rs/.env`(`DESKTOP_E2E_CLOUD_*`)。
