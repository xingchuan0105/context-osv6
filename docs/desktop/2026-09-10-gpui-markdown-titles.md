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

待用户检查历史回答的标题、粗体和列表，侧栏名称与选中状态，切换后的内容恢复，以及新版本流式与停止行为。自动测试不替代这些视觉与交互确认。
