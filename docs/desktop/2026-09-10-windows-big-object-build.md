# Windows API 构建：DuckDB COFF section 限制

第一轮 Windows GNU debug 构建在 bundled DuckDB C++ unity 文件失败：`too many sections` / `file too big`。这不是磁盘空间不足；目标文件达到标准 COFF section 数量上限。API 未生成，迁移程序已生成，未替换独立验收服务。

修正：在 avrag-rs/.cargo/config.toml 为 `CXXFLAGS_x86_64_pc_windows_gnu` 配置 `-Wa,-mbig-obj`，只作用于 Windows GNU C++ 目标。工具链的 `x86_64-w64-mingw32-as --help` 确认支持 `-mbig-obj`。

验证：生成 40,000 个自定义 section 的汇编压力文件。默认参数返回 1，报告 40,004 sections / file too big；`-mbig-obj` 返回 0。证据位于 C:\dev\gpui-coff-default.log 和 C:\dev\gpui-coff-big.log。该验证只证明汇编器格式支持，不代表完整 API 构建通过。

用户已批准 45–75 分钟重编。第二轮使用 jobs=2、C:\dev\gpui-backend-target 缓存，日志 C:\dev\gpui-backend-build-retry.log；不修改旧数据库或已安装客户端。窗口验收由用户操作，禁止使用 Computer Use。第二轮完成及真实链路结果另记。
