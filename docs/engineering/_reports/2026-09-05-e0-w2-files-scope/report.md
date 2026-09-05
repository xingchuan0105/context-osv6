# E0 阶段：W2.4–W2.5 会话文件与 Scope 能力收尾验证报告

日期：2026-09-05
执行者：Agent (Solo Trunk)
门禁目标：Gate 0 (G0)

---

## 1. 验证摘要

- **Rust 纯模型与单元/集成测试**：
  - 命令：`CARGO_BUILD_JOBS=2 cargo test -p web-sdk -p web-ui`
  - 结果：**10 个测试套件，83 passed / 0 failed**（耗时 8.29s）
  - 覆盖项：`auth_tests`, `citations_tests`, `conversation_api_tests`, `markdown_tests`, `progress_tests`, `scope_tests`, `session_files_tests`, `sse_decoder_tests`, `chat_canvas_lifecycle_tests`, `nav_parity_tests`, `reducer_fixture_tests`, `sse_decoder_parity_tests`, `stream_benchmark_tests`, `style_baseline_guard`, `transport_adapter_tests`。
- **SSR 与 Hydrate 检查**：
  - 命令：`CARGO_BUILD_JOBS=2 cargo check -p web-server --features ssr` 及 `CARGO_BUILD_JOBS=2 cargo check -p web-ui --target wasm32-unknown-unknown --features hydrate`
  - 结果：**两端检查均通过（exit code 0）**。
- **浏览器产物构建**：
  - 命令：`export PATH="$HOME/.local/opt/wasm-bindgen-cli-0.2.127:$PATH" && CARGO_BUILD_JOBS=2 cargo leptos build`
  - 结果：成功生成 wasm hydrate 客户端与 native ssr 服务端（产出 `target/site/pkg/*` 与 `target/debug/web-server`）。
- **Playwright 浏览器旅程自动化测试**：
  - 命令：`pnpm exec playwright test chat-journey.spec.ts`
  - 结果：**16 passed / 0 failed**（耗时 16.5s）
  - 涵盖：
    - SSR HTML 表单与能力标签断言
    - Hydration 与 Storage 水合
    - 恶意 Markdown / JS 过滤
    - 引用芯片与来源卡联动
    - 进度终态折叠与推理展开
    - 会话文件上传、新会话落地与自动附带知识库能力 (W2.4)
    - 移除最后一份就绪文件后自动撤回知识库能力 (W2.4)
    - 会话文件解析中（parsing）阻塞发送按钮 (W2.4)
    - 默认能力为空及网络搜索（search）开启 (W2.5)
    - 3000+ 字流式全旅程、停止（abort）、401 报错与重试（retry）新 scope 隔离。
- **Live Backend Smoke**：
  - 命令：`LIVE_BACKEND=1 LIVE_API_BASE=http://127.0.0.1:18081 pnpm exec playwright test --config playwright.live.config.ts`
  - 结果：**2 passed / 0 failed**（耗时 5.4s），通过真实 `avrag-api` 走通 Quick Chat 与 401 对照。
- **真实文件上传链路探查与边界记录**：
  - 前端 SDK 扩展了 `resolve_upload_url`，确保在本地多端口（如后端配置 8080 但实际运行在 18081）情况下，客户端 PUT 正确直达当前 API。
  - 实测后端接口：`POST /api/v1/chat/sessions` (200)、`POST /files` (200)、`PUT /uploads/*` (200)、`POST /complete-upload` (200) 均工作正常，任务成功进入 PostgreSQL `ingestion_tasks`（queued 状态）。本地未启动 `avrag-worker` 进程，未伪造就绪态。

---

## 2. 结论

E0 阶段的全部代码与测试均已验证通过，各项核心不变量完备，符合 G0 门禁要求。
