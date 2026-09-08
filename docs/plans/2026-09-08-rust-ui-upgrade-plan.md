# Rust 前端 UI 升级：聊天首轮实施方案

日期：2026-09-08

状态：第一切片已实施，定向测试、SSR/WASM 构建和聊天浏览器回归通过；后续切片尚未实施。用户视觉验收、部署与真实 LLM 端到端验收未执行。

**同日用户意图纠偏**：本文件只保留已完成的工程记录，不再作为个人聊天知识库/会话附件行为的需求依据。用户明确个人 `/chat` 不带知识库检索，附件只用于本轮直接上下文；旧测试通过不代表这些行为符合需求。现行目标见 `docs/design/PRODUCT_IA.md`，跨页面问题与修复顺序见 `docs/design/2026-09-08-rust-product-intent-audit.md`。后续组件推广暂停，先对齐真实任务与业务结果。

## 目标与范围

以 Rust/UI 的 Leptos 组件组合方式升级 `frontend_rust`，首轮形成可用的聊天输入与操作体验，随后逐层覆盖弹窗菜单、消息阅读、工作台和设置。

视觉遵循 `docs/design/STYLE_BASELINE.md` 的 Cream × Void：暖米白与近黑、400 字重、胶囊按钮、8px 输入容器、细边线；复用现有语义 token。

业务状态仍由 ChatCanvasModel、session、reducer 和 web-sdk 承担。组件只消费状态和发出交互事件。首轮不变更产品导航、后端接口、能力选择语义和附件入库规则。

## 已核对的事实

| 位置 | 当前状态 | 改造含义 |
|---|---|---|
| `frontend_rust/crates/web-ui/Cargo.toml` | Leptos 0.8，独立 ssr/hydrate feature | 分别检查服务端与 WASM；同版本不代表组件直接兼容 |
| `frontend_rust/Cargo.toml` | cargo-leptos，style-file 指向 design-tokens.css | 现有配置未接入上游的 Tailwind 构建入口 |
| `components/ui/mod.rs` | Dialog、PageStatus、Toast | 以真实页面需要补齐基础组件 |
| `components/chat/chat_page.rs` | 输入表单内联；自动增高、手动拖高、键盘调整；发送/停止/重试三个按钮 | 提取 ChatComposer，完整迁移已有输入能力 |
| `components/chat/scope_bar.rs` | RAG/Search 多选与 ready_count 约束 | 两种能力保持可组合；不能改成互斥模式选择 |
| `components/chat/session_file_tray.rs` | 独立附件状态、重试、删除与发送阻塞 | 上传与索引状态继续由现有逻辑负责 |
| `components/ui/dialog.rs` | div 对话框与关闭按钮；该文件未见焦点锁定、Esc 和焦点恢复 | 弹层行为需要浏览器验证和补齐 |
| `components/shell/account_menu.rs` | 本地开关、遮罩按钮、主题/语言子层 | 菜单键盘和子层退出需要专项验证 |
| `tests/browser/chat-journey.spec.ts` | SSR 断言发送、停止、重试三按钮文字同时出现 | 新交互上线时同步替换该旧 UI 契约 |

代码关系图已可通过 `/home/chuan/.local/bin/code-review-graph` 访问，并查询了聊天文件摘要及 AppDialog 调用关系。后者返回零条调用边，不能据此认定组件未使用：Leptos 宏调用仍需精确源码搜索补充。当前工作区有其他后端和文档改动，本方案文件单独管理。

## 接入决策

Rust/UI 是按需复制的组件源码 registry。上游 workspace 使用 Leptos 0.8 并开启 nightly，样式使用 Tailwind；引入前逐个检查组件真实依赖、浏览器 API 与许可证。

推荐路径：保留项目 CSS/token 管线，将选中组件的样式转换为项目语义类，复用上游适合的组件结构与交互实现。Button 与输入容器先完成一个真实聊天切片，用验证结果确认该接入方式。不要为少量基础控件引入整套上游应用依赖。

若选中组件高度依赖 Tailwind 插件或共享行为库，先核对其能力与版本，再决定直接引入所需依赖或更换候选组件；不自行重写复杂焦点/定位库，不声称未经验证的适配可用。采用源码时记录上游文件、commit、MIT 许可和本地修改。

## 第一切片：聊天输入区与 Button

交付路径：进入会话 → 输入/选择能力/添加附件 → 发送 → 流式输出 → 停止或重试。

### 用户界面

- 输入区形成一个 8px 圆角容器，统一附件区、文本区和底部操作区的视觉归属；避免嵌套 form。
- 底部左侧为附件入口与能力选择，右侧为主要动作。
- 空闲状态展示发送；输出中同一位置展示停止；可重试状态展示重试入口。
- 空输入、附件处理中等禁用条件使用现有业务判断，给出可读的原因；新文案进入 i18n 目录。
- 自动增高、手动拖高和键盘调整继续可用，保留已有边界值。
- 输入法组合期间 Enter 不发送；换行与提交遵守现有明确的键盘约定。
- 长文件名截断且可查看完整名称，状态不能仅靠颜色传达；窄屏工具栏允许换行。

### 文件落点

| 文件 | 操作 |
|---|---|
| `crates/web-ui/src/components/ui/button.rs` | 新增基础按钮，按首轮实际需要提供主按钮/弱按钮与尺寸；保持原生 button 语义 |
| `crates/web-ui/src/components/ui/mod.rs` | 导出新组件 |
| `crates/web-ui/src/components/chat/chat_composer.rs` | 新增输入区组合组件，持有输入 DOM 行为，接收业务状态和回调 |
| `crates/web-ui/src/components/chat/mod.rs` | 注册输入区组件 |
| `crates/web-ui/src/components/chat/chat_page.rs` | 使用新输入区，删除被迁出的表单与相关重复实现 |
| `crates/web-ui/src/components/chat/session_file_tray.rs` | 为组合布局调整必要结构，不重复建立上传状态 |
| `assets/style/chat.css`、`components.css`、`responsive.css` | 收敛相关样式，删除被替换的旧规则 |
| `tests/browser/chat-journey.spec.ts` | 改为按状态断言动作，补充输入法与布局回归 |

组件 API 在核对实际使用处后确定，避免传入整个业务模型或预设通用框架。

### 验证门

1. 静态核对：组件职责、i18n、主题 token、删除旧路径、上游来源记录。
2. 定向 Rust 测试与样式守卫：检查现有生命周期/样式/i18n 用例，按实际命令确认集成测试被执行；`--lib` 不包含 `tests/` 集成测试。
3. SSR + hydration 构建：基于本轮源码产生新产物，jobs=2；失败则修复本切片，不进入下一波。
4. 定向 Playwright：空闲发送、流式停止、失败重试、输入法不误发、附件未就绪阻塞、能力组合、切会话后迟到事件、输入区调整高度。
5. 视觉验收：中英文、浅深主题、桌面与 390px 窄屏；检查溢出、遮挡、焦点可见和点击目标。
6. 结构变化完成后执行 code-review-graph update，再检查状态；不提交图索引。

浏览器夹具验证不代表真实 LLM 端到端验收；截图审阅与自动测试分别报告。

## 后续分层

| 波次 | 可交付结果 | 进入条件 |
|---|---|---|
| 第二切片：Dialog 与菜单 | 统一 Esc、初始焦点、焦点恢复、菜单方向键与弹层定位 | 第一切片构建、交互和视觉门通过 |
| 第三切片：消息阅读 | 消息操作、引用展开、长内容、加载与空状态统一 | 弹层行为稳定 |
| 第四切片：设置与工作台 | 表单字段、标签页、表格和状态反馈复用同一组件层 | 前三切片形成已验收的基础组件 |

每切片删除其已替换旧路径，保持产品端到端可用，不全站一次替换。

## 耗时与执行授权

用户已于本轮授权实施与定向验证；第一切片执行结果见下。无依赖安装或部署。

第一切片预计编码与适配 30–60 分钟；定向测试约 2–5 分钟，SSR/WASM 构建约 5–15 分钟，定向浏览器测试与视觉检查约 5–10 分钟。以上为估计，缓存失效或新增依赖可能显著延长构建；进入超出预估的长运行前重新说明情况。

仓库 AGENTS.md 要求编译或脚本运行前说明耗时并取得同意。实施时仅执行获批范围，验证按顺序运行，避免并行堆叠 Rust 编译。

## 上游依据

- [Rust/UI 组件目录](https://rust-ui.com/docs/components)
- [Rust/UI 源码与接入说明](https://github.com/rust-ui/ui)
- [上游 workspace 依赖与样式入口](https://github.com/rust-ui/ui/blob/main/Cargo.toml)

上游 main 可变；上述资料为本轮读取结果，正式引入源码时锁定具体 commit。

## 第一切片执行记录

### 实现

- 新增 `Button` 与 `ChatComposer`，上游锁定 `0c6b79e04a3ddc5fb9997e8cd9d9c8a6d9cbba9b`，来源和 MIT 全文见 `frontend_rust/THIRD_PARTY.md`。
- 输入区整合附件组件与能力选择；发送/停止按状态切换，可重试时提供重试。空白消息不可发送，附件处理/历史加载/生成中有状态说明。
- 提交使用同一业务回调；输入法组合事件、isComposing 和 keyCode 229 均受保护。Shift+Enter 换行，Enter 提交。
- 输入 DOM 与 resize 行为移入 ChatComposer，删除旧内联表单、重复键盘提交和按钮样式。空输入保持 96px；内容自动增高，手动调整仍限制在 72–320px。
- 胶囊按钮使用 `--radius-pill`；输入容器统一焦点提示，消除内外双框。附件名通过 title 提供完整名称。
- 无新增 crate/npm 依赖，无锁文件变更。

### 验证证据

运行目录 `frontend_rust`：

```sh
CARGO_BUILD_JOBS=2 cargo test -p web-ui --lib \
  --test style_baseline_guard --test i18n_source_guard \
  --test i18n_catalog_tests --test chat_canvas_lifecycle_tests
```

结果：45 passed（4 个 lib、34 个生命周期、3 个目录、1 个 i18n 源码、3 个样式测试）。最终焦点/圆角样式调整后，3 个 style_baseline_guard 用例再次通过。

SSR/WASM 构建成功。系统默认 wasm-bindgen CLI 0.2.114 与锁文件 0.2.127 不一致，使用本机已有匹配工具，仅对构建进程覆盖 PATH：

```sh
env PATH=/home/chuan/.local/opt/wasm-bindgen-cli-0.2.127:/home/chuan/.cargo/bin:/usr/local/bin:/usr/bin:/bin \
  CARGO_BUILD_JOBS=2 cargo leptos build
```

运行目录 `frontend_rust/tests/browser`：

```sh
pnpm exec playwright test chat-journey.spec.ts --config playwright.config.ts --max-failures=1
```

最终结果：34 passed，42.6 秒。覆盖输入法、空白禁用、调高、SSR/hydration、上传/就绪/阻塞、检索能力、工作区归属、停止/重试与迟到字节、长答案、引用与滚动等。

生成中英文 × 浅深主题 × 1280/390px 共 8 张截图，检查了桌面/窄屏代表图；自动断言输入区及发送按钮在视口内。已修复本轮发现的空输入初始高度回归和双焦点框问题。

本机运行日志：`/tmp/context-rust-ui-tests.log`、`/tmp/context-rust-ui-style.log`、`/tmp/context-rust-ui-build.log`、`/tmp/context-rust-ui-browser.log`。

### 验收边界与未解决项

- 英文通过产品账户菜单切换后显示正确；预先写入英文偏好再首次打开页面，观察到按钮仍显示中文。该变体未解决，未计入通过结论。i18n 初始化模块本轮未修改，原因和修复范围仍需单独确认。
- 截图审阅是工程视觉检查，不代表用户已接受视觉设计。
- 流式与上传使用现有 fixture 服务；无真实 LLM/线上服务端到端验收，无部署。
- 构建仍有原文件的 unused_parens 与 ts-rs serde 属性警告，本轮未扩大修复范围。
