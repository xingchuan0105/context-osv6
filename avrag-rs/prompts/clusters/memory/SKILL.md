---
name: memory
description: "Load when the user refers to earlier conversation beyond the two prior user turns already shown, asks about past preferences or decisions, or uses pronouns/ellipsis that need earlier context. Skip for self-contained questions answerable from the current turn and the default recent history."
disclose_at: retrieve
atomic: false
applicable_modes: [rag, search, chat]
version: "3.0"
---

## 默认可见历史

上下文里通常已有：

- 当前用户问题
- 最近 **2** 条更早的用户发言（常标 prior user）

系统**不会**自动把「它 / 那位 / 这本书」消解成实体。指代与更早偏好需要额外回传才闭合。

## 取更早记忆的入口

`client.history` / `client.user_profile` 是每轮可用的基础原语，在 **Python 沙箱**中随时可调：

```python
hist = await client.history(limit=20)   # 可带 query；字段以回传为准
profile = await client.user_profile()
print(hist)
print(profile)
```

| 回传状态 | 含义 |
|----------|------|
| 仅有默认 2 条 prior | 更早轮次仍为未知 |
| `history` 非空 | 可用其中最近相关用户话锚定实体 |
| `user_profile` 有字段 | 长期偏好可引用；`null`/缺字段 ≠ 用户无偏好，只是未记载 |
| 多候选实体同等可能 | 实体未闭合；澄清问题比臆造实体更贴合证据边界 |

## 指代与检索用词

- 用户可见答复可用其原措辞；检索 query 可用消解后的实体词。
- 话题已切换时，以最近一轮明确实体为准。
- 「记忆」= 更早对话 + 长期画像，不限于指代消解。

更细的表述模式见 `reference/anaphora.md`（若已加载）。
