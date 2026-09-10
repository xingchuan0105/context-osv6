# Windows API 构建：DuckDB COFF section 限制

第一轮 Windows GNU debug 构建在 bundled DuckDB C++ unity 文件失败：`too many sections` / `file too big`。这不是磁盘空间不足；目标文件达到标准 COFF section 数量上限。API 未生成，迁移程序已生成，未替换独立验收服务。

修正：在 avrag-rs/.cargo/config.toml 为 `CXXFLAGS_x86_64_pc_windows_gnu` 配置 `-Wa,-mbig-obj`，只作用于 Windows GNU C++ 目标。工具链的 `x86_64-w64-mingw32-as --help` 确认支持 `-mbig-obj`。

验证：生成 40,000 个自定义 section 的汇编压力文件。默认参数返回 1，报告 40,004 sections / file too big；`-mbig-obj` 返回 0。证据位于 C:\dev\gpui-coff-default.log 和 C:\dev\gpui-coff-big.log。该验证只证明汇编器格式支持，不代表完整 API 构建通过。

用户已批准 45–75 分钟重编。第二轮使用 jobs=2、C:\dev\gpui-backend-target 缓存，日志 C:\dev\gpui-backend-build-retry.log；不修改旧数据库或已安装客户端。窗口验收由用户操作，禁止使用 Computer Use。第二轮完成及真实链路结果另记。

## 第二轮结果与独立环境更新

第二轮构建成功，日志 `Finished dev profile ... in 71m 22s`。Windows API 于 2026-09-10 01:34 生成。

08:45 后执行 upgrade-isolated-backend.ps1：在隔离 PG 15433 内创建 avrag_gpui_current，保留上一测试库及原安装数据库；当前迁移通过后，只替换已核对 PID/路径的测试 API。新 API 路径为 C:\dev\gpui-acceptance-20260909\backend-current\avrag-api.exe，18082 的 /health 返回 status=ok、postgres:ok。对应 prompts/modes/migrations 已同步至独立目录。

新 GPUI 窗口使用 session-current 目录打开，等待用户手工点击连接和发送。尚未据此判定真实聊天、停止或历史恢复通过。

## 用户首轮反馈与价格配置修正

用户窗口连接成功、会话已创建，但发送返回 `No official price row for dashscope/qwen3.8-flash`。项目 .env 已有该模型的价格表，验收脚本只传 AGENT_LLM 配置而漏传 PLATFORM_OFFICIAL_RATES_JSON。

两条隔离环境脚本补传原价格表；upgrade 脚本新增 Refresh 模式，不重复创建数据库/迁移/覆盖程序，仅按已记录 PID 和路径核对后刷新独立 API。09-10 08:52 刷新后 /health 为 ok。测试签名/加密密钥开始持久化到受限权限的 current-secrets.json；首次切换前核对 user_provider_secrets 为 0，未破坏已存 BYOK 数据。首次切换需要重新连接，后续刷新复用密钥。旧库与安装版服务未动，价格门通过和真实模型输出仍待用户重试确认。

08:55 用户重试出现 LLM client is not configured。核对 AppConfig 后确认个人聊天使用 QUICK_CHAT_LLM，密钥默认读取 DASHSCOPE_API_KEY，而验收脚本只传了 AGENT_LLM。两条脚本补齐 QUICK_CHAT_LLM_* / DASHSCOPE_API_KEY，并增加密钥和价格 JSON 的启动前检查。

08:57 真实 API 复验通过：请求 7cc2bf3f-7518-4c08-bd04-2ea2d12d6a31，session 34a97cc6-a14c-4b88-850a-5e23c745a802，收到 token 与 done，回答“连接正常”。done 记录 dashscope/qwen3.8-flash，prompt 1186 / completion 12 tokens。重新读取 messages 得到用户问题和 assistant 回答，workspace_id_at_send=null。SSE 证据保存于隔离 logs/quick-chat-live.sse。该证据为接口真实模型/历史持久化通过；GPUI 用户窗口重试、长正文流式和停止仍待验。

08:58 用户提供天空问题的完整回答截图，并明确反馈“有打字机效果”。GPUI 真实模型回答及正文逐步显示由用户确认通过，截图状态为“已完成”。可见缺陷仍有 Markdown 原文符号直接显示、多个历史条目均为“未命名对话”；不据此宣布视觉验收或 D1 全部通过。停止、中止后继续发送及窗口内历史恢复仍待用户操作验证。

用户随后发送太阳系长文请求，在正文开始后点击停止，并明确反馈“文字立即停止，已显示内容保留”。截图显示保留了标题和首段片段，状态为“已停止”，发送按钮已恢复。窗口侧停止与部分正文保留由用户确认通过；不据此推断服务端模型请求取消或停止内容已持久化。下一轮发送及窗口内切换历史恢复仍待验。

用户继续在同一对话发送新一轮，并确认“能正常收到新回复，且之前停止的回答没有继续追加”。停止后继续发送及窗口内旧回答不再追加由用户确认通过。窗口内切换会话后的历史恢复仍待验，尤其需核对中止回答的已显示片段与停止后新一轮回复是否保留。

用户切换到另一条会话后再返回，确认“切回来都保留”。窗口内历史恢复由用户确认通过，包含中止回答的已显示片段、随后发送的问题和新回复。该确认覆盖会话切换，不扩展为客户端重启恢复或服务端模型取消的验证。Windows 原生中文 IME 候选/组字仍待用户确认；Markdown 渲染和历史名称问题仍未修复。
