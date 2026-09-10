# GPUI D3.2 前置门：Windows Office 解析包

2026-09-10。有效文件入库前检查发现，原 Windows `anydoc-lite` 仅实现 DOCX/CSV，而 XLSX/PPTX 等 Office 文件仍由后端路由给它。此前文档中“office ingest 无需额外解析器”的表述范围过宽。当前独立验收 API 也没有运行 worker；本门先解决解析包阻断，后续再验证入库和 RAG。

## 修改

- 删除简化的 `anydoc_lite.py` 及旧命令入口，复用后端已有 `avrag-rs/scripts/anydoc-extract`。共享 desktop-core 写入新的 `anydoc-extract.cmd` 路径，Tauri 和 GPUI 使用同一配置生成逻辑。
- Windows staging 将 `firecrawl-anydoc==0.1.2` 的 CPython abi3 x64 wheel 放入嵌入式 Python，保留原 `._pth` 隔离设置，不依赖系统 pip 或用户 Python 包。
- wheel SHA256：`dcb20ff01a7874acd3397903271da64e0cc966d2a516afff54527df8dc58ba30`；下载及缓存均校验。打包保存依赖版本、来源、wrapper/命令摘要，缺失 `.pyd`、wrapper、命令或清单会阻止生成包配置。
- 自动验收使用真实 CLI、已安装 Python 的独立副本和固定依赖；复制出开发目录及 NSIS 安装目录两种资源布局。没有操作已安装程序。

依赖来源：[PyPI 0.1.2](https://pypi.org/project/firecrawl-anydoc/0.1.2/)、[上游项目](https://github.com/firecrawl/anydoc)。解析始终在本机执行。

## 授权与结果

用户在 15–25 分钟说明后确认“同意，完成后自动验证”。本批没有调用模型、操作桌面或重启已有服务。

| 验证门 | 结果 | 范围 |
|---|---|---|
| Office 解析 | 9/9 通过 | DOCX、XLSX、PPTX，两种资源布局；中文 CSV/空格路径；损坏文件和缺失文件非零退出且无成功产物 |
| 共享 Tauri/GPUI 核心 | 52/52 通过 | desktop-core lib 30，GPUI lib 17，原 Tauri SSE 4，隔离 HTTP 1 |
| GPUI 无头 UI | 21/21 通过 | TestPlatform 真实视图、Host、隔离 HTTP；不含 GPU 像素或系统输入法 |
| 打包配置 | 通过 | 执行 `build-windows.sh` 实际 Python 配置块：完整 Office 资源映射正确，缺件拒绝输出配置；未生成/安装 NSIS 包 |
| Shell 语法 | 通过 | `bash -n scripts/stage-desktop-sidecars.sh scripts/build-windows.sh` |
| Windows 生产 UI 构建 | 通过 | 313.9 秒；独立文件，不覆盖或启动现有程序 |
| 代码关系图 | 已更新 | `code-review-graph update --brief`，包含本批新增脚本与测试 |

合计 82 条自动测试通过；打包配置检查另外记录。Excel 输出保留 `Cedar | 137` 同一行；PPT 读取到 `Violet Harbor`。这些是已有合成夹具的解析断言，没有写入产品提示词。

第一次直接从 UNC 运行未签名 PowerShell 脚本被本机执行策略拒绝，测试尚未开始。随后仅对本次子进程使用 `pwsh -NoProfile -ExecutionPolicy Bypass -File`，没有修改系统执行策略，解析首次实际执行即通过。

## 证据与复跑

源仓库：`\\wsl.localhost\Ubuntu\home\chuan\context-osv6`。Windows Rust 构建镜像：`C:\dev\context-osv6`。

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/accept-office-parser.ps1
pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/sync-windows.ps1
pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/accept-tauri-shared.ps1 -WithHttpFixture -WithHeadlessUi
pwsh -NoProfile -ExecutionPolicy Bypass -File desktop_gpui/scripts/build-acceptance.ps1
```

- 解析包：`C:\dev\context-osv6\desktop_gpui\target\acceptance\office\gpui-office-46b226105ab6443cbbf942e2aaeabc25`。`result.json` 含源文件与夹具 SHA256、耗时和进程身份；`stage.log`/`tests.log`、`output/*.md`/`*.stderr` 保留产物；`bundle-check.json`/`bundle-invalid.log` 保留配置检查证据。
- 共享回归：`C:\dev\gpui-office-shared.log`，详细日志在 `C:\dev\context-osv6\desktop_gpui\target\acceptance\tauri-shared`。
- 无头结果：`C:\dev\context-osv6\desktop_gpui\target\acceptance\headless\gpui-ui-bb894cc9996e4836bfb9479587951a34`，21 条通过，0 失败，18.72 秒。
- 构建：`C:\dev\context-osv6\desktop_gpui\target\acceptance\build\desktop-gpui-acceptance-20260910-144416`。
- 产物：`C:\dev\context-osv6\desktop_gpui\target\debug\desktop-gpui-acceptance-20260910-144416.exe`，SHA256 `CCE1BF4B6F4B84348644B25E83478D6FB7F9D8C66BE49138D5A372514C0ADE43`。当前源码和锁文件的构建，使用独立二进制名，不是已安装版本。构建结束后原程序摘要未改变；修改过的 desktop-core 源文件与 Windows 镜像摘要一致。
- 现有 API PID 37708、GPUI PID 46220 的路径及启动时间在解析前后保持一致，安装目录未写入。
- 完成时复查 18082 `/health` 为 `ok`、`postgres:ok`，两个原进程身份仍一致。Rust 构建使用 jobs=2，无头 UI 单线程；未以 `ignored` 项计入通过数量。

## 未覆盖

此门只证明打包后的 Office 解析能力和已有 UI/Host 回归。尚未验证签名上传后的真实 worker 入库、数据库/向量写入、嵌入计费、RAG 回答/引用；也未完成 NSIS 安装或 macOS/Linux 验证，D3 仍为部分完成。运行中的旧客户端和安装版解析器没有更新。

下一门使用新目录和端口启动独立 API/worker/PG/Redis，接入现有模型配置，先验证有效文件处理完成，再验证限定资料的真实 RAG；模型费用及该批运行耗时另行确认。
