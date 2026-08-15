# Windows 沙箱 bridge 移植（TCP transport + Job Object）交接

| 字段 | 内容 |
|------|------|
| **日期** | 2026-08-15 |
| **状态** | **已闭环**（2026-08-15 第二 session 完成 §2 全部剩余工作；闭环记录见 §4；代码已落地，待提交） |
| **上游** | `2026-08-14-windows-desktop-client-acceptance-handoff.md` §4 的修正诊断 + 用户决策「做方案 2（原生移植）」 |
| **改动** | `avrag-rs/crates/code-interpreter/{Cargo.toml, src/bridge.rs, src/lib.rs}`（3 文件，+683/−199） |

## 0. 背景与根因（修正 08-14 交接文档 §4）

08-14 交接文档把 D-rag-full 失败归因于 knockout 剔除——**该假设已证伪**。真机日志（`cos-e2e-20260814-035418-22260/state/logs/api.log`）时间线：

1. wave-1 short_sac（4717ms）窗口内**零条** `sandbox retrieval bridge call` —— worker 一条检索都没发出；
2. wave-2 re-brief host leaf 的 9 条 `bm25_terms` 日志 = 1 次整查询（0 命中）+ 8 次 `per_term_hit_counts` 逐词探测（3 词单独命中）——是 AND 语义 0 命中的 hint 探测，不是命中被丢弃；
3. knockout 假设不成立：`KNOCKOUT_HARD_SUPPRESS` 是编译期 `false`（`agent-loop/src/helpers/knockout.rs:24`），false 时 `align_value_no_count`/`apply_to_bridge_data` 直接 no-op（`:179-182`）。

真正根因：**Windows 构建里沙箱 bridge 是编译期存根**（`#[cfg(not(unix))]` 直接返回 `Err(Bridge("requires a Unix platform"))`）。Linux 上全绿是因为走真实 fd 实现；桌面端每次 SaC codegen 必败 → 零检索 → `n_hits=0`。次要根因：re-brief host leaf 的 `lexical_terms_from_query` 把整句英文按空格拆词全 AND，自然语言问题必然 0 命中（`run_lead_workers.rs:1416`；另注意 `run_rag_worker_host:942` dense 分支误写 `doc_ids` 字段，应为 `doc_scope`，未触发是死代码 bug）。

## 1. 已完成（本 session，未提交）

### 1.1 架构重构（`bridge.rs`）

| 部分 | 内容 |
|---|---|
| **共享层 `mod shared`** | `wait_child` / `parse_child_output` / `run_bridge_pump_sync<R,W>`（泛型化 Read/Write）/ `bridge_pump_runtime` 抽出，unix 与 windows 共用同一 line-JSON pump 与并发 worker 模型 |
| **`SandboxOpts`** | `build_bridge_sandbox_wrapper` 参数化：`prelude_imports`（hook 前预导入，Windows 传 `import socket`——`socket` 在 BLOCKED 表里）+ `transport_setup`（Python 片段，赋值 `_bridge_transport = {"req":…, "resp":…}`） |
| **shim 去缓存** | `bridge_shim_source` 每次 build（曾用 LazyLock 缓存导致 Windows 第二次执行连旧端口——每 run port/token 不同，必须重建；unix fd 是常量无感） |
| **unix_impl** | 逻辑不变，仅改为消费 shared 层 + `SandboxOpts`（fd3/4 pre_exec 语义原样） |

### 1.2 Windows 实现（`mod windows_impl`）

- **传输**：host `TcpListener::bind(("127.0.0.1",0))` → port + 32hex 随机 token 以字面量注入 shim；Python `socket.create_connection` 后先发 token 行，host 校验不匹配即断连；socket 半 dup 后喂泛型 pump。
- **Python 发现**（`resolve_python_path`）：`AVRAG_SANDBOX_PYTHON` env → exe 旁 `python\python.exe`（捆绑）→ PATH 探测（`--version` 真跑验证，防 WindowsApps 2 字节 Store 存根——该存根静默 exit 49）。
- **Job Object**：`KILL_ON_JOB_CLOSE | PROCESS_TIME | PROCESS_MEMORY` + PerProcessUserTimeLimit/ProcessMemoryLimit，超时 `TerminateJobObject`。
- **降级路径**：`AssignProcessToJobObject` 失败（本机 360 实测 ACCESS_DENIED，CREATE_SUSPENDED 也一样）→ `tracing::warn` + 超时改 per-pid `TerminateProcess`（`terminate_process_by_pid`），墙钟超时不受影响。
- **非 bridge `execute()`**（lib.rs）：Windows 也创建 Job Object（`bridge::job_object_for_child`）+ 超时 terminate；默认 python 改 `default_python_path()`（win=`python`，unix=`python3`）。
- Cargo：`[target.'cfg(windows)'.dependencies] windows-sys 0.59`，features `Win32_Foundation, Win32_Security, Win32_System_JobObjects, Win32_System_Threading`。

### 1.3 测试

- 新增平台无关 `mod bridge_interop_tests`（`bridge_rpc_roundtrip_across_transport`：client.save RPC 往返断言 stdout 含 echo；`bridge_blocks_os_import_on_all_transports`：安全钩子跨传输一致）。Python 解析走生产同款发现逻辑。
- **Linux**：`cargo test -p avrag-code-interpreter --lib` → **18 passed / 1 ignored**（ignored 为手动孤儿进程检查，`-- --ignored` 单跑也 ok）；`cargo test -p agent-loop --lib` → **426 passed**。
- **Windows 真机**（交叉编译 test exe 拷到 `C:\Users\xingc\AppData\Local\Temp\bridge_test.exe`，`AVRAG_SANDBOX_PYTHON=C:\Users\xingc\AppData\Local\Temp\pybundle\python.exe`，python-3.12.10-embed-amd64）：**bridge interop 2/2 绿**（TCP+token+真 Python 完整链路）；随后全量 11 个测试 **4 passed / 7 failed**。

### 1.4 已坐实的机器事实

- **验收机无系统 Python**：`WindowsApps/python.exe` 是 2 字节存根；`Program Files` 无安装。已下载 embeddable 解压在 `C:\Users\xingc\AppData\Local\Temp\pybundle\`（来源 `/tmp/pyembed.zip`，python.org 3.12.10 embed amd64，含 asyncio/socket/threading）。
- **360 安全软件**（`C:\$360Honeypot` 等）阻断 `AssignProcessToJobObject`（err 5），spawn/`CreateJobObjectW`/`SetInformationJobObject`/`OpenProcess`/TCP 全部正常——winprobe.exe（`/tmp/winprobe`）逐项验证过。
- WSL→Windows exe 输出不回显，用 `Out-File -Encoding utf8 <文件>` + `/mnt/c/...` 读回。

## 2. 剩余工作（按顺序）

1. **修 7 个 Windows 测试失败**：全是非 bridge `execute()` 路径（`lib.rs` `tests::test_simple_expression` 等）——`execute()` spawn `default_python_path()`（win=`python`）但**没有走 `resolve_python_path` 发现**，PATH 上只有 Store 存根。修法：`CodeInterpreter::execute` 在 windows 上先 `bridge::resolve_python_path(&self.python_path)`（发现失败给出可操作错误信息），预计一并让 `job_object_for_child` 的 warn 路径可见。修完 `--test-threads=1` 全量应 11/11。
2. **Python 捆绑进安装树**：`stage-desktop-sidecars.sh` 增加：下载/复制 embeddable 包到 `desktop/runtime/bin/python/`（`python.exe` 与 DLL 平铺，**保持 `python312._pth` 原样**——import site 注释掉没关系，asyncio 在 zip 内）。注意 NSIS 打包要包含该目录；`resolve_python_path` 的 `exe 旁 python\python.exe` 探测已就位，无需代码改动。版本 pin 3.12.10 embed amd64，校验 sha256。
3. **force 交叉编译 + 真机 D-rag-full**（记住 08-14 §5 教训，`ensure_built` 不重编）：
   ```bash
   cd avrag-rs && export USERPROFILE="$HOME/.cache/context-osv6/win-userprofile"
   cargo build --release --target x86_64-pc-windows-gnu -p avrag-api -p avrag-worker
   bash scripts/stage-desktop-sidecars.sh   # STAGE_TARGET_TRIPLE=x86_64-pc-windows-gnu
   # 手动拷 desktop/runtime/bin/{avrag-api.exe,python/} 到 %LOCALAPPDATA%\Context-OS Client\
   DESKTOP_E2E_YES=1 DESKTOP_E2E_LLM=1 DESKTOP_E2E_GREP='D-rag-full' \
     DESKTOP_E2E_WIN_FRONTEND='C:\dev\context-osv6\frontend_next' bash scripts/desktop-e2e/run.sh l1
   ```
   预期：wave-1 short_sac 出现 `sandbox retrieval bridge call` 日志、n_hits>0、citation 断言绿。
4. **收尾**：`cargo test -p avrag-code-interpreter --lib`（Linux）+ agent-loop 回归双绿后提交；`code-review-graph update`；把本文件的 §0 修正回写 08-14 交接文档 §4（避免后人沿 knockout 死线索走）；提交信息建议 `code-interpreter: windows sandbox bridge (TCP transport + job object containment)`。
5. **（可选后续）** wave-2 host leaf 的 lexical AND 语义清理 + `run_rag_worker_host` dense `doc_ids`→`doc_scope` 字段修正——沙箱移植后 wave-2 仅在 re-brief 触发，优先级降但仍是真 bug。

## 3. 关键机制备忘（接手人必读）

- **transport_setup 每次执行都要重新生成**（port/token 每 run 不同）；shim 其余部分（RPC 多路复用、reader 线程、`asyncio.to_thread`）与 unix 完全同一份源码。
- **`socket`/`sys` 等 BLOCKED 模块的预导入必须在 hook 安装前**（wrapper 的 `prelude_imports` 位点），hook 安装后用户代码 import 才会被拦——interop 的 blocked-os-import 测试守护这一点。
- **Job assign 失败是预期路径**（360 环境），不要把它当错误重试或 fail-fast；`terminate_process_by_pid` 只杀 python 本体，python 的子进程不在覆盖面内（当前沙箱 BLOCKED 了 subprocess，风险可控）。
- **embeddable 包无 pip**：SaC 沙箱只用标准库（asyncio/json/threading/socket），不需要 pip；不要往 bundle 塞 site-packages。
- 交叉编译要 `USERPROFILE` 指到 WSL 侧可写目录（脚本里已有）。

## 4. 闭环记录（2026-08-15 第二 session）

§2 全部完成，D-rag-full 真机跑绿。新增事实：

1. **§2.1 已修**：`lib.rs` `execute()` Windows 先走 `bridge::resolve_python_path`（`bridge.rs` 新增 `pub(crate)` re-export）。真机全量 **11/11 绿**（终码复验一次）。
2. **交接外编译错（Windows-only）**：`windows_impl::WindowsSysHandle(*mut c_void)` 不 Send，`execute_with_bridge` 跨 `.await` 持有 `JobObject` 导致 app-chat release 交叉编译 E0277（Linux 不可见——cfg(windows) 不参与；之前只交叉编过 code-interpreter test exe 所以没暴露）。修法：`unsafe impl Send for WindowsSysHandle`（Win32 HANDLE 无线程亲和性，与 std 对 RawHandle 的 Send 实现同理）。
3. **§2.2 已做**：`stage-desktop-sidecars.sh` 增加 embeddable 捆绑（pin 3.12.10 + sha256 `4acbed…25a3c3`，与 python.org content-length 11133606 交叉验证；`PYTHON_EMBED_ZIP` 可覆盖；缓存 `desktop/runtime/vendor/` 已 gitignore）；`tauri.conf.json` resources 加 `{ "../runtime/bin/python": "python/" }`（目录映射，装到 `$INSTDIR\python\`，与 `avrag-api.exe` 同级）；`dev-windows-hotswap.sh` `copy_sidecars_into` 同步拷贝 `python/`。
4. **D-rag-full 两轮**：
   - 第一轮（新 bridge、旧 POM）：服务端已产出答案但 Playwright 在 `waitForAssistantMessage` 超时 120s——真因是 **POM 选择器 bug**：`chat-message-assistant` testid 在组件树中不存在（真实 DOM 是 `[data-testid="chat-message"][data-role="assistant"]`，`chat-message-list.tsx:327/329`）。该选择器自 `559c042d` 起就错，08-14 的「citation 失败」叙事实为同一超时。已修 POM 并同步 `C:\dev` 副本。
   - 第一轮另一教训：release 交叉编译与并行 subagent 编辑 agent-loop 存在时序竞争，exe 未含 T5 修复（从 bm25 日志 1+8 形态推断）。**改代码后必须等编译完成再验收。**
   - 第二轮（含 T5 + POM 修复）：**37.7s 绿**。wave-1 short_sac 7 条 `sandbox retrieval bridge call`（dense×3 / grep×3 / lexical×1，均 `chunk_count=1`），`n_hits=7 tool_calls=3 tool_results=7 sandbox_errors=0 coverage=partial rebrief_used=0`，citation 断言通过。
5. **观测增强**：`run_lead_workers.rs`「lead_workers rag worker done」加 `tool_calls / tool_results / sandbox_errors` 三字段（SaC 循环内部此前零日志，无法区分「模型没 codegen / 沙箱失败 / 代码没调 client」三种零命中形态）。
6. **§2.5（可选项）已提前完成**：dense host-leaf `doc_ids`→`doc_scope`（`scoped_rag_dispatch.rs:17` 证实 dense 的 scope 字段是 `doc_scope`；arg struct 有 `deny_unknown_fields`，旧写法会硬失败）；`lexical_terms_from_query` 改非字母数字切分 + 19 词英文停用词过滤 + 空过滤回退整查询（CJK 单词行为不变）；新增 2 单测。agent-loop 全量 **428 绿**（`codegen_bridge::test_observation_includes_graph_context_from_augment_telemetry` 首跑 flake、单跑与重跑均过——既有顺序依赖问题，与本次改动无关，值得后续单独看一眼）。
7. **回归**：Linux `cargo test -p avrag-code-interpreter --lib` 18 passed/1 ignored、`agent-loop --lib` 428 passed；`code-review-graph update` 已跑。
8. **NSIS 端到端验证（2026-08-15 第三轮）**：`SKIP_FRONTEND=1 bash scripts/build-windows.sh` 出 `Context-OS Client_0.2.0_x64-setup.exe`（78M），清洁目录静默安装（`/S /D=…`）验证：`python/`（35 文件，`python.exe` 跑通 3.12.10）、`runtime/`（pgsql/redis/migrations）齐。**发现并修掉既有缺口：NSIS 从不安装 `modes/`+`prompts/`**（sidecar 按 CWD=安装目录解析 `modes/*.yaml` / `prompts/**`，真机此前靠 hotswap `copy_runtime_assets` 补上，全新安装必缺）——`build-windows.sh` resources map 现含 `modes`/`prompts`/`python` 三项。注意：Tauri `--config` 合并对数组是**替换**语义，`tauri.conf.json` 的 resources 在 `build-windows.sh` 路径下被 extra config 整体覆盖，两边都要维护（dir 映射形式 `{ "../runtime/bin/python": "python/" }`）。
