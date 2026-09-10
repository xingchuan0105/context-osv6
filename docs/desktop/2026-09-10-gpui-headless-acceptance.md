# GPUI 无头交互验收

## 目的与边界

用户要求无需 Computer Use、无需逐项人工确认的验收。日常交互和布局回归由本套件自动判定；测试不操作 Windows 桌面，也不使用截图识别。用户已授权本批 25–40 分钟 Windows 构建及自动验证。

沿用当前锁定的 GPUI Kit `d2304b9` 的 `test-support`。测试在 GPUI TestPlatform 中实例化产品 `ChatApp`、Root 和真实输入/按钮组件，通过控件 ID 命中、输入和滚动事件驱动 UI；仍使用产品 `Host`、HTTP 和 SSE 实现。服务端为测试独占的 loopback HTTP 夹具，无付费模型调用。现有后端、用户档案和正在使用的原生窗口不参与测试。

框架的元素观察读取实际布局、焦点和无障碍属性，不另写一套测试状态。普通构建中的 `.test_support()` 返回原元素，运行时不保留观察层。

## 自动检查

- 服务面板往返、刷新、断开重连、历史恢复和草稿保留；个人历史不混入工作区会话。
- 连接进行中，重复点击连接、启动和刷新不产生重复登录。
- API 健康失败显示重试入口，保留输入，恢复后能连接。
- 通过发送/停止控件验证首段显示、取消保留已显示内容、下一轮可完成。
- 中文文本编辑和从面板返回输入焦点。
- 历史加载时发送按钮不可触发请求，新对话解除加载状态。
- 明暗主题 × 1440×900、1280×720、768×600、640×480 × 空态/长回答，检查输入与发送不越出窗口、正文滚动不带走发送区。包含长代码和表格内容。
- 服务面板在明暗主题 × 1280×720、768×600、640×480 下，操作按钮滚动可达，返回入口固定可见；短屏仍可断开重连。
- 连接仍在进行时触发 GPUI 关闭事件，等待连接结束与清理完成，迟到的登录结果不能恢复 token，独立 API 保持健康。此项为 TestPlatform 关闭回调与真实 Host 的集成，不等同于受管产品进程清理。

## 运行入口

Windows 源码镜像仍使用 `desktop_gpui/scripts/sync-windows.ps1`。从仓库运行：

```powershell
pwsh -NoProfile -File desktop_gpui/scripts/accept-headless.ps1
# 每项使用 3 个种子重复运行，失败不会自动重试掩盖：
pwsh -NoProfile -File desktop_gpui/scripts/accept-headless.ps1 -Iterations 3
# 同时运行既有共享 Tauri 宿主套件：
pwsh -NoProfile -File desktop_gpui/scripts/accept-tauri-shared.ps1 -WithHttpFixture -WithHeadlessUi
```

`accept-headless.ps1` 使用独立档案目录和测试端口，固定 jobs=2、单测试线程。端口被占用会失败，不终止占用者。每次在 `C:\dev\context-osv6\desktop_gpui\target\acceptance\headless\gpui-ui-<uuid>\` 写出 `tests.log` 与 `result.json`；编译失败、零用例、失败或跳过用例均不能判为通过。

## 不覆盖

本套件验证真实 GPUI 组件的事件与几何布局，但不比较显卡输出像素，不操作系统输入法候选窗或文件管理器。输入中文文字不代表系统 IME 已验证。受管环境的产品冷启动、真实迁移、API/worker/PG/Redis 清理仍由独立进程 E2E 覆盖；HTTP 夹具不替代该门。

## 2026-09-10 执行结果

| 检查 | 结果与证据 |
|---|---|
| 统一入口 `accept-tauri-shared.ps1 -WithHttpFixture -WithHeadlessUi` | 54 项通过：desktop-core 24 + GPUI lib 16 + 原共享流 4 + 隔离 HTTP 1 + 无头 UI 9 |
| 新无头 UI 套件 | 9 通过、0 失败、0 跳过；包含构建 8.93 秒，日志目录 `gpui-ui-5fc31d377b874ebc80a46af1d519d24f` |
| 夹具稳定性复验 `-Iterations 3` | 9 个具名用例各运行 3 次，共 27 次实例通过；14.35 秒，日志目录 `gpui-ui-c4a4681211684eb99b54de395ea42804`。不重复计入 54 项 |
| 常规产品 UI 编译检查 | `cargo check --locked --features ui` 通过，1.49 秒；`headless/normal-ui-check.log`。测试程序已构建运行，未替换正在使用的 GPUI 程序 |
| 环境 | API PID 37708、GPUI PID 46220 保持运行；未重启后端、调用模型、操作桌面或要求人工点验 |

上述日志目录位于 `C:\dev\context-osv6\desktop_gpui\target\acceptance\headless\`。每份 `result.json` 包含源文件 SHA-256、命令、测试次数、耗时和覆盖边界。共享 lib 默认忽略的 HTTP 旅程已单独执行；另一项指向现有 API 18082 的只读探测没有在本批重复执行。

首次失败暴露了夹具适配问题：补齐 SSE done 契约、允许 GPUI 调度器接收真实外部 I/O、等待 Markdown 异步排版，并显式将 Windows accepted socket 设回阻塞模式。失败记录保留，未用跳过或失败重试改写结果。

当前 Kit 的 disabled 按钮会阻断事件，但未向元素快照提供 `aria-disabled`。测试通过点击后状态与实际请求计数验证禁用行为，不将属性缺失当作通过的辅助功能证明。

本批关闭上述 UI 交互/布局的自动回归门；不要求用户逐项重复点击。D2 受管冷启动、真实迁移与退出清理的进程 E2E 仍待实施，D3 未推进。
