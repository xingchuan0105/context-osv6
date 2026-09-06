# D0.3 任务记录:documents/REST 代理/上传安全边界抽库 (desktop/core)

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-05 |
| 负责人 | Agent / Solo Trunk |
| 关联计划 | [`2026-09-05-development-execution-plan.md`](2026-09-05-development-execution-plan.md) §7 GPUI |
| 门禁目标 | **D0.3 完成**(宿主抽库第二波;D0.4 未开始) |

---

## 1. 任务目标与交付范围

按权威计划 §7 D0.3(“documents、REST、上传、本地目录接口抽库”):

1. **`core/src/api_proxy.rs`**(自 Tauri `commands/api.rs` 代理部分迁入,HostError 化):
   - `api_call`:本机产品 REST 代理(GET/POST/PUT/PATCH/DELETE、Bearer、publish/export 长超时 180s + Accept-Encoding zstd、普通调用 60s)
   - `upload_bytes` + `assert_desktop_upload_url`:上传安全边界——**仅 loopback + `/uploads/` 路径 + 端口与本机 API 一致**的签名 URL 可接收原始字节(防宿主被诱导上传到任意远端)
   - zstd 导出解码(128MiB 上限,413 拒绝)、路径归一化
   - 7 个单测(上传 URL 拒绝远端/错路径/错端口、zstd 往返、非导出透传、路径断言)
2. **`core/src/documents.rs`**:`reindex_local_documents(token)`(列表 dual-shape 解析 + 逐文档重索引 + 错误聚合),1 单测
3. **Tauri 侧收敛**:`api.rs` 保留 `IpcApiError`(补回 `not_found`,license 在用)+ `From<HostError>` + 命令薄包装;`documents.rs` 薄包装(token 由 `local_session_token` 解析);`publish.rs` 的 `api_call` 引用指向 core
4. **范围决策**:Tauri `backend.rs`(common::Document → IPC 载荷塑形)**不迁**——desktop/core 工具链为 1.94 而 `common` crate 要求 1.96,且该塑形绑定 IPC JSON 形状;`cache.rs`/`chat.rs` 为 State/事件绑定薄层,不在 D0.3 清单
5. **行为不变量**:上传范围(拒绝远端/错路径/错端口)、失败恢复(不可达 → 503 service_unavailable、上游错误体透出)、本地服务不可达消息均原样保留

## 2. 执行步骤

| 次序 | 工作项 | 状态 | 产物与证据 |
|---|---|---|---|
| D0.3.1 | 盘点 api/documents/backend/cache/chat 耦合面 | 已完成 | 全部纯逻辑;cache/chat 为薄 IPC 层不迁 |
| D0.3.2 | `core/src/api_proxy.rs`(HostError 化 + 7 单测) | 已完成 | — |
| D0.3.3 | `core/src/documents.rs`(token 参数化 + 1 单测) | 已完成 | — |
| D0.3.4 | Tauri 薄包装:IpcApiError/From + api_call/upload_bytes/reindex 委托 | 已完成 | — |
| D0.3.5 | checklist D0.3a–c 标记完成、任务文档、提交 | 已完成 | 提交号见 git log |

## 3. 验证证据

- `cargo check`(desktop/core):exit 0、零警告;`cargo test`:**19+4 passed**(api_proxy 7 / documents 1 / 既有 15)
- `cargo check`(desktop/src-tauri):exit 0(仅遗留 license/types 与 api not_implemented 既有警告);`cargo test`:**21 passed / 0 failed**
- 行为验收(检查清单 D0.3c):上传范围/失败恢复/不可达映射均有单测锁定

## 4. 剩余问题

- D0.4(cloud session/Publish/深链抽库)未开始;`render_relay_env` 已就位,cloud session 文件层留待 D0.4 参数化
- Windows 真机点验挂 `GPUI_PARITY_CHECKLIST.md`

## 5. 图谱状态

`code-review-graph update` 已执行。
