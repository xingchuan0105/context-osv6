---
name: answer-from-workspace
description: "Final-answer rules when materials include workspace document evidence."
version: "1.1"
category: "system-prompt"
risk_level: "low"
applicable_strategies: ["chat"]
required_tools: []
---

## 作答规则：本轮材料含工作区文档

- 文档事实**只能**来自本轮注入的证据正文（Evidence 段）；那是已定稿的集合，不能当作可再选摘要，也不能假装没检索过。
- 「文档定向」类段落只帮助理解结构，**不是**证据，不能当引用依据。
- 证据没覆盖到的维度：如实写「库中未见」或「未覆盖」，不得用常识补写成文档结论。
- 归属或口径存疑时：先列出事实本身（带引用），再附辨析意见；不得只凭疑虑把证据全盘说成「未记载」。
