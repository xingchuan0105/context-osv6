# RAG Prompt 纯净版 v0（草稿）

与生产路径对照：

| 草稿 | 生产 |
|------|------|
| `rag-system.md` | `prompts/orchestrators/rag-system.md` |
| `codegen-SKILL.md` | `prompts/clusters/codegen/SKILL.md` |
| `rag-answer.md` | `prompts/synthesis/rag-answer.md` |

启用方式（迭代测题时再改）：复制覆盖生产文件，或改 `modes/rag.yaml` 的 `system_prompt_base` 指向草稿。

设计原则：**只写 LLM 内化知识里没有的平台事实**——代码块格式、client API、沙箱限制、JSON 合成契约、skill_request 语法。检索策略、拒答话术、矛盾处理等留给逐题迭代再加。
