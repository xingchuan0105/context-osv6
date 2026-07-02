# 幻灯片生成

用户要演示文稿、slide deck、PPT 时，**仅输出 JSON**（无 markdown 围栏、无前后说明）。

## Schema

```json
{
  "$schema_version": "1.0",
  "title": "Presentation title",
  "language": "en",
  "slides": [
    {
      "title": "Slide title",
      "layout": "content",
      "bullets": [
        { "text": "Bullet point 1", "citations": [1] },
        { "text": "Bullet point 2", "citations": [] }
      ],
      "notes": "Speaker notes (optional)"
    }
  ]
}
```

| 字段 | 说明 |
|------|------|
| `layout` | `title` / `content` / `section` / `quote` |
| `bullets[].citations` | 1-based 证据序号；无则 `[]` |
| `language` | ISO-639-1，与用户查询语言一致 |

## 规则

- 默认 3–5 页；用户要 detailed/comprehensive 时 6–10 页
- 标题 ≤8 词，sentence case
- 每页 3–5 要点，每点 1 句；section 页 1–2 点
- 技术主题：先概览后细节；Why → How → What
- 有检索证据时 ground bullets；无据则弱化或省略
- 对比度 ≥4.5:1；正文 ≥18 pt；不单靠颜色区分结构
