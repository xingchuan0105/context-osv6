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
