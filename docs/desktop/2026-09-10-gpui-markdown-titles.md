# GPUI Markdown 与会话标题

本批承接 D1 Windows 基础聊天验收，仅修正原生聊天阅读与历史辨认体验。用户授权 15–25 分钟构建和验证；窗口操作由用户执行，不使用 Computer Use，不重启后端，不调用付费模型。

## 实现

- 历史与当前助手回答统一使用锁定版本 gpui-kit 的 `TextView::markdown`，保留模型原始内容；不再直接将助手回答挂为纯文本。不另写 Markdown 解析器或引入新依赖。用户问题继续原样显示。
- 渲染标识按会话与消息序号保持稳定，输入下一轮时，上一轮回答进入历史仍使用相同标识。复用组件库的选择、复制与增量内容更新能力；表格使用组件库横向滚动样式。
- 无标题的个人会话从第一条非空用户问题提取名称，合并空白，最多保留 48 个 Unicode 字符，超出显示省略号。使用已有共享 SDK 和 PATCH 会话接口保存，不调用模型，不修改已有非空标题或工作区会话。
- 列表先显示，缺失标题后台按最多 4 个请求任务并发补齐；空会话和名称未加载时显示创建时间与短标识。命名失败显示可重试提示。侧栏左对齐、标记选中会话，长标题省略并提供全文提示。
- `run-isolated-gpui.ps1` 只打开客户端，核对既有独立 API 的 PID/路径和健康状态，复用 session-current；不再为打开新窗口而刷新后端。

## 自动验证

- 共享 Tauri desktop-core lib：19 通过。
- GPUI lib：10 通过，1 项隔离 HTTP 用例单独执行；其中新增 3 项标题测试覆盖首条问题、空白、Unicode 长标题、已有名称及空会话区分。
- 原共享流传输测试：4 通过，覆盖中文跨块、CRLF、错误帧及取消。
- 独立 HTTP 会话/历史用例：1 通过，扩展验证标题 PATCH、重新读取后保留、已有名称不被覆盖、个人与工作区隔离。
- 合计 34 项不同测试通过。初次编译发现共享 JSON 编码器返回字节数组的类型不匹配，修正为 `from_slice` 后复验通过。
- Windows `cargo check --features ui --locked` 通过（2 分 32 秒），`cargo build --features ui --locked` 通过（12.68 秒），均为 jobs=2。已有 ts-rs 属性解析及 desktop-core 未使用函数警告不作为本次新增缺陷。
- 用户已关闭全部旧窗口。新程序由独立启动脚本打开（PID 46820），路径 `C:\dev\context-osv6\desktop_gpui\target\debug\desktop-gpui.exe`；既有 API 18082 / PID 37708 保持运行。启动成功不替代窗口视觉验收。
- 日志：`C:\dev\context-osv6\desktop_gpui\target\acceptance\tauri-shared\` 下 shared-core、gpui-and-original-stream、local-session-history、ui-check、ui-build 日志。代码关系图已更新；仅提交本批 GPUI 和验收文档。

## 用户窗口验收

用户提供新版窗口截图并反馈“应该都正常了”。截图确认历史已恢复，侧栏显示首条问题标题及选中背景，大部分粗体、有序/无序列表已经生效。此反馈记录为主体验确认，不扩展为全部 Markdown 边界或新版流式/停止的再次验收。

截图复核仍发现两处排版余项，未标记清零：

- 首句 `**瑞利散射（Rayleigh scattering）**现象` 中的星号仍直接显示。只读 API 回查原文确认标记内没有多余空格；现有 markdown 1.0.0 的 attention 规则在闭括号紧邻中文时不将后侧 `**` 识别为关闭标记，属于需要处理的中文加粗边界，不能误记为本次已解决。
- 原文 `4. **结果**` 在窗口显示为 `1. 结果`。这是新有序列表起始编号的保留问题，尚待修正。

原文回查会话为 a2fcbbed-56c4-42cb-90b4-e1c491462c29；只读取历史，没有产生模型调用、重启或改写消息。用户截图为 `C:\Users\xingc\AppData\Local\Temp\codex-clipboard-1dad6878-f687-4b49-bf0a-cde4088e3302.png`。

## 2026-09-10 Markdown 余项修正

用户继续授权本批 15–25 分钟定向测试、Windows 增量构建和验收程序更新。仍不使用 Computer Use、不重启后端、不调用付费模型。上节保留首次修正的历史结论，本节记录后续状态。

### 实现

- 保留锁定的 gpui-kit 版本，通过其 `MarkdownPlugin` 块扩展接入 Comrak 0.55.0，关闭不需要的默认特性。现有解析器没有中文友好加粗选项；Comrak 提供 `cjk_friendly_emphasis`，避免自行修改星号、插入空格或重写 Markdown 语法。该依赖是首次修正之后新增的。[Comrak 选项](https://docs.rs/comrak/0.55.0/comrak/options/struct.Extension.html)
- `markdown.rs` 负责源文本解析及保留列表起始编号的数据，`markdown_view.rs` 负责原生块渲染。段落、标题、表格中涉及加粗/强调的内容使用成熟解析器输出；有序、无序和任务列表保留起始编号、嵌套层级及多段内容。代码和转义符不做字符串替换。组件库丢弃列表起始编号已有上游问题记录：[gpui-kit #2633](https://github.com/longbridge/gpui-kit/issues/2633)。
- 模型回答及历史存储原文不变；扩展节点保留原文 Markdown，渲染标识使用会话/消息标识和原文偏移，避免不同回答复用富文本状态。制表符在两个解析器中的显示列号不同，定位统一使用字节偏移，并覆盖 LF、CRLF 和 CR 换行。
- 共用正文渲染入口，历史恢复与当前回答应用相同规则。当前轮次的发送、停止、会话状态及标题保存逻辑未改动。

### 自动验证与交付状态

- 新增 6 项 Markdown 定向测试通过：中文标点旁加粗与引用链接；代码/转义保护；独立及嵌套列表起始编号；标题/表格/任务/多段列表；流式未闭合片段与不同文档隔离；原生解析器与 Comrak 在制表符、嵌套和不同换行下的定位一致性。
- 共享 Tauri 套件重新执行：desktop-core 19 项、GPUI lib 16 项、原始共享流测试 4 项、隔离 HTTP 会话/历史 1 项，共 **40 项不同测试通过**。上述 6 项包含在 GPUI lib 中，不重复计数。
- Windows `cargo check --features ui --locked` 通过（3.35 秒），jobs=2。已有 ts-rs 属性解析和 desktop-core 未使用函数警告仍在。
- 用户确认关闭旧窗口后，Windows `cargo build --features ui --locked` 通过（10.44 秒），jobs=2。新版已通过 `run-isolated-gpui.ps1` 打开，进程 PID 34404；独立 API 18082 的 PID 仍为 37708，未重启后端或调用模型。
- 程序为 `C:\dev\context-osv6\desktop_gpui\target\debug\desktop-gpui.exe`，本地构建时间 2026-09-10 10:13:21，SHA-256 为 `03761D7D679FFE948D6FCE95DF6D06820C151EB9E1159417FC3209DF083B1C4B`。交付时已核实进程启动，原生表现当时待用户复验；后续确认见下节。
- 日志位于 `C:\dev\context-osv6\desktop_gpui\target\acceptance\tauri-shared\`：`markdown-position.log`、`markdown-ui-check.log`、`markdown-ui-build.log`，以及重新运行的 `shared-core.log`、`gpui-and-original-stream.log`、`local-session-history.log`。代码关系图已更新；提交范围仅为本批 GPUI 文件与验收文档。

### 用户复验项

打开原“天空为什么是蓝色的？”历史，检查首句不再显示加粗星号，末项按原文显示 `4. 结果`；拖选并复制含列表的正文，检查可读内容和编号。已有基本流式/停止验收保持历史记录，本版渲染变化的原生确认单独记录。

### 用户复验结果：通过

2026-09-10，用户对上述三项检查回复“已检查，全部正常”。对应修复提交 `09b7466b`，确认结果为：

- 中文括号旁的文字正常加粗，不再残留 `**`。
- 独立有序列表的“结果”项保留原文起始编号 `4.`。
- 含列表正文的拖选与复制正常。

本批两处排版余项及选择/复制的原生复验已通过。40 项自动测试、Windows 构建与本次用户验收分别记录；本次没有重新进行模型请求或流式/停止复验，相关基本能力沿用已有 D1 用户验收证据。完整客户端对等仍按 D2–D6 清单推进。
