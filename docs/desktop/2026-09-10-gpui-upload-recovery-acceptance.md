# GPUI D3 上传恢复与真实工作区验收 · 2026-09-10

本批通过 74 项自动检查：共享核心/聊天/HTTP 52 项、GPUI 无头 UI 21 项，以及真实 API 1 条 8 步旅程。修复上传提交失败重复建资料、已提交但响应丢失的恢复，以及真实服务发现的笔记更新 HTTP 方法错误。Windows 独立验收程序构建成功，当前运行的客户端与服务保持不变。

授权范围：用户在收到 15–25 分钟耗时说明后回复“同意，完成后自动验证”。未使用 Computer Use、未调用模型、未重启已有服务，不要求逐项人工确认。测试只在既有独立 API `127.0.0.1:18082` 创建本批对象，不执行有效文件入库。

## 问题与修复

1. 文件 PUT 成功后，complete-upload 若返回错误，旧“重试上传”会重新创建资料。现在保留原 document_id，显示“重试提交”，只提交已有资料；切换工作区仍保留任务。主机内存任务不存在时，资料列表的 pending 记录提供“继续提交”。
2. complete-upload 可能已经入队，但响应丢失。错误后读取当前工作区的授权资料列表，核对同一资料状态；只有 enqueueing/queued/processing/completed/failed 这些已越过上传阶段的状态才确认提交已发生。其他状态保留错误，不重复上传或删除已提交资料。夹具实际断开 HTTP 响应，验证只发生一次创建、一次 PUT、一次提交。
3. 真实验收首次在笔记更新处返回 HTTP 405：GPUI 使用 PATCH，现有服务与 Rust Web 使用 PUT。本批修正 GPUI，并让夹具拒绝不支持的方法和不属于当前工作区的笔记。新增“保存后再次修改”的真实控件用例；首批夹具通过不再被用作真实笔记更新已通过的证据。

本批没有修改后端接口、shared-core、锁文件、运行服务或正在使用的旧程序。

## 自动验证证据

工作源码：`\\wsl.localhost\Ubuntu\home\chuan\context-osv6`；Windows 镜像：`C:\dev\context-osv6`。Rust jobs=2，UI 单 worker。

| 门 | 命令与结果 | 证据路径 |
|---|---|---|
| 上传恢复初验 | `accept-headless.ps1`，20/20 通过 | `C:\dev\gpui-d3-recovery-ui.log` |
| 真实 API 首次 | `accept-workspace-live.ps1`，笔记 PATCH 返回 405，失败；已创建对象清理成功 | `workspace-live\gpui-workspace-d1492948b2794007a9e8528432a5cd1b\journey.json` |
| 修正后真实 API | 同命令，1 条/8 步通过；脚本 4.76 秒 | `workspace-live\gpui-workspace-74adf9451eb341fe8a52c7271d908f09\journey.json` |
| 完整共享与无头 | `accept-tauri-shared.ps1 -WithHttpFixture -WithHeadlessUi`，core 30 + lib 17 + 原流 4 + HTTP 1 + UI 21，共 73 项通过 | `C:\dev\gpui-d3-recovery-shared.log`；`headless\gpui-ui-4f4b6acd6b3246f38657f09edafeaf09\result.json` |
| 强化清理回读后真实 API | `accept-workspace-live.ps1`，1 条/8 步通过；额外确认文档列表无测试资料，脚本 3.84 秒 | `workspace-live\gpui-workspace-bb130e904ae740ef8b58ed7761689536\journey.json`、`result.json` |
| Windows 生产 UI 构建 | `build-acceptance.ps1`，通过，10.31 秒 | `build\desktop-gpui-acceptance-20260910-142514\result.json`、`build.log` |

表内相对结果路径均位于 `C:\dev\context-osv6\desktop_gpui\target\acceptance\`。74 是 73 项共享/无头与最终 1 条真实旅程相加，重复复验不另计通过数；默认忽略测试不计通过。live_workspace 默认忽略，只有专用脚本设置隔离 API、已有会话和输出目录后才运行。

新增 4 条无头 UI 用例覆盖：提交失败后切换工作区再重试而不重传；提交成功但连接断开；没有内存上传任务时从 pending 记录继续提交；笔记保存后再次编辑使用 PUT。既有 17 条回归继续通过。

真实旅程覆盖：

1. 使用现有本机会话完成鉴权，不更新凭据或启动服务。
2. 创建两个带唯一标识的测试工作区，列表回读均可见。
3. 新工作区无其他工作区资料或笔记。
4. 创建中文多行笔记并回读，资料列表保持为空，笔记未自动入库。
5. 修改笔记，错误工作区更新被拒绝，原内容保持，另一工作区仍为空。
6. 删除笔记并回读确认消失。
7. 仅创建资料元数据，不发送文件字节；提交被拒绝，状态为 upload_invalid，未进入解析/嵌入队列。
8. 清理本次创建的笔记、资料和工作区，并回读列表确认工作区及资料不可见。失败路径同样执行清理；不按名称批量删除或接触既有对象。

## 产物与边界

- 新程序：`C:\dev\context-osv6\desktop_gpui\target\debug\desktop-gpui-acceptance-20260910-142514.exe`，未启动。
- SHA256：`2F38768693757F06BF440B22C97FA39CBD8EDDDE517808900635D4F4A49781B4`。
- 真实测试 API PID 37708、启动时间 08:56:41、路径 `C:\dev\gpui-acceptance-20260909\backend-current\avrag-api.exe`；SHA256 `36E6D76200364ECC81C37EFAA84B2E021498C6B1436B34EB27EB228B42E5EB53`。脚本核对服务与原 GPUI 的进程身份不变。
- 无头、真实旅程和构建结果共 45 项源码哈希核对（有重复文件），全部与 WSL 源工作区一致。关系图已更新，14:29:46 状态为 2,564 个文件、21,634 个节点；暂存差异空白检查通过。最终 API 健康回读为 `status=ok`、`postgres:ok`，原 GPUI PID 46220 的启动时间仍为 10:52:01。
- 无头测试对应 TestPlatform 控件/Host/隔离 HTTP；真实旅程对应 GPUI Host/现有 API/数据库。真实旅程没有打开 GPUI 窗口，不能称为 GPU 像素验收。
- 实际办公文件解析、嵌入计费、真实 RAG、macOS/Linux 和完整 Tauri 对等仍待验。本批的“缺少文件内容时提交失败”不代表有效文件入库通过；没有把模拟队列状态当作真实解析结果。

下一门仍是有效办公文件的真实入库和 RAG，需按模型调用范围安排授权；D3 其余知识入口按 [对等清单](GPUI_PARITY_CHECKLIST.md) 继续推进。
