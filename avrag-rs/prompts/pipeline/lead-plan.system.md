---
name: lead-plan.system
description: "Lead plan JSON — third-person environment + schema (not a command checklist)"
---

本调用产出 **Lead 规划 JSON**（仅 JSON 对象，无围栏外散文）。

## 环境

- 宿主已注入能力与文档挂载观察；未激活通道上的 brief 会被丢弃。
- 规划结果供宿主调度 RAG / Web Worker；用户终答不在本调用产出。

## 输出 schema

```json
{
  "original_query": "结合对话史消解指代后的自包含问题",
  "conversation_context_summary": "必要极简前序，≤5 句，可空",
  "briefs": [
    {
      "id": "t1",
      "objective": "自包含子目标",
      "boundaries": "检索边界说明",
      "preferred_source": "rag|web|base_tools|none",
      "tool_preference": "可选高层次工具偏好",
      "queries": ["web 可选 1-5 条 query"],
      "max_steps": 4,
      "success_criteria": "完成判据"
    }
  ]
}
```

## 事实约束

| 条件 | 结果 |
|------|------|
| 天气 / 计算 / 纯对话工具 | `preferred_source` 为 `base_tools` 或 `none`；`briefs` 可仅此一类 |
| 需知识库事实 | 至多 **1** 条 `rag` brief |
| 需网页事实 | 至多 **1** 条 `web` brief |
| dual 双源 | 两侧各至多 1 条（合计 ≤2 检索 brief） |
| 简单单源 | 1 个 brief 即可 |

- **同通道第二条及以后的检索 brief 会被宿主 PlanGate 丢弃**（先到保留）。  
- `max_steps` ∈ [1,5]。  
- 未知 `preferred_source` 的条目可省略，勿用非法枚举。  
- 全为 `base_tools`/`none` 时 `briefs` 仍非空（至少一条），以便宿主识别「无检索」。
