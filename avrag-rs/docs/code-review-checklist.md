# 代码审查 Checklist

> 每次提交涉及以下模块时必须逐项检查。

## Milvus 查询

- [ ] 每个查询必须带服务端 ACL filter（`owner_user_id`、`workspace_id`、`doc_scope`）
- [ ] 每个检索结果必须带 provenance（`doc_id`、`chunk_id`、`page`、`parse_run_id`、`source_locator`）
- [ ] 图扩展必须有 `fan_out_limit`、`hop_limit`、`relation_count` eviction

## Agent 事件流

- [ ] 新增 `AgentEvent` 变体必须同步更新 SSE serializer 和 frontend parser
- [ ] Streaming 路径和非 streaming 路径行为一致（`ChannelSink` vs `CollectingSink`）

## RAG 相关

- [ ] Planner 输入不注入 `session history` 或 `recent_messages`
- [ ] `doc_scope` 为空时必须返回明确错误（不静默降级）
- [ ] 新增 tool 必须注册到 `ToolCatalog` 并更新文档 §5

## Prompt

- [ ] 新增/修改 prompt 必须外置到 `prompts/` 目录，使用 `include_str!`
- [ ] 不在代码中硬编码系统提示词

## Guard

- [ ] Input guard 变更必须同步更新测试（`guardrails/src/input/`）
- [ ] Output guard 变更必须同步更新测试（`guardrails/src/output/`）

## Auth

- [ ] `login_required` 仅用于未登录场景（token 缺失/过期）
- [ ] `unauthorized` 仅用于已登录但权限不足场景
- [ ] 新增路由必须检查 auth middleware 覆盖
