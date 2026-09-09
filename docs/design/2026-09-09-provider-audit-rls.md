# Provider resolve 审计 RLS 修复

## 问题与修正

承接 `2026-09-09-chat-progressive-answer.md` 的待查项。密钥读取事务提交后，`resolve` 将审计 INSERT 直接交给裸连接池；原事务的身份上下文已失效，触发 `provider_secret_audit` 的 RLS 拒绝。

审计现在通过独立事务写入，调用既有 `set_current_user_sqlx` 设置事务级 owner 身份，成功后提交。没有关闭 RLS、修改表策略或新增管理员豁免。保留原有 best-effort 契约：审计基础设施故障仍记录告警，不改变密钥解析的返回行为；因此本修正不等于提供审计必达保证。

## 验证证据

- `cargo test -p app-bootstrap --lib`：19 通过，数据库用例默认忽略。
- 单独显式执行 `audit_resolve_enforces_owner_rls_and_clears_identity -- --ignored`：1 通过，使用 `.env` 配置的真实 PostgreSQL 运行时角色，并断言该角色不是 superuser / BYPASSRLS。
- 数据库用例调用产品的审计函数，确认一条记录落库及字段正确；另一用户查询结果为零，冒用 owner 插入返回 SQLSTATE 42501；单连接池事务结束后身份为空。生成的合成记录已删除。
- `cargo build -p avrag-api -j 2`：通过。API 已刷新，健康检查通过。
- 代码关系图已增量更新，`git diff --check` 通过。

日志：`/tmp/context-provider-audit-lib.log`、`/tmp/context-provider-audit-db.log`、`/tmp/context-provider-audit-build.log`，同时保存到 `frontend_rust/target/acceptance/evidence/provider-audit/`。

本批验证为真实数据库的产品写入函数回归，没有重新发起真实模型 RAG 调用或全站浏览器回归；未执行部署、支付。其他未提交的 Subtex / transport-http 改动保持原状，本地构建使用当前工作区。
