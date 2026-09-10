# GPUI D3.2：Office 真实入库与 RAG 验收

用户已批准 25–40 分钟自动验证和少量嵌入、入库模型及 3 次 RAG 问答费用。不操作桌面，不更新运行中的客户端，不重启原有服务，不支付或部署。

## 验收范围

`desktop_gpui/tests/live_rag.rs` 使用实际 GPUI Host 和独立 Windows API/worker/PG/Redis，创建新本机账号和工作区，经签名上传 DOCX/XLSX/PPTX，等待 worker 完成并回读正文及 chunks。三份资料全部完成后才开始单文档范围的 3 次问答，检查答案、引用归属、嵌入计量、钱包扣费和 RAG 审计。只使用产品注册赠送余额，未注入余额或绕过计费。清理测试文档/工作区并回读确认后，由 Host 释放本批拥有的服务。

运行：`pwsh -NoProfile -File desktop_gpui/scripts/accept-managed.ps1 -OfficeRag -ApiPort 18192 -PgPort 15440 -RedisPort 16390`。模型和价格从现有 `.env` 白名单复用，报告不含凭据。测试问句位于 `avrag-rs/prompts/eval/gpui-office/`；合成资料预期值仅位于测试断言。

## 首次真实失败，保留记录

目录：`C:\dev\context-osv6\desktop_gpui\target\acceptance\managed\gpui-managed-0ba115d3347b4ad2b0df3499db3ff49e`。

- DOCX 签名上传、真实 worker 入库、chunks 和预览通过。
- XLSX 解析和 IR 投影通过；结构化表格阶段 worker 在 15:05:45 发生 Windows `0xc0000005` 异常，未完成入库。PPTX 和 3 次 RAG 未执行。
- 原用例等待入库超时后失败退出，`journey.json` 的 `cleanup_errors=[]`；`result.json` 的 `remainingProcesses=[]`，6 个原有主进程身份不变。
- 补入 worker 存活检查，使此类失败立即结束该验收门。

## 运行库问题与修正

当前 C++ 编译器为 GCC 13 `win32` 线程模型；旧 `stage-desktop-sidecars.sh` 优先取 `13-posix` DLL，验收/升级脚本又复制安装版 DLL。GCC 明确要求应用与 libstdc++ 使用相同线程模型：[官方并发说明](https://gcc.gnu.org/onlinedocs/libstdc++/manual/using_concurrency.html)。

已删除按版本和线程模型猜路径的逻辑。`stage-mingw-runtime.sh` 通过实际 C++ 编译器的 `-print-file-name` 解析完整 DLL 集，记录编译器身份与 SHA256；验收和升级从相同 BuildDir 取运行库，不再借用安装版。

未重新编译 DuckDB 或后端：使用已有 `libduckdb.a` 链接 `scripts/desktop-e2e/duckdb-smoke.cpp`。探针配旧 DLL 无法启动（退出 `0xc0000139`）；配匹配的 `libstdc++-6.dll` 与 `libgcc_s_seh-1.dll` 后，DuckDB v1.5.5 打开内存库、执行 `SELECT 42`、关闭通过。独立线程/互斥锁/条件变量探针也通过。探针与 worker 的故障码不同，真实 worker 是否恢复由下面的复验判定，不以探针替代入库验收。

证据在首次目录的 `duckdb-original.stderr.log`、`duckdb-matched.stderr.log`、`thread-matched2.stderr.log`；原 DLL 以 `*-original.dll` 保留。首次长链接在跨系统盘写入过慢，停止仅本次链接进程后改在 WSL 临时目录链接，相同静态库和参数成功。未停止既有产品服务。

## 修正后复验

目录：`C:\dev\context-osv6\desktop_gpui\target\acceptance\managed\gpui-managed-6f4d540e4f0847958578edcb2a5179ce`。

复验仍失败：DOCX 再次完成，XLSX 在结构化表格阶段再次触发 `0xc0000005`、地址 0；运行库修正不足以消除该崩溃。新的 worker 存活检查使整条测试在 42.79 秒内退出，不再等待 6 分钟；`cleanup_errors=[]`、`remainingProcesses=[]`，6 个原有主进程身份仍不变。PPTX/3 次 RAG/最终计费审计门未运行，D3.2 未通过。

使用后端同一版本 Rust 1.96.1 和已编译 DuckDB rlib 的探针通过：内存库、普通表、参数化写入和查询均正常。输出在首次目录 `duckdb-rust.stderr.log`。

进一步在 C++ 探针创建 `checks JSON, notes JSON` 表时，自动安装/加载 JSON 扩展后稳定触发 `0xc0000005`。普通表格操作不触发；证据 `duckdb-json.stderr.log` 含故障地址和调用栈。因此 JSON 扩展加载是一个已复现阻断点，仍需重建后的真实 worker 验证是否消除全部问题，不能仅凭触发点断言故障已修好。

探针自动下载产生的两个 JSON 扩展缓存文件（创建时间 15:33:20）已先复制到首次目录 `downloaded-json-extension/` 保存证据，再按完整路径和创建时间校验后删除原缓存文件；未删除此前已有文件。更新后的探针禁用自动安装并使用独立扩展目录，避免再次写用户缓存。

## 已准备、尚未编译的修复

worker、struct-supervision、rag-core 的 DuckDB 依赖显式启用现有 `json` feature，让 JSON 扩展与内核使用同一工具链构建，不在首次入库时获取外部 JSON 二进制。没有降级表格为纯文本或跳过结构化能力。新增探针在基础查询后验证 JSON 列及全文索引 `create_fts_index` / `match_bm25`；当前 `bundled` 不等于内置 FTS，FTS 打包及加载也必须过门。

这会触发 DuckDB 原生重编，尚未执行；下批需确认约 45–75 分钟构建/扩展检查和真实复验耗时。未将解析或探针通过表述为 D3 全量通过；其余 Tauri 知识能力、客户端三平台对等仍不在本门。
