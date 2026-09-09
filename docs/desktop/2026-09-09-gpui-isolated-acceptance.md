# GPUI 独立 Windows 数据库验收

用户确认：新建独立 Windows 测试库，旧库不动。

## 已完成

- 新增 `desktop_gpui/scripts/start-isolated-acceptance.ps1`：新目录拒绝覆盖；端口占用时拒绝启动；每个进程记录 PID/路径；后台服务隐藏窗口。仅从 avrag-rs/.env 复用 AGENT_LLM 配置，其余身份、JWT、BYOK、对象目录、会话目录均独立。
- 状态目录 `C:\dev\gpui-acceptance-20260909`；PG 15433、Redis 16380、API 18082。旧 PG 5433 / Redis 6380 没有停止或修改。
- 使用 Windows 安装包的 PostgreSQL 16、API 和 migrations。首次迁移缺少目录参数，补 AVRAG_MIGRATIONS_DIR 后通过；ResumeMigration 仅允许本脚本跟踪的隔离进程且尚未登录的测试环境。
- SQL 核对 avrag_cluster_admin 为管理员；avrag、avrag_runtime 均 NOSUPERUSER / NOBYPASSRLS。不是对旧 bootstrap 角色降权。
- API /health 成功；GPUI 原生点击连接后显示“本机 · 已连接”，独立 users 表有 1 个本地用户。

## 失败门与范围

GPUI 原生发送“请用三句话解释为什么天空是蓝色的。”后，API 返回 `workspace_id is required`，界面显示该错误。未给个人聊天注入隐藏工作区。安装版 API/迁移程序为 2026-08-31 构建，不能验证当前个人聊天契约。

真实模型回答、正文流式、停止、历史恢复均未通过；未观察到模型输出。中文文字输入通过不代表输入法组字通过。后台未启动 worker，本批仅个人聊天，不验收文档入库。

下一门：编译当前 Windows API/迁移程序及同步对应运行资源，只替换独立验收环境，保持旧库和安装客户端不变。已单独请求 30–60 分钟构建授权。

## 当前运行状态

独立 PG、Redis、API 保持运行，日志及 PID 记录在状态目录；GPUI 窗口保留错误现场。没有支付、部署、清理旧数据。
