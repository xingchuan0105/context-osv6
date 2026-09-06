# D0.2 任务记录:本地数据栈/产品生命周期/本地会话抽库 (desktop/core)

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-05 |
| 负责人 | Agent / Solo Trunk |
| 关联计划 | [`2026-09-05-development-execution-plan.md`](2026-09-05-development-execution-plan.md) §7 GPUI |
| 门禁目标 | **D0.2 完成**(宿主抽库第一波;D0.3–D0.4 未开始) |

---

## 1. 任务目标与交付范围

按权威计划 §7 D0.2(“local session、PG/Redis、本机产品与生命周期抽库”),把 Tauri 宿主中的本地栈管理逻辑抽入 `desktop/core`(宿主无关,无 tauri/gpui 依赖):

1. **搬移模块**(净迁移,行为不变):
   - `win_cmd.rs`(Windows 控制台抑制 / kill tree / scoped kill — 防误杀非本应用进程的路径级过滤)
   - `secret_fs.rs`(unix 0600 / Windows DACL)
   - `docker_status.rs`(docker 探测 + 平台安装引导;Windows 打包端不探测)
   - `native_stack.rs`(native PG+Redis ensure/stop:initdb、角色供给 avrag_cluster_admin/avrag/avrag_runtime、pgvector、stale pidfile 清理、client.env 生成)
   - `local_stack.rs`(栈状态探测、runtime config 脱敏、ensure 90s 超时、非 Windows bash 回退)
   - `local_product.rs`(avrag-migrate 迁移 → api/worker spawn、pidfile、健康等待、停止、bash 回退)
   - `local_session.rs`(本地 B2C 凭据文件 + 登录/注册 + 会话持久化)
   - `lifecycle.rs`(退出收摊:产品 → 数据面 → scoped sweep)
2. **宿主注入点**(取代对 Tauri 内部模块的依赖):
   - `device_id: Option<&str>`(host 的 license machine-id;None 回退 `cos-local-device`)
   - `relay_env: Option<String>`(云中继块,host 由 cloud session 渲染;None = BYOK-only)
   - `data_dir: &Path`(本地会话文件目录,host 由 AppHandle 解析)
3. **`HostError`**(原 IpcApiError 平台中立形态):status/code/message;Tauri `IpcApiError` 通过 `From<HostError>` 转换。
4. **Tauri 侧收敛为薄包装**(`commands/local_host.rs`,~190 行):`#[tauri::command]` 委托 core + AppHandle 参数绑定;`product_api_base_url` / `log_dir_path` / `local_session_token` 经包装层再导出,其余消费方(api/chat_stream/documents/publish/system/cloud_session)仅改引用路径。
5. **D0 开工要求**:创建 `docs/desktop/GPUI_PARITY_CHECKLIST.md`(D0.1–D6 全能力条目,状态/证据占位)。
6. **不变量**:
   - 不误停非本应用进程:stop/sweep 仍以 pidfile + 安装/状态树路径过滤(行为未改动,仅迁移)。
   - 迁移唯一路径仍是 avrag-migrate sidecar;DATABASE_URL 仍指向 DML-only 角色,migration DSN 走 owner 角色。
   - 云会话抽库属 D0.4:core 的中继块由 host 注入,`cloud_session::load_session_standalone` 留在 Tauri。

## 2. 执行步骤

| 次序 | 工作项 | 状态 | 产物与证据 |
|---|---|---|---|
| D0.2.1 | 盘点 8 个命令模块的耦合面(全部纯逻辑;仅 device_id/relay_env/data_dir 三处宿主依赖) | 已完成 | 见 §1 注入点 |
| D0.2.2 | core 新模块搬移 + 机械替换(super::→crate::、IpcApiError→HostError)+ 签名参数化 | 已完成 | desktop/core/src 10 模块 |
| D0.2.3 | core lib.rs 再导出 + Cargo 依赖(base64/uuid/url/windows-sys/tokio time) | 已完成 | core 零警告 |
| D0.2.4 | Tauri `commands/local_host.rs` 薄包装 + mod.rs 收敛 + lib.rs Exit 挂 core lifecycle | 已完成 | src-tauri check 通过 |
| D0.2.5 | 消费方引用更新(api/chat_stream/documents/publish/system/cloud_session) | 已完成 | — |
| D0.2.6 | `docs/desktop/GPUI_PARITY_CHECKLIST.md` 建立 | 已完成 | D0.2a–i 标记完成 |
| D0.2.7 | 验证收敛、任务文档、提交 | 已完成 | src-tauri 29 tests / core 16 tests 全绿 |

## 3. 验证证据

- `cargo check`(desktop/core):exit 0,零警告
- `cargo test`(desktop/core):**16 passed / 0 failed**(native_stack 4 + local_stack 3 + local_product 2 + local_session 2 + docker_status 3 + chat_stream 既有)
- `cargo check`(desktop/src-tauri):exit 0(遗留 4 条既有警告:license `types::*`、api `not_implemented` ×2、ts-rs serde attr,均非本切片引入)
- `cargo test`(desktop/src-tauri):**29 passed / 0 failed**
- Tauri 调库回归:所有命令路径经薄包装仍指向 core 实现(编译期保证);启动/迁移/登录/退出真机点验留待 Windows(GD1 前置点验,清单已挂)

## 4. 剩余问题

- Windows 真机点验(启动/迁移/登录/退出 + 不误停)挂 `GPUI_PARITY_CHECKLIST.md`,待 GD1 前窗口执行。
- D0.3(documents/REST/上传/目录接口抽库)、D0.4(cloud session/Publish/深链抽库)未开始。
- cloud_session(758 行)仍含 IPC 序列化与文件持久化;D0.4 抽库时一并参数化。

## 5. 图谱状态

`code-review-graph update` 已执行。
