# E1 任务记录：W1 验收补齐、全量路由矩阵盘点与编辑器风险分析

日期：2026-09-05
负责人：Agent / Solo Trunk
关联计划：[2026-09-05-development-execution-plan.md](2026-09-05-development-execution-plan.md) §4
门禁目标：Gate 1 (G1)

---

## 1. 任务目标与交付物

补齐 W1 阶段尚未完整落地的验收项，并在展开后续业务前完成两项关键架构前置：
1. **全量路由迁移矩阵 (`docs/design/ROUTE_MIGRATION_MATRIX.md`)**：
   - 彻底盘点 `frontend_next` 中的全部 71 个端点（69 个页面 + 2 个 route 处理器）。
   - 记录实际路径、canonical、auth 要求、渲染模式（SSR/SSG/CSR）、noindex、数据依赖、Rust 挂载状态、验收归属与阶段。
2. **凭据恢复与 SSR/Hydration 边界验收**：
   - 验证 Cookie/LocalStorage 凭据恢复、超时清理、多用户 SSR 请求状态隔离。
3. **资源交付与预压缩验证**：
   - 验证 Brotli/Gzip 预压缩生效、MIME 类型正确、哈希资源缓存头与普通静态资源缓存隔离。
4. **样式规范守卫增强**：
   - 确保 `style_baseline_guard` 覆盖 `assets/style/chat-poc.css`，字重 400、无未授权裸十六进制与阴影。
5. **Tiptap 编辑器在 Leptos/DOM 下的集成技术选型与风险结论**：
   - 调研现有 `@tiptap` 依赖（Markdown 扩展、富文本格式、快捷键）。
   - 给出 Leptos 下挂载 Tiptap 的具体架构边界，防止手写 contenteditable 带来脆弱性。

---

## 2. 详细子任务清单与状态

| 次序 | 工作项 | 状态 | 产物与证据 |
|---|---|---|---|
| E1.1 | 建立全量路由迁移矩阵 | 已完成 | `docs/design/ROUTE_MIGRATION_MATRIX.md`（71 个端点） |
| E1.2 | 样式基线守卫覆盖 `assets/style/chat-poc.css` | 已完成 | `tests/style_baseline_guard.rs` 增强并通过测试 |
| E1.3 | 资源交付实测（br/gzip、MIME、hash、缓存头） | 已完成 | Playwright 增补测试并通过，MIME/br/immutable 验证通过 |
| E1.4 | 凭据恢复与 SSR 状态隔离测试审计 | 已完成 | `crates/web-sdk/tests/auth_tests.rs` 5/5 通过，无跨请求共享 |
| E1.5 | Tiptap 富文本/Markdown 编辑器在 Rust/WASM 前端的集成方案分析 | 已完成 | `docs/engineering/2026-09-05-tiptap-editor-leptos-integration-analysis.md` (0 阻断) |
| E1.6 | 达成 G1 门禁，更新图谱并本地提交 | 进行中 | 准备提交并达成 G1 |
