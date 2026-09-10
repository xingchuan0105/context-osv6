# GPUI 客户端开发交接

交接日期：2026-09-11。代码基线：本地主干 `master`，交接前 HEAD 为 `5289d13d`（`feat(gpui): finish native layouts and automate visual acceptance`）。本文件是时间点快照；后续状态以新提交、验收回执及 [GPUI 对等清单](GPUI_PARITY_CHECKLIST.md) 为准。

## 1. 接手结论

GPUI 已具备 Windows 本机聊天、流式停止与历史、工作区资料/笔记、服务生命周期的可用切片，刚完成公共壳和主要页面的 UI 收口。最新 UI 批次 **78 项自动测试通过，24 张原生截图生成，12 张由代理逐图复核，独立 Windows 程序已构建**。本次 UI 尚无用户逐项视觉确认，不能把更早的聊天人工验收套用到新界面。

下一功能阶段是 D4（BYOK、模型角色、云登录/钱包/Publish）；尚未开工。本次用户只要求写交接文档，不应据此直接开始 D4、重跑付费验收、重启服务或部署。GPUI 全量 Tauri 对等、三平台和发布门仍未完成，不能删除 Tauri。

## 2. 用户意图与不可丢失的约定

- 用户先完成 Rust Web 的 Grok 布局改造并验收，再推进 GPUI；随后明确要求优先完成原生 UI，避免长期停留在功能原型。原生布局沿用已确认基准，不承诺逐像素复刻 Grok。Web W5 的 72 个入口/288 次模拟布局检查见 [W5 矩阵](../design/2026-09-09-grok-w5-matrix.md)；这是独立历史证据，不能当作 GPUI 验收。
- **禁止 Computer Use。** 验收优先自动判定，不要求用户逐项点击或反复关闭窗口。已建立无头交互及隐藏原生窗口截图通道。
- 个人聊天用于快速对话，不出现知识库检索。用户要求的个人附件语义是“仅本轮上下文”，需要办公文件支持；**GPUI 个人附件尚未接入**，不能误把已通过的工作区 Office 入库等同于个人附件完成。
- Workspace 是唯一持久知识容器。工作区入口到总览；进入对象后才显示该工作区会话。资料默认收起；“资料”“笔记”各有直达入口，“添加资料”直接选择文件，取消不展开。笔记独立保存，不自动进入问答资料。
- 已有用户确认：回答逐段出现；停止立即生效且保留片段；停止后续聊无旧流追加；切走再切回保留内容；中文输入法正常；后来 Markdown/标题/复制修正也获“全部正常”确认。详见 [真实聊天记录](2026-09-10-windows-big-object-build.md)、[Markdown 验收](2026-09-10-gpui-markdown-titles.md)。这不包括客户端进程重启恢复、服务端模型请求取消或本次新视觉。
- 长构建、完整 E2E、真实模型及影响运行服务的操作按当前 [AGENTS.md](../../AGENTS.md) 报耗时并取得该范围授权；已授权的同批工作不要重复询问。历史付费/刷新许可不是新批次的无限授权。
- 本地 `master`、仅提交任务文件，不 push/PR/部署。保留其他任务的未提交改动；不盲目停止服务或清理数据目录。

## 3. 进展与证据分层

| 阶段 | 已有证据 | 尚未覆盖 |
|---|---|---|
| D0 共享宿主 | 已有 desktop-core 的传输、本机会话、上传代理、原生进程与数据栈；Tauri 与 GPUI 复用 | D0.4 云会话、Publish、深链相关抽库仍未完成 |
| D1 Windows 聊天 | 真实模型及窗口流式、停止、历史切换、中文 IME、Markdown/标题有用户确认 | 三平台、客户端重启恢复、服务端模型取消不能由上述结果推出 |
| D2 本机服务 | 共享/无头 60 项；真实 PG/Redis 4 步与产品进程 8 步；失败恢复、超时退出、归属与清理通过 | 系统文件管理器、macOS/Linux、安装升级 |
| D3 工作区 | 创建/切换、作用域、上传恢复、引用原文、笔记和草稿保护；真实 API 工作区/笔记/非法上传旅程通过 | 其余知识能力仍需对照完整清单 |
| D3.2 Office/RAG | `6387bf93`；87 项原生定向测试，真实旅程 1 条/13 步；DOCX/XLSX/PPTX 入库、3 次限定文档问答、引用、计量/钱包/审计、清理通过 | 仅三份小型合成办公文件；没有图片/图表理解、幻灯片渲染或全量质量结论 |
| 最新 UI | `5289d13d`；78 项共享/无头测试；24 PNG；12 张代理复核；独立 `ui` 特性程序 | 非阻断中文断行细节；用户新视觉验收；三平台 |
| D4 / D5 / D6 | 尚未开工 | 设置与云能力 / 系统集成 / 三平台及发布 |

证据入口：[D2 受管进程](2026-09-10-gpui-managed-acceptance.md)、[上传恢复与真实 API](2026-09-10-gpui-upload-recovery-acceptance.md)、[Office 解析](2026-09-10-gpui-office-parser-acceptance.md)、[Office/RAG 最终门](2026-09-10-gpui-office-rag-acceptance.md)、[UI 收口](2026-09-10-gpui-ui-finish-acceptance.md)。

注意：对等清单 D3 行和 Office/RAG 报告中保留了早期失败/待验记录。应读后续“最终真实门通过”，不能停在旧段落，也不要抹掉失败历史。不同日期、不同层级的 60/73/78/87 不能相加称为一次全量测试。

## 4. 最新 UI 的实现位置

| 文件 | 责任 |
|---|---|
| [main.rs](../../desktop_gpui/src/main.rs) | ChatApp 状态、Host 更新和窗口组合；旧整页 Render 已删除 |
| [ui.rs](../../desktop_gpui/src/ui.rs) | 尺寸、基础控件、外观偏好、Surface 与原生 sheet 层 |
| [shell_view.rs](../../desktop_gpui/src/shell_view.rs) | 导航、作用域历史、页头、模态导航与资料抽屉协调 |
| [chat_view.rs](../../desktop_gpui/src/chat_view.rs) | 空态、阅读区、消息层级、单工具栏 composer、复制与状态 |
| [knowledge_render.rs](../../desktop_gpui/src/knowledge_render.rs) | 工作区总览、资料/笔记/原文呈现；业务逻辑仍在 knowledge_view/workspace |
| [service_view.rs](../../desktop_gpui/src/service_view.rs) | 服务状态、操作与返回；宿主生命周期仍由原有模块负责 |
| [presentation_tests.rs](../../desktop_gpui/src/ui_tests/presentation_tests.rs) | 5 条新布局/抽屉/偏好旅程，既有 21 条 UI 用例继续保留 |
| [visual_preview.rs](../../desktop_gpui/src/visual_preview.rs) | 仅测试特性：合成数据、隐藏原生窗口、DirectX 像素读回 |

尺寸：导航 248/60px，页头 52px，聊天最大 760px，总览最大 1120px，宽屏资料 336px；<768px 导航抽屉，768–1199px 默认折叠导航，<1200px 资料抽屉。窄屏工作区操作独占第二行。偏好存于客户端目录 `gpui-appearance.json`；移动抽屉开关不持久化。详见 [Product IA](../design/PRODUCT_IA.md)、[本批计划](../plans/2026-09-10-gpui-ui-finish.md)。

Kit 原生 sheet 已承担焦点圈定、Esc、点击遮挡与焦点返回；`Surface` 将 sheet 渲染放在 ChatApp 可变借用外，避免重入借用冲突。不要为下一阶段重新搭建第二套壳或模态系统。

## 5. 当前程序、环境与已核验内容

| 用途 | 路径/身份 |
|---|---|
| 主源码 | `/home/chuan/context-osv6`；Windows 同一目录为 `\\wsl.localhost\Ubuntu\home\chuan\context-osv6` |
| Windows 源码镜像与缓存 | `C:/dev/context-osv6`；通过 sync-windows.ps1 同步，不是第二个开发主本 |
| 最新验收程序 | `C:/dev/context-osv6/desktop_gpui/target/debug/desktop-gpui-acceptance-20260910-201230.exe` |
| 程序 SHA-256 | `AFC1FF11778A7C68F170ADCC7774C8D46AE850AD015899CA76267D4B61385430` |
| 原生后端编译目录 | `C:/dev/gpui-backend-target/x86_64-pc-windows-gnu/debug`；具体后端身份以 Office/RAG 报告为准 |
| 旧独立聊天环境 | `C:/dev/gpui-acceptance-20260909`，历史 API 18082、会话目录 `session-current` |

2026-09-11 本次写交接时只读核验：最新验收 exe 存在且哈希匹配；构建回执 `features=ui`；回执的 24 个源码/清单文件哈希仍与主源码匹配；无头回执 26 通过；截图回执成功、24 张 PNG 全部存在；归档日志目录存在。**没有重跑测试或探测/启动服务**，不要据此声称 18082 或其他历史服务仍在运行。

最新程序为独立名称，不覆盖旧 `desktop-gpui.exe`，且本次交付没有启动可见窗口。现有 `run-windows.ps1` 和 `run-isolated-gpui.ps1` 仍固定启动 `desktop-gpui.exe`，**它们不会自动选择上述新版验收 exe**。接手若要增加新版启动入口，应显式接收已核验的 exe 路径，保留现有 runtime、API PID/路径及 session 归属校验，不能偷偷替换旧程序或启动后端。

## 6. 验收材料定位

以下路径的共同前缀：`C:/dev/context-osv6/desktop_gpui/target/acceptance/`。这些是本机证据，未提交进 Git；换机器前需要另行转存或在获授权后复现。

| 材料 | 相对目录/文件 |
|---|---|
| 最新无头 26 项及源码哈希 | `headless/gpui-ui-5999907db3284c01b2e094e62c2cf65a/result.json`、`tests.log` |
| 24 组实际几何记录 | 同上目录 `layout-matrix.jsonl` |
| 24 张最终原生截图 | `visual/gpui-ui-2f9a2c54c2d341cdb9cba5804f99f3e8/` |
| 截图回执和程序身份 | 同上目录 `visual-result.json`、`build-identity.json` |
| 正式 ui 特性验收构建 | `build/desktop-gpui-acceptance-20260910-201230/result.json`、`build.log` |
| 防覆盖的共享日志/身份检查 | `ui-finish-20260910-201230/` |
| Office/RAG 最终真实门 | `managed/gpui-managed-85c78b19711a464c9f7238fd77b421e2/`，`journey.json` 与 `result.json` |

UI 测试数为 30 shared-core + 17 GPUI lib + 4 原流协议 + 1 HTTP 夹具 + 26 UI = 78。默认列表中 opt-in 的真实服务/模型用例不计通过；HTTP 夹具单独运行通过，其余 5 项本次 UI 批未运行。

原生截图六场景：`empty`、`chat`、`workspaces`、`workspace-documents`、`workspace-notes`、`services`，文件名为 `{scene}-{1280|390}-{light|dark}.png`。逻辑窗口 1280×800、390×720；当时 175% 缩放下 PNG 为 2240×1400、683×1260。合成内容不等于真实业务数据。

## 7. 可复现的自动验证流程

下列为接手命令，不是本次交接已重跑的记录。先按修改影响选择范围和取得必要耗时授权，串行运行 Cargo；失败先修复，不跨过失败门。Windows 使用 MSVC、jobs=2，无头 UI 单线程。

```powershell
Set-Location -LiteralPath '\\wsl.localhost\Ubuntu\home\chuan\context-osv6'
pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/sync-windows.ps1
pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/accept-tauri-shared.ps1 -WithHttpFixture -WithHeadlessUi
pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/render-previews.ps1
pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/build-acceptance.ps1
```

共享 HTTP/无头/截图夹具分别使用隔离端口 18180/18181/18183，不应改指向真实服务。截图脚本构建测试特性程序，以一个不显示、不聚焦的窗口读回实际 DirectX 像素，自动检查 24 文件完整性；不使用 Computer Use。正式验收构建只有 `ui` 特性，不含捕获模式。

真实验收另选对应脚本：`accept-workspace-live.ps1` 会写入/清理真实测试 API 对象；`accept-managed.ps1` 会启动全新隔离栈；`accept-managed.ps1 -OfficeRag -ApiPort 18192 -PgPort 15440 -RedisPort 16390` 会产生嵌入与模型费用。它们均不是普通 UI 回归的附带动作。脚本参数、前置条件和完整证据见各阶段报告；旧端口占用时应核对归属或换新测试端口，不先杀占用进程。

## 8. 已知余项与建议接手顺序

| 顺序 | 工作 | 完成标准/边界 |
|---|---|---|
| 1 | 若继续视觉收口，修复窄屏中文句号单独换行 | 已见于 `chat-390-dark.png`；检查 Kit/Markdown 的断行处理，保留原文和复制结果，不为截图改写答案；定向排版回归与相同场景截图通过 |
| 2 | 需要给用户启动新版时，补充独立 exe 启动入口 | 选择正确新版；沿用原生环境/服务归属检查；不覆盖旧程序、不顺手重启后端 |
| 3 | D4 先形成一个可运行切片 | 建议先核对已有 Tauri BYOK/角色 API 与共享宿主边界，再实现配置读取、编辑、保存、失败反馈；形成小批计划并验证。本顺序是建议，尚未开发 |
| 4 | D4 后续云能力 | 云会话与本机会话分清；复用既有钱包/Publish，不建立第二套支付或业务协议；具体验收另立范围 |
| 5 | GPUI 个人本轮附件 | 单列对等缺口；复用已有解析契约，办公文件只进本轮上下文，不借工作区索引冒充实现；不能遗漏此项 |
| 6 | D5/D6 | 系统集成、三平台、安装/更新/卸载；全量对等达成后才讨论移除 Tauri |

当前没有仍在执行的本批构建或测试需要接续。不要为了写文档、查看进度或查验回执重新跑整套测试。

## 9. 排错经验与工作树保护

- 编译和运行目录：在 `C:/dev/context-osv6/desktop_gpui` 执行 Cargo，避免从 UNC 父目录继承不同 registry 配置导致冗余编译；不要删除现有缓存。
- GPUI 测试：`.test_support()` 在测试特性下会改变包装类型；相关渲染返回 `impl IntoElement + use<>`。按钮需先滚入可视区并确认可达；原生抽屉动画要有界等待最终位置，不能放松边界断言掩盖失败。
- 隐藏截图：Windows 隐藏窗口需要显式 resize 分配目标；复用一个窗口，全部回执写完再退出。早期 1×1 图片和提前关闭最后窗口的失败不能算渲染通过。
- Windows 后端曾受 DuckDB COFF section、扩展与 C++ 运行库问题阻断；已在 Office/RAG 批解决。不要恢复旧 DLL 猜测路径、自动下载扩展或旧 PPTX 每页渲染校验。完整根因及前后证据见 Office/RAG 报告，不能把一次普通 UI 构建升级为后端重编。
- 历史隔离 API 曾漏传价格表和 QUICK_CHAT_LLM/DASHSCOPE 配置，造成“价格缺失”和“LLM client is not configured”。配置相关工作应读实际 `avrag-rs/.env` / `.env.example` 并静默复用；文档与日志不写密钥。
- 交接时 GPUI 目录及本批设计/验收文件干净；仓库有 Subtex、transport-http、若干 Cargo.lock、docs/README.md、研究/文章等其他任务的未提交改动。不要 `git add .`、reset、clean 或代为提交。开始前重新检查状态；结构修改前查 code-review-graph，结构修改后更新，不提交图索引。

本次交接只添加本文和 GPUI README 的入口链接，做文件/哈希/链接/diff 检查，不编译、不运行 E2E、不更新代码图、不操作服务。
