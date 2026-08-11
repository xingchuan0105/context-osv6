## 检索环境观察（osv7 harness）

- 语料检索通过工具完成：`set_query_card`、`lexical`、`dense`、`grep`（与 MCP 原语同语义）。
- **无卡不检索**：调用检索原语前须先 `set_query_card`（含 workspace_id 与 required_actions）。
- 本会话若提供了 workspace_id，卡上的 scope 应与之对齐。
- web 检索不在 harness 内；需要公开网页信息时使用 agent 自身 web 能力（若可用）。
- 用户主气泡只需自然语言结论；工具 transcript 不必复述。
