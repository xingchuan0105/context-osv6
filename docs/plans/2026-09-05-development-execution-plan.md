# Rust 在线优先与 GPUI 对等：开发执行计划

日期：2026-09-05。状态：E0/E1/E2/D0.1/E3.1/E3.2/E3.3/E3.4/E3.5 已全部完成并通过，**G3 达成**（E3.5 见任务记录 [`2026-09-05-e3-5-admin-help-task.md`](2026-09-05-e3-5-admin-help-task.md)）；下一步 E4 (W4 公共 SSR / SEO / 双语)。

本计划细化 [09-04 路线图](2026-09-04-rust-web-first-gpui-parity-roadmap.md)，保留其阶段编号与优先级；架构以 [ADR-0011](../adr/0011-rust-web-gpui-desktop.md) 晚间修订为准，产品路径以 [PRODUCT_IA](../design/PRODUCT_IA.md) 为准。旧迁移设计只引用仍有效的产品不变量、路由和内容要求；Tauri CSR 与性能 20% 开发门不再执行。

## 1. 起点与执行范围

本次按“单切片推进”处理：后续执行采用本地 `master`、单个实现切片在途；严格遵守门禁推进。

| 当前对象 | 本次核查事实 | 不能据此声称 |
|---|---|---|
| Git | HEAD `82076d9b`；已本地提交 E0 (G0)、E1 (G1)、E2 (G2)、D0.1、E3.1、E3.2、E3.3、E3.4 | 已与远端实时同步、已部署 |
| Chat-first 后端与 Next | W0–W3 已提交；第九轮修复文档记录 L1 OK | 本轮已复跑，或 Rust UI 已全部对等 |
| Rust W1 (E1) | 全量 71 路由矩阵就位、样式守卫增强、资源 hash/br 与未 hash 样式无 immutable 实测通过、Tiptap 方案明确 | 已有完整管理后台与公开营销站 |
| Rust W2 (E2) | W2.1–W2.9 全部完成，十二条 Chat-first 不变量逐一验收通过，**Gate 2 (G2) 达成** | 完整管理后台已迁完 (属于 E3.5) |
| 桌面 D0.1 | `desktop/core` 抽离，复用 `web-sdk::SseDecoder`，多字节中文跨包无损重组已验证并提交 (`b3ce4ad7`) | GPUI 已完全对等 (属于 D1~D6) |
| Rust E3.1 | 登录/注册/密码重置三步走/登出 + `/settings?tab=providers` 自备密钥闭环已验证并提交 (`b2e71dfc`) | 完整管理后台已迁完 (属于 E3.5) |
| Rust E3.2 | `/dashboard` 概览与建库、`/dashboard/:id` 工作台与持久资料/笔记、分析页面已验证并提交 (`03bb02a6`) | 完整公开站已迁完 (属于 E4) |
| Rust E3.3 | 分享中心、访问日志、流量分析、`/shared/kb/:token` 公开问答、`/shared/u/:id` 主页与邀请已验证并提交 (`65ab035a`) | 完整管理后台已迁完 |
| Rust E3.4 | `/pricing` 定价与钱包充值、`/upgrade/paywall` 拦截说明、`/upgrade/success` 支付成功、`/desktop/buy` 购买页已验证并提交 (`82076d9b`) | 完整管理后台已迁完 (属于 E3.5) |
| 最近测试证据 | Playwright 38/38 passed (33.7s)；desktop-core 4/4 passed；unit tests 101/101 passed；live smoke 2/2 passed (7.5s) | 未包含的后台运维用例已通过 |
| Rust 页面 | `App` 挂载对话、工作区、分享、公开知识库、主页、邀请、认证、设置、定价、充值等 24 个端点；全量 71 路由矩阵已建立 | 已实现 admin 页面 |
| GPUI | 独立初始 crate；`desktop/core` 已就绪可供复用 | 已实现本机产品、已通过三平台验收 |

后端现有契约是迁移输入。发现真实 API 缺口时记录最小复现和阻断项，另立后端任务；前端不能伪造成功态或新增第二协议绕过。

## 2. 顺序、交付物与依赖

```text
E0 当前改动收尾 → E1 W1 验收补齐 → E2 W2 完整聊天
  → D0.1 桌面流解析修复 → E3 W3 应用与交易 → E4 W4 公共 SSR
  → D0.2–D0.4 宿主抽库 → D1 GPUI 真聊天 → D2–D5 桌面对等
  → W5 Web 切流（另批）→ D6 三平台验收与发布（发布另批）
  → X 最终删除旧前端（全部消费者退出后，另批）
```

其中 W5 只承担 Web 切流及其专属运行时退役；整库删除 `frontend_next` 移到 X。原因是 Tauri 桌面、导航 parity、测试和构建仍可能消费 Next 源码/产物。此拆分保留原路线图的开发顺序，补全删除依赖，不增加长期兼容层。

| 阶段 | 交付后用户可完成什么 | 前置 | 出口 |
|---|---|---|---|
| E0 | 稳定上传会话文件，选择文档/网络检索 | 当前工作区 | G0：改动经验证、记录完整、本地提交 |
| E1 | 可靠恢复登录态、加载当前聊天页 | E0 | G1：SSR/hydrate、鉴权、样式与资源交付验证 |
| E2 | 个人与 Workspace 共用完整聊天体验 | E1 | G2：十二条 Chat-first 不变量有对应证据 |
| D0.1 | 桌面中文流不因网络分块损坏 | G2；若独立复现用户影响可提前 | 共享 decoder 回归 + Windows 点验 |
| E3 | 管理工作区、设置、分享、交易与后台 | G2、D0.1 | G3：每个应用路由族业务闭环 |
| E4 | 浏览完整中英文公开站与帮助内容 | G3 | G4：路由、HTML、SEO、内容与法律文本对等 |
| D0.2–D1 | GPUI 本地登录、真实聊天、取消 | G4、D0.1 | GD1：Windows 真机里程碑 |
| D2–D5 | 在 GPUI 完成现有桌面全部工作 | GD1 | GD5：逐项桌面对等 |
| W5 / D6 / X | 完成发布切换和旧实现退出 | 各自 Gate 与发布批准 | 发布证据、回滚演练、零旧消费者 |

Gate 未过停在当前阶段。阶段内只推进不依赖失败项的准备工作，不能把失败标成已完成。Gate 0 性能数据仍是观察项，不因未达 20% 停开发，也不记作收益已证明。

## 3. E0：先收尾 W2.4–W2.5

范围：现有 `frontend_rust` 工作区的 session files、scope、REST、ChatCanvas、CSS、相关测试与 lockfile；保留其他已有修改，不整批提交工作区。

| 次序 | 工作 | 必要验收 |
|---|---|---|
| E0.1 | 核对当前 diff、未跟踪源码、现有测试和构建产物；建立本次证据目录 | 报告包含 HEAD、工作区差异标识、命令、起止时间、退出码、测试数量/skip、日志路径 |
| E0.2 | 验证新会话首次上传及已有会话上传 | 首次只创建一个个人 Conversation；签名上传→complete→processing→ready；不暗建 Workspace |
| E0.3 | 验证删除、轮询、失败重试与导航竞态 | 删除后迟到 GET 不复活文件；DELETE 失败可恢复并告知；A 会话迟到响应不改写 B；解析中不能发送；失败能重试 |
| E0.4 | 验证自动 RAG 与手动能力选择 | ready 0→1 自动增加 RAG，1→0 去掉自动 RAG；手动选择按现行语义保留；Web 独立；实际请求 capabilities/agent_type 正确 |
| E0.5 | 运行定向 Rust 测试、native/WASM 检查、浏览器文件/scope 旅程；波尾真实文件链路 smoke | 有已知文档内容与引用的真实问答、刷新恢复和删除证据；fixture 通过不替代真实入库/检索 |
| E0.6 | 处理本切片失败项；更新图谱、核对 diff、归档并本地提交 | 提交包含新增模块和测试；路线图明确“实现/验证/提交”状态；无夹带文件 |

当前零字节 `rebuild-playwright.log` 文件名末尾含回车，只记作工作区残留，不能用作测试证据；收尾时确认来源后处理。本计划不删除它。

预计有效开发时间 0.5–2 个工作日，主要变量是竞态与真实入库问题；这不是实测工期承诺。

## 4. E1：补齐 W1 验收和迁移清单

复用已有实现；只修验证发现的缺口。

1. **凭据与 SSR**：持久/会话存储恢复、空凭据、过期、`/api/auth/me` 超时、登出；auth hint 不作授权依据；不同 SSR 请求不共享用户状态；无 hydration 错误。
2. **资源交付**：实测 br/gzip、WASM MIME、内容 hash、缓存头和 HTML→WASM/CSS 一致性；普通未 hash CSS 不能误设永久 immutable。记录传输体积，不直接宣称性能提升。
3. **路由矩阵**：逐一扫描当前 Next 的 page/route/metadata/static 消费者，而不是沿用历史“69 页”计数；记录实际路径、canonical、auth、render、noindex、数据接口、Rust 挂载状态、验收和生产 owner。现有 `ROUTE_FAMILIES` 只覆盖导航条目，不能代替全量矩阵。
4. **样式与可访问性**：当前 `style_baseline_guard` 主要扫描 `style/` 与 `src/`，补核对实际运行的 `assets/style/chat-poc.css`；按现行颜色、字重、阴影规则校验，覆盖键盘输入、焦点恢复、ARIA、窄屏和减少动画偏好。
5. **编辑器风险提前验证**：读取现有 Tiptap 依赖和用法，确认可靠的 Leptos/DOM 接入边界，验证 Markdown 往返、撤销、粘贴清洗、中文输入；先复用已用依赖。不能以手写 contenteditable 替代原编辑行为。

产物：路由矩阵、G1 验证记录、编辑器接口选择与未解阻断；不提前搬完整设置、后台或公开站。预计 1–2 个工作日，编辑器结论不成立时重估受影响路由。

## 5. E2：完成 W2 Chat-first

每项独立切片，沿用 `web-sdk` / `web-ui` / `web-server` 分工和唯一 ChatCanvas。任务结束后才叠加下一项。

| 次序 | 范围 | 完成标准 |
|---|---|---|
| W2.6 | `model_role`、当前模型与 quick-chat BYOK 状态读取/切换 | 个人默认 quick_chat；agent 与 quick_chat 配置分离；历史恢复一致；错误和计费来源遵循后端响应，不由前端猜测 |
| W2.7 | feedback 与回答操作、引用补齐 | feedback 真实持久化且失败可感知；原文定位、图片引用、已删除来源按实际契约展示；现有复制/代码/表格行为纳入对照 |
| W2.8 | Workspace 最小 shell 内复用 ChatCanvas | `/dashboard/:id?session=:sid` 正确恢复；全局最近区分归属；来源选择/`doc_scope` 增量叠加；快速切库无旧流污染 |
| W2.9 | 会话/文件归属动作与综合收尾 | 移动对话不自动把附件入库；“加入工作区资料”走新增 binding；既有 Snapshot/Evidence 历史事实不被改写；stop/retry/断流/刷新链路完整 |

W2.6 先消费现有配置和 API；完整 provider 编辑页属于 E3.1。W2 验收可复用已有账户配置与测试 fixture，但不能把“仅会读配置”记成“Rust 设置已迁完”，也不能上线一个唯一出口指向未挂载设置页的流程。

G2 按设计 §7 的十二条不变量逐条映射：

| 不变量 | 验收归属 |
|---|---|
| 1 普通会话不暗建库；4 两类文件生命周期 | E0 + W2.9 |
| 2 Workspace 共用聊天；3 显式带入；5 最近会话归属 | W2.8 |
| 6 模型角色独立；7 Quick Chat BYOK 独立 | W2.6 |
| 8 Web 独立；9 scope 增量叠加 | E0 + W2.8 |
| 10 完整聊天交互 | W2.7 + W2.9 |
| 11 分享不能扩大授权上下文 | G2 先做共享 ChatCanvas 的受限上下文测试；E3.3 完成真实分享页旅程，不把前者记为后者通过 |
| 12 Snapshot/Evidence 叠加不破坏会话 | W2.9 的请求与历史证据对照 |

G2 包含 fixture 旅程与本地真实 API smoke（个人、文件 RAG、Web、Workspace、取消/重试、BYOK 来源）；性能对照单独归档。全部关键用例结果明确后才能记“W2 完成”。预计 3–6 个工作日，基于尚未校准的切片规模估计。

## 6. D0.1、E3、E4：修桌面流，再完成在线产品

### D0.1：最小宿主库切片

先用跨块中文 UTF-8、CRLF/多行、坏帧、取消/终态夹具复现；将桌面 `chat_stream` 的实际流处理抽入 `desktop/core`，复用 `web-sdk::SseDecoder`，Tauri 调用该库。核实已无产品用途的许可门后删除；删除被替代的解析代码。

验收：正常/异常流、单一终态、取消与晚到事件测试，加 Windows 现有 Tauri 真机点验。库无 Tauri/GPUI 依赖，宿主用事件回调；此时不展开全部本机栈。预计 0.5–1.5 个工作日。

### E3：W3 应用与交易，依赖顺序固定

| 次序 | 路由族/交付 | Gate |
|---|---|---|
| E3.1 | 登录/注册/密码重置/登出 + `/settings`（先 providers，再其余 tab） | 认证过期、回跳、安全错误；自有 Key 配置完整闭环，不记日志明文 |
| E3.2 | `/dashboard`、Workspace 管理、来源/上传/笔记 | CRUD、入库状态、错误恢复、编辑器往返与历史；复用 W2 聊天 |
| E3.3 | 分享中心、公开分享、分享者主页、邀请 | owner/visitor/未授权/失效路径；访问者无法扩大 scope；分享者主页开关语义 |
| E3.4 | analytics、用量、pricing、top-up、paywall、success | 现有会员名额/钱包语义；支付回跳与失败/取消；使用既有测试模式，不发起真实扣款 |
| E3.5 | `/admin/*`、应用帮助及矩阵余项 | 全部实际后台路由；非 admin 拒绝；错误、分页、空态；未知路由行为 |

E3 每个路由族单独验收、单独本地提交。鉴权与写入约束由既有后端保证；SSR metadata、响应式和可访问性逐族验证。交易套餐/价格来自现行契约，迁移不重新设计商业模式。

### E4：W4 公共 SSR / SEO / 双语

按“中英文首页与下载 → 帮助与 integrations → legal → metadata/static 与全站校验”推进。根路径 PoC 重定向在此阶段由产品首页替换；登录用户导向 `/chat`，公开 SSR 内容仍可抓取。

交付覆盖 canonical/hreflang/noindex、结构化数据、robots/sitemap/manifest/llms.txt、OG/图标/验证文件、状态码与重定向。法律文本保持结构、数字、日期、版本与链接逐项一致；不借迁移改写条款。G4 使用路由矩阵、HTML/metadata crawl 对照和关键页面视觉对照。

E3/E4 工期先按路由矩阵和编辑器结论拆分，不给未经盘点的全站完工日期；每个路由族预计 0.5–2 个工作日，编辑器、支付与权限复杂项另估。

## 7. GPUI：宿主复用到全量对等

| 次序 | 实现范围 | 独立验收 |
|---|---|---|
| D0.2 | local session、PG/Redis、本机产品与生命周期抽库 | Tauri 调库仍可启动/迁移/登录/退出；不误停非本应用进程 |
| D0.3 | documents、REST、上传、本地目录接口抽库 | 上传范围、失败恢复、目录与本地服务不可达行为 |
| D0.4 | cloud session、Publish、深链及剩余宿主能力 | 本地/云身份分离、发布状态/失败恢复、深链验证；明确 updater/单实例在 D5 的平台 adapter 边界 |
| D1.1 | 校准依赖、窗口、composer、中文 IME | 开工时核对 gpui-kit 所锁来源/版本并锁 rev；Windows MSVC 本地路径构建，真实输入无重复字 |
| D1.2 | 共享 reducer 流式显示、local session 真 API、取消、历史 | `127.0.0.1:18080` 本机产品链路；无云账号也能进入；真实流与取消通过 |
| D2 | 栈状态、启动迁移、进程/日志、退出收摊 UI | 对照本机栈 S0–S6；不依赖预装 Docker |
| D3 | Workspace、文件、入库、引用、会话与笔记 | 真实知识路径与已有桌面交互逐项对等 |
| D4 | BYOK、角色、云登录、钱包、Publish | 全量桌面对等范围，不能沿用旧 smoke 中“连云可选”将其豁免 |
| D5 | updater、深链、单实例、浏览器打开、MCP/CLI | 安装后实际行为、离线/失败恢复；不以无窗口单测替代 |
| D6 | 同一 crate 的 Windows/macOS/Linux 构建与点验，安装/升级/卸载、发布产物 | 三平台各有证据；机器或签名条件不足则相应项待验，不判整体完成 |

D0 开工时创建 `docs/desktop/GPUI_PARITY_CHECKLIST.md`，结合现有桌面代码、现行 roadmap 与旧 SMOKE 清单建立条目。每项记录 Windows/macOS/Linux 的实现、验证、产物与结果，覆盖全部能力，不照抄旧版号或旧“可选”结论。D1 只是中途里程碑，不能用于宣布可删 Tauri。

桌面工期在 D0 抽库边界与 D1 IME 验证后重估。机器可用性、GPUI 依赖和安装更新通道均是日历时间变量，不把 WSL 编译成功当三平台就绪。

## 8. 发布与最终清理

发布准备与实际发布分开。W5 批准前完成可审阅的构建产物、路由 owner 清单、正式部署脚本变更、资源清单、回滚方案和本地/隔离验收；之后再提交实际切流批准。

| Gate | 具体条件 |
|---|---|
| W5 Web 切流 | G3/G4/GD5 通过（沿用现行排期）；所有在线路由完成；版本化产物可回滚；观测窗口、样本量、错误率/成功率/延迟阈值在首批切流前按基线冻结 |
| D6 桌面发布 | 三平台对等与安装/升级/卸载验证；更新通道、签名和故障回退方案可审阅；实际发布另批 |
| X 删除旧实现 | Web 观察窗口结束、桌面对等与发布 Gate 完成；Next/Tauri 源码、资产、导航 parity、合约生成、测试和脚本消费者逐项迁出；删除另批 |

仅通过 `scripts/deploy-*.sh` / 现行正式桌面发布脚本交付。每条生产路由只有一个 owner；rollback 恢复整份上一产物，Rust 内不留兼容分支。云端仅使用现行 `VPS_MAIN_*`；不引入已退役主机或其他环境。

X 同步将唯一导航定义移入 Rust，更新 PRODUCT_IA 和守卫，移除 Next 消费路径与只服务旧实现的依赖。发布包可保留用于回滚，但产品源码不长期双轨。

## 9. 验证成本与记录规范

以下为未来执行时的命令与初始耗时估计，不表示本轮已运行或已获批。每组运行前按实际缓存和范围报耗时并获得批准；已有明确覆盖本组的授权不重复询问。WSL `CARGO_BUILD_JOBS=2`，不叠加完整 cargo 测试。

| 验证 | 工作目录与命令/方式 | 初始时间预算 |
|---|---|---|
| Rust 纯模型与 integration tests | `frontend_rust`：`cargo test -p web-sdk -p web-ui`；不能只跑 `--lib` 漏掉 `tests/` | 热缓存 1–5 min |
| SSR/hydrate 检查 | `frontend_rust`：`cargo check -p web-server --features ssr`；`cargo check -p web-ui --target wasm32-unknown-unknown --features hydrate` | 1–5 min |
| 浏览器产物 | `frontend_rust`：`cargo leptos build` | 热缓存 2–8 min；冷缓存另估 |
| fixture Playwright | `frontend_rust/tests/browser`：`pnpm exec playwright test --config playwright.config.ts`；切片可用 `--grep`，报告明确范围 | 1–5 min，不含构建 |
| 真实链路 | 同目录 live config；复用 `.env`，先确认 API health 为真实 API 响应，再跑所需旅程 | 聊天 2–10 min；入库 5–20 min，按文件重估 |
| 样式/无障碍/视觉 | 本次涉及页面和交互的自动检查与人工点验 | 每族 10–30 min |
| 桌面 | 定向库测试 + Windows MSVC/真机；按现行脚本执行 Tauri 点验 | 热构建 2–10 min；首次 GPUI 依赖预算 5–15 min，超出则报告 |
| 图谱 | 结构改动完成后 `code-review-graph update`，检查状态 | 增量通常数秒，规模变化另估 |

后端未变更时不以全量后端测试代替前端验收；如后端缺口另立任务，则遵循相关 crate `--lib` 和波尾 L1。长 E2E 采用日志、后台与 watchdog，按仓库约定报告进展。

任务记录统一存入 `docs/plans/<date>-<slice>-task.md`；证据可放 `docs/engineering/_reports/<date>-<slice>/`。每次完成至少记录：范围与不变量、变更文件、源码/产物对应标识、命令与退出码、passed/failed/skipped、测试环境和时间、剩余问题、图谱状态、提交号。日志不包含密钥或 JWT。不把“未运行”或“环境不足”改记为通过。

## 10. 第一轮执行队列

1. 建立 E0 任务与证据目录，复核当前未提交 diff 和文件/scope 竞态。
2. 给出本组 Rust 检查、构建与浏览器验收的实际时间预算，按授权运行；遇失败先收敛。
3. 完成真实文件链路 smoke、图谱更新与本地提交，记录 G0。
4. 补 E1 的全路由矩阵、W1 验收与编辑器风险结论。
5. 依次 W2.6 → W2.7 → W2.8 → W2.9，达到 G2 后进入后续阶段。

时间估计为单开发主线的有效工作量，不含等待批准、环境修复、机器/签名条件或远端服务等待。先用 E0/E1 实际耗时校准 E2；未完成路由盘点和 GPUI IME 验证前不承诺整项目上线日期。
