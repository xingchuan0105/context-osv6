# Rust 前端视觉对齐复核 · 2026-09-08

基准为 `STYLE_BASELINE.md` 的 Cream × Void，以及仓库保存的 `design-md/cursor/DESIGN.md` 和 `design-md/x.ai/DESIGN.md`。上游来源为 https://getdesign.md/cursor/design-md 与 https://getdesign.md/x.ai/design-md。Rust/UI 提供组件实现参考，不另建视觉体系。

## 已修正的问题

| 问题 | 根因与修正 |
| --- | --- |
| Chat 附件按钮占满整行 | 纵向 flex 默认 stretch。改为紧凑、带回形针的胶囊按钮，保留文件选择与可访问名称；本轮上下文说明常驻，详细限制保留在按钮提示和辅助说明中。 |
| 长附件名挤压移除操作 | 文件名缺少可收缩区域。改为单行省略、完整名称提示，操作不收缩。 |
| 手机会话按钮贯穿页面 | 横向 chat shell 内按钮被纵向拉伸。手机改为纵向壳，入口靠左且按内容定宽。 |
| 新旧按钮、表单各用一套外观 | 原生控件缺少公共基础样式，旧按钮使用卡片圆角。补齐原生输入与按钮基础，所有操作按钮采用 pill，卡片和输入仍为 8px；移除点击时 translateY 跳动。 |
| 标题和表头出现粗体 | CSS 扫描未覆盖浏览器默认样式。统一 h1–h6、strong、b、th 字重为 400，使用字阶、字距建立层级。 |
| 字体名称正确但实际未加载 | 复用 Next 现有 Inter / JetBrains Mono 400 字体的 13 个 Unicode 子集，随 Rust 静态资源自托管，附 OFL 许可；补充 Linux 中文无衬线字体名称。 |
| 深色面板与 getdesign 基准偏离 | Rust 使用的 `--cos-surface`、侧栏和边线别名覆盖为透明白叠层。明暗主题统一映射 `surface-elevated` / `surface-muted` / `border`。 |
| 深色弹窗和抽屉背景发白 | 蒙层使用 `foreground`，深色时反转成白色。改为现有 `dashboard-overlay` 黑色蒙层 token。 |
| 手机分享链接、文档表格横向溢出 | 分享行可换行且链接可收缩；文档表格在自身区域滚动，长代码和内容可断行；公开知识库手机改为上下布局。 |
| 选中状态悬停后不明显 | 通用 hover 覆盖选中背景与边线。保留选中态的语义颜色；文件名等次要操作使用 ghost 样式。 |

## 验证范围与证据

- `visual-baseline.spec.ts` 从 Rust 路由表生成路径，替换为 fixture 的工作区、分享、用户参数，并补充设置页签，共 72 个入口。
- 每个入口检查 390px / 1280px × light / dark，共 288 个页面状态，检查横向溢出、实际计算字重、按钮形状。包含正常页及无效/缺参的反馈页，不等于所有业务状态都已人工验收。
- 另查四组菜单、创建弹窗、焦点返回、输入与搜索选中态，附件短按钮和超长文件名的添加/移除。
- 截图分别保存在 `/tmp/context-rust-visual/before/` 与 `/tmp/context-rust-visual/after/`。截图关闭有限时长动画以检查最终外观。
- 首轮检测出 8 个手机入口溢出；其中英文页面与对应中文页面分别计数。字体通过 Chromium 实际字体检测确认 Inter 与 Noto Sans CJK SC 正在使用。
- 构建使用本机已有 wasm-bindgen 0.2.127；未变更 Rust 依赖版本。
- 本轮检查为本地浏览器与模拟接口回归，未执行真实支付、真实模型请求或部署。用户验收地址为 `http://localhost:3100/chat`。

## 最终结果

- 最新 `cargo leptos build` 通过；Rust lib / i18n / style 11 项通过，后续 CSS 调整再次通过 style guard。
- 85 个不同的浏览器用例分批通过：首批 49 项中的 45 项通过，4 项登录旅程加载等待修正后通过；追加 36 项聊天回归。最终蒙层和悬停状态的 5 项定向复验全部通过。
- 登录自动化改为等待网络加载结束后再操作：原脚本在 SSR 字段可见、WASM 尚未绑定事件时输入，首个字段会被初始化覆盖。这是测试等待的修正，不代表登录页水合前输入保护已经实现；聊天输入区现有的水合前输入保留回归已通过。
- 截图复核覆盖聊天、工作区、设置、定价、登录、管理、公开分享、账户菜单及创建弹窗。明暗蒙层使用同一黑色语义 token；抽屉蒙层覆盖整个视口，点击空白与 Esc 均可关闭。
- 构建会清理过期的 `.br` / `.gz` 静态副本；仅复制源 CSS 不足以更新启用预压缩服务的浏览器资源。本次已重新构建后复验。
- `code-review-graph update --brief` 已更新；Windows 验收入口 `/chat` 与后端 `/ready` 均返回 200，入口可读取本轮简短附件说明与最新蒙层样式。

日志：`/tmp/context-rust-visual-build.log`、`/tmp/context-rust-visual-unit-final.log`、`/tmp/context-rust-visual-final.log`、`/tmp/context-rust-visual-close-final.log`、`/tmp/context-rust-visual-overlay-close.log`。其中早期失败保留在日志中，最终状态以针对对应问题的后续复验为准。
