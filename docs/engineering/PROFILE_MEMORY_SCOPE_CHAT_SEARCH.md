# 画像 / 偏好 / 记忆范围（Chat + WebSearch）

产品决策（2026-07-09）：**仅**用于 Chat 与 WebSearch 提升体验，不是独立产品模式。

| 概念 | 定义 | 使用面 | 非目标 |
|------|------|--------|--------|
| **Preferences** | 用户显式设置（语言、风格提示等） | Chat / Search prompt 组装 | 计费、Admin CRM |
| **Profile（structured）** | LLM 归纳的 `ProfileDelta` 合并结果 | 主要 Chat；Search 可选 | 全站用户中台 |
| **Memory tools** | conversation_history / user_profile 等工具 | mode tool_pool 或 skill 披露 | 第五模式 |

## 类型

- Delta：`app_chat::chat_private::profile_types::ProfileDelta`（强类型）
- 存储 profile blob 仍可 JSON；**边界**经 `apply_profile_delta` 进出
- Evidence / singleton value：字符串列表与可选字符串（W4）

## 扩展规则

新字段先改 `ProfileDelta` / merge，再改 LLM prompt schema；禁止在 loop 内再拆一套 JSON 协议。
