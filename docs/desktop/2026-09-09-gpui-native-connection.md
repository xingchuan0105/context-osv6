# GPUI 原生连接验收：阻断记录

## 结果

真实模型聊天、停止和历史恢复未通过验收，也未产生模型调用。本批 Windows UI 点击连接后观察到连接中及失败可重试状态。

1. 直接运行开发 exe 选中了 C:\dev\context-osv6\desktop\runtime。该目录 PostgreSQL 配置为 dynamic_shared_memory_type=posix，Windows 启动失败；其中 API/worker 为 8 月 12 日构建。
2. 新增 run-windows.ps1，显式指定 Windows 安装运行目录；共享产品路径解析支持 CONTEXT_OS_CLIENT_HOME 根目录的已安装 sidecar。实际重试后 Windows PG 5433 / Redis 6380 启动成功。
3. 现有 Windows 库角色查询得到 oid=10、rolname=avrag、rolsuper=true、rolbypassrls=true，无 avrag_cluster_admin / avrag_runtime。初始化试图降权 bootstrap 角色，PostgreSQL 16 拒绝。移除该旧自动修复路径，缺少当前管理员角色时明确报错，保留现有库。

Windows 安装中的 API/worker/migrate 为 8 月 31 日构建；不能作为最新后端功能对等证据。模型配置存在，但未调用。默认用户数据没有复制、删除或重建。

## 验证与运行状态

- 修改后 Windows UI 构建通过；desktop-core lib 19 测试通过。原有测试不覆盖真实数据库历史迁移；不将其标记为迁移验收。
- 代码关系图已更新，git diff --check 通过。
- 第一次失败启动创建的开发 Redis 已按 PID/路径核对后停止。第二次启动的 Windows 安装 PG/Redis 保持运行，API 未就绪；GPUI 验收窗口已关闭以完成构建。
- 后续需要用户选择独立测试库或旧数据备份迁移；在选择前不修改数据库归属或重建数据。D1 仍为待验。
