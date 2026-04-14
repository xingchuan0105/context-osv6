# V6 Frontend Dev Plan & Orchestration

本计划旨在将 `context-osv6` 的前端从 React/Next.js 彻底迁移到 Rust (Leptos) 架构，并实现“Session 优先”的三栏布局优化。

## Phase 1: 基础设施与架构奠基 (Week 1)
- [ ] **T1.1: WASM 编译链优化**
  - 配置 `Cargo.toml` 生产环境优化参数 (LTO, codegen-units)。
  - 集成 `wasm-opt` 到构建流程。
- [ ] **T1.2: 类型安全共享层**
  - 在 `crates/common` 中定义 100% 兼容后端的 DTO。
  - 创建 `web-sdk` 包装层，统一处理 Fetch/SSE 的 Rust 类型转换。
- [ ] **T1.3: 离线存储适配器**
  - 基于 `IndexedDB` 实现本地存储抽象层，支持笔记与 Session 草稿。

## Phase 2: “Session 优先”三栏布局实现 (Week 2)
- [ ] **T2.1: 全局 Shell 迁移**
  - 实现左侧 Session 历史导航栏（支持 Session 独立上下文）。
  - 实现右侧“上下堆叠”资源面板（Sources + Notes）。
- [ ] **T2.2: 交互式中栏 (Chat)**
  - 实现流式对话展示。
  - 实现 V5 风格的轻量化 Citation 小标，并支持点击后右侧内容源高亮。
- [ ] **T2.3: Resize Handle 系统**
  - 实现左/中、中/右、右侧内部（Source/Note）的三向可调布局。

## Phase 3: 生产力转化模块 (Week 3)
- [ ] **T3.1: 浮动富文本编辑器**
  - 集成 Rust 友好的富文本组件，支持从聊天气泡提取内容。
  - 实现“保存到笔记”与“导出为 .md 内容源”的双向逻辑。
- [ ] **T3.2: 智能 @Note 指令**
  - 扩展聊天输入框，支持 `@笔记` 快速唤起编辑器。
- [ ] **T3.3: 异步同步系统**
  - 实现笔记的乐观更新与后台重试逻辑。

## Phase 4: 后台管理与全局补齐 (Week 4)
- [ ] **T4.1: 重构 Admin 面板**
  - 按照“组织中心”模型，实现租户/用量/健康监控。
- [ ] **T4.2: 分享与分析页面**
  - 实现完整的分享管理、访问日志与统计。
- [ ] **T4.3: 性能调优与 E2E 验证**
  - 针对 WASM 进行体积分析，实现动态加载。
  - 运行 Playwright 测试套件确保迁移无功能回归。
