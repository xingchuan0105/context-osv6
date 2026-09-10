# GPUI D1 聊天首批

## 聊天空态修正与原生检查（2026-09-09 追加）

- 输入区和消息最大宽度 760px；空态引导与输入区居中，会话中输入区靠底。历史栏补充未连接/无历史说明，顶部显示本机连接状态，连接期间给出准备服务说明，失败后允许重试。
- Windows `cargo build --features ui --locked` 通过；`cargo test --locked --lib --tests` 11 通过、1 个隔离 HTTP 测试本批未运行。复用既有测试，没有将 UI 截图结果混入测试计数。
- 本批通过 computer-use 成功打开新 exe，检查了实际空态窗口、输入框焦点、中文文本输入和点击新对话后的草稿保留。中文文本注入不代表输入法候选/组字验证。
- 没有连接真实服务或调用模型，连接失败文案与对话中布局尚未在原生窗口复验。没有重启现有服务。工作区、资料、笔记及设置未实现，D1 完整链路仍待验。
- 首次构建缺少 FluentBuilder trait，补充导入后构建通过。一次从 UNC 根目录调用 Cargo 触发不同镜像下载，已停止该构建并回到 Windows 工作区增量构建；全依赖格式化因镜像目录缺少 web-ui 未执行，改为仅格式化修改的 main.rs。

Web 基线已验收。当前 GPUI 仅夹具终态窗口，desktop-core 已完成本地会话 / SSE 抽库。本批复用这些能力，实现本地登录、个人聊天、逐段输出、停止、个人历史；不接云登录、附件、知识库或新协议，不停止 Tauri 发货。

依赖使用 ADR-0011 指定仓库锁定 rev d2304b9063b902fc7ac19b97a7cb5bf3e650b4f5 的 gpui-kit facade，透传该仓库同源 gpui/component/platform，避免独立重复 GPUI 类型。输入使用组件库 TextareaState，不自写 IME。

单 tokio worker 处理共享宿主调用，futures channel 将事件交给 GPUI。取消直接 drop 进行中的共享传输 future；本地代次挡住旧流/旧历史覆盖。contracts + web-sdk 保持唯一协议/解码/reducer。

验证门：无窗口状态测试、Windows MSVC 构建、原生窗口启动；中文 IME 与真实模型属于需要明确证据的验收，未操作不得声称通过。用户已批准本批 20–30 分钟构建与验证。本机服务不可用时给出可恢复错误，不将云 API 默认当本机服务。

## 本批结果

- 已删除夹具终态窗口，改为本地连接、个人会话侧栏、历史、composer 与逐段正文。共享宿主不改动，原 Tauri 不改动。
- 单 tokio worker + channel；个人 capabilities 为空；请求代次阻止旧流覆盖；取消直接 drop 网络 future。接近阅读底部时跟随新正文。
- Windows MSVC `cargo test --lib`：4 通过，包括停止保留部分正文、旧代次隔离、共用夹具以及等待响应头时 TCP 连接取消。
- `cargo build --features ui --locked`：通过。对齐组件仓库 Cargo.lock 的 gpui-pre 0.3.2；第一次误选更新版本的构建已主动终止，不作为证据。
- 本地可执行文件：`C:\dev\context-osv6\desktop_gpui\target\debug\desktop-gpui.exe`。
- 验证日志归档在 `desktop_gpui/target/acceptance/d1/`。代码关系图已更新。

### 原生验收尚未完成

自动审批两次拒绝启动客户端（普通启动及隐藏启动），仅返回 `blocked by policy`，未说明具体原因。未绕过审批继续启动。受控接口服务已停止；没有完成原生窗口截图、中文 IME、窗口内登录/发流/停止/历史、真实本地模型链路。4 项无窗口测试及构建不替代这些门。

`CONTEXT_OS_DESKTOP_DATA_DIR` 可显式设置隔离的开发数据目录，未设置时沿用现网 `com.contextos.desktop`；不向旧账户注入测试凭据。未启动真实本机数据栈、未调用真实模型、未部署或更改现网桌面。

下一步：用户手工启动上述 exe 后，先完成原生输入和窗口交互检查，再对真实本机服务验收；D1/M1 状态仍为待验，不推进 D2 或删除 Tauri。Markdown 富文本、完整主题与全站布局对等不属于本批已验收能力。

## 2026-09-10 用户手工验收进展

后续验收在独立 Windows 数据库和 API 18082 上完成，窗口操作由用户执行，不使用 Computer Use。完整过程见 [Windows 构建与真实链路证据](../desktop/2026-09-10-windows-big-object-build.md)。上述未验结论保留为当时记录，当前进展以本节和对等清单为准。

- 用户已确认本机连接、真实模型回答、打字机效果、立即停止并保留正文、停止后正常续聊且旧回答不再追加，以及切换会话后恢复中止片段和后续问答。
- API 真实请求与正常回答的历史读取另有日志证据；用户确认的窗口行为单独记录，不由自动化测试替代。
- 中文 IME 候选/组字仍待用户确认；客户端重启恢复与服务端模型取消尚未验证。Markdown 原文符号直接显示及历史均为“未命名对话”的问题仍待修复。
- D1 尚未整体关闭，D2–D6 未据此推进，Tauri 保留。
