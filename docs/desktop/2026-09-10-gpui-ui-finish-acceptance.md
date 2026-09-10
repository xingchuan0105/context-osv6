# GPUI 界面收口与自动验收

日期：2026-09-10。范围：[本批计划](../plans/2026-09-10-gpui-ui-finish.md)。用户已授权修改后自动验证，禁止 Computer Use、付费模型调用及重启既有后端。

## 实现结果

本批将已接通功能的原生界面统一到已确认的布局规范：个人聊天、工作区总览、工作区聊天及资料/笔记、服务面板。删除 `main.rs` 中旧的整页渲染，分为共享外观、公共壳、聊天、知识和服务视图；HTTP、会话 reducer、宿主生命周期继续复用原模块。

- 导航展开 248px、折叠 60px，折叠保留图标和短文字；第二入口固定为工作区总览。低于 768px 使用原生模态导航；桌面折叠及明暗主题偏好本机保存。
- 52px 上下文页头；窄屏工作区操作独占第二行，避免与标题/服务入口挤压。资料与笔记可直达，添加资料仍直接选择文件。
- 聊天空态聚合引导与输入卡片；对话正文独立滚动，最大阅读宽度 760px；输入区为一个文本框和一条工具栏。助手 Markdown 与复制、用户消息、错误/停止状态各有层级。
- 工作区总览使用创建卡片和对象列表。资料宽屏 336px 并排，1200px 以下为 Kit 原生抽屉；保留上传失败恢复、选择范围、原文和笔记草稿。原生层提供 Esc、焦点管理和点击遮挡。
- 服务页使用状态卡片和操作区；在壳中只有一个服务入口。个人聊天没有知识库按钮；个人附件、云登录和其他 D4/D5 功能仍以对等清单为准。

## 自动验证

Windows MSVC，所有 Cargo 运行 `jobs=2`，无头 UI 单线程。最终命令为：

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/sync-windows.ps1
pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/accept-tauri-shared.ps1 -WithHttpFixture -WithHeadlessUi
pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/render-previews.ps1
pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/build-acceptance.ps1
```

| 层 | 最终结果 | 证据 |
|---|---|---|
| Tauri 共享 desktop-core | 30 通过 | `tauri-shared/shared-core.log` |
| GPUI lib 与原流协议用例 | 17 + 4 通过 | `tauri-shared/gpui-and-original-stream.log` |
| Host 本机会话/历史 HTTP 夹具 | 1 通过 | `tauri-shared/local-session-history.log` |
| GPUI TestPlatform 真实视图与事件 | 26 通过，0 失败/忽略 | `headless/gpui-ui-5999907db3284c01b2e094e62c2cf65a/result.json` |
| 原生 DirectX 隐藏窗口渲染 | 24 PNG 生成并通过尺寸/完整性检查 | `visual/gpui-ui-2f9a2c54c2d341cdb9cba5804f99f3e8/visual-result.json` |
| 独立 Windows 生产特性构建 | `ui` 特性通过，74.23 秒 | `build/desktop-gpui-acceptance-20260910-201230/result.json` |

合计 **78 项自动测试通过**。表内相对证据均位于 `C:/dev/context-osv6/desktop_gpui/target/acceptance/`。完整入口日志为 `C:/dev/gpui-ui-finish-shared-final.log`。默认测试列表的 6 个 opt-in 用例中，HTTP 夹具已单独运行通过；其余 5 个真实服务/数据/模型用例本批未运行，不能将它们计为通过。

5 条新 UI 用例覆盖：导航/主题偏好与草稿作用域；手机导航 Tab/Esc/背景发送遮挡；空态输入区聚合和工具栏；总览/服务页窄屏操作可达；资料/笔记切换、调整窗口和 Esc 保留笔记。既有 21 条聊天、停止、历史、上传、笔记、引用、服务退出等用例全部保留。

`layout-matrix.jsonl` 保存 24 组 TestPlatform 几何记录，覆盖空态/总览/服务页 × 1440×900、1280×720、768×600、390×720 × 明暗主题；另有既有聊天布局和本批资料抽屉旅程。几何记录不代表 GPU 像素。

## 原生截图与复核

截图复用实际 `ChatApp`、`Root`、Kit 资源与 Windows DirectX 渲染，通过一个 `show:false / focus:false` 的窗口渲染目标读回生成。没有显示窗口、模拟桌面输入或截取桌面；数据仅来自进程内合成夹具。捕获代码仅编入 `headless-tests` 产物，不进入交付的 `ui` 特性程序。

24 张图为以下六场景 × 1280×800 / 390×720 × 明暗主题。当前 Windows 缩放 175%，实际 PNG 为 2240×1400 / 683×1260。

| 场景 | 已逐图复核的桌面图 | 已逐图复核的窄屏图 |
|---|---|---|
| 聊天空态 | `empty-1280-light.png` | `empty-390-dark.png` |
| 对话中 | `chat-1280-light.png` | `chat-390-dark.png` |
| 工作区总览 | `workspaces-1280-dark.png` | `workspaces-390-light.png` |
| 工作区资料 | `workspace-documents-1280-dark.png` | `workspace-documents-390-light.png` |
| 工作区笔记 | `workspace-notes-1280-light.png` | `workspace-notes-390-light.png` |
| 本机服务 | `services-1280-light.png` | `services-390-dark.png` |

上述 12 张为本轮代理逐图复核，不是用户人工验收，也不是全图逐像素比对。修正手机标题孤字换行、复制操作居中、资料抽屉重复标题、状态标签整行拉伸后生成最终截图；未见操作区遮挡或横向裁切。仍有一项非阻断排版细节：`chat-390-dark.png` 的中文列表句号偶尔单独换行，未通过改写原文隐藏问题，后续中文断行处理继续跟踪。

## 失败记录与修复

- 首轮编译发现测试观察包装使返回类型不同，渲染方法改为准确的 `impl IntoElement + use<>`。
- 初轮无头 24/26、随后 25/26：恢复按钮中心位于可视区外、抽屉动画未结束即断言。用例先滚动并断言按钮可达，再点击；抽屉使用有界帧等待，保留最终边界断言。修改后 26/26，最终收口复验仍 26/26。
- 首次隐藏窗口捕获产生 1×1 图：Windows 隐藏窗口初始 placement 未分配目标，且关闭最后一个窗口会结束事件循环。改为一个隐藏窗口复用、显式隐藏 resize，并检查读回尺寸；最终 24 图全部合格。
- 从 UNC 目录启动 Cargo 曾继承父级 registry 配置，触发冗余构建；只中止了经身份核实的本批构建进程，随后构建脚本显式进入 Windows 工程目录。既有服务及运行程序未中断。

失败日志保留于原运行目录；最终通过结果不覆盖先前失败记录。

## 交付与边界

独立 Windows 程序：`C:/dev/context-osv6/desktop_gpui/target/debug/desktop-gpui-acceptance-20260910-201230.exe`，SHA-256 `AFC1FF11778A7C68F170ADCC7774C8D46AE850AD015899CA76267D4B61385430`。构建回执记录全部源码哈希，`features=ui`、`developmentBinaryUnchanged=true`。它使用实际源码与锁文件，仅改变临时 manifest 的绝对路径和二进制名称；不覆盖或启动已有客户端。共享日志另存于本次 `target/acceptance/ui-finish-20260910-201230/`，避免后续运行覆盖证据。

代码关系图在结构修改后执行 `code-review-graph update --repo /home/chuan/context-osv6 --base HEAD --brief` 成功，日志 `C:/dev/gpui-ui-finish-graph.log`；图包含工作树其他任务，风险汇总不属于本批测试数。`.code-review-graph/` 不提交。

本批实施和自动交互门完成，原生截图已有独立证据；用户对本次视觉未逐项确认。未复验真实模型/办公入库、系统 IME 候选、系统文件管理器、macOS/Linux 或安装升级；此前通过记录继续保留在 [对等清单](GPUI_PARITY_CHECKLIST.md)。本批不代表全量 Tauri 对等或可发布安装包，下一功能批为 D4。
