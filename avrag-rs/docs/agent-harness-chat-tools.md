# ChatAgent 工具集设计 — Context OS 场景

> 状态: 待确认  
> 目标: 定义 ChatAgent（非 RAG、非搜索）可用的工具，提升纯聊天场景体验  
> 原则: 不涉及检索、不涉及网络搜索，聚焦"生成/渲染/计算"类能力

---

## 一、已确认工具（当前已实现 stub）

| 工具名 | 用途 | 状态 |
|--------|------|------|
| `load_skill` | 加载领域 skill 文件，获取特定领域指令 | Stub |
| `compact_history` | 压缩对话历史，缓解上下文窗口压力 | Stub |

---

## 二、候选工具清单（待确认）

### 2.1 HTML 渲染类（Context OS 核心场景）

| 工具名 | 用途 | 前端效果 | 优先级 |
|--------|------|---------|--------|
| `render_html` | 渲染交互式 HTML 组件 | 图表/表格/时间线/卡片/看板等 | **P0** |
| `create_document` | 生成可编辑结构化文档 | 会议纪要/报告/大纲/清单 | **P0** |
| `visualize_data` | 数据可视化 | 柱状图/折线图/饼图/热力图 | **P1** |

#### render_html 参数设计
```json
{
  "component_type": "chart|table|timeline|card|comparison|mindmap|kanban",
  "title": "string",
  "data": {},
  "style": {
    "theme": "light|dark",
    "width": "100%"
  }
}
```

#### create_document 参数设计
```json
{
  "doc_type": "meeting_notes|report|outline|checklist|proposal",
  "content": {},
  "editable": true
}
```

#### visualize_data 参数设计
```json
{
  "chart_type": "bar|line|pie|scatter|heatmap|treemap",
  "dataset": [],
  "x_axis": "string",
  "y_axis": "string"
}
```

### 2.2 计算/执行类

| 工具名 | 用途 | 场景 | 优先级 |
|--------|------|------|--------|
| `calculate` | 精确数学计算 | 算术、财务计算、单位换算 | **P1** |
| `get_datetime` | 获取当前时间 | "现在几点"、时区转换 | **P1** |
| `execute_python` | 执行 Python 代码 | 数据分析、复杂计算、可视化 | **P2** |

### 2.3 内容处理类

| 工具名 | 用途 | 场景 | 优先级 |
|--------|------|------|--------|
| `parse_csv` | 解析 CSV 数据 | 用户粘贴表格数据后分析 | **P2** |
| `render_mermaid` | 渲染 Mermaid 图表 | 流程图、时序图、甘特图 | **P2** |

---

## 三、Context OS 场景示例

| 用户请求 | 调用工具 | 前端效果 |
|---------|---------|---------|
| "帮我整理这次会议的关键决策" | `create_document` (meeting_notes) | 可编辑的会议纪要卡片 |
| "对比这三个方案的优缺点" | `render_html` (comparison) | 三列对比表格，高亮差异 |
| "展示项目进度" | `render_html` (kanban) | 看板视图 |
| "分析销售数据" | `visualize_data` (bar+line) | 交互式图表 |
| "梳理产品功能结构" | `render_html` (mindmap) | 可折叠思维导图 |
| "帮我算一下 ROI" | `calculate` | 精确计算结果 |
| "现在纽约几点" | `get_datetime` | 带时区的时间显示 |

---

## 四、实现策略

### 后端 → 前端协议

工具返回结构化数据，前端根据 `render_type` 路由到对应 React 组件：

```rust
ToolResult {
    status: Ok,
    data: json!({
        "render_type": "html_component|document|chart",
        "component": "comparison",
        "payload": { ... }
    })
}
```

### 前端渲染流程

```
LLM 返回 ToolUse → AgentLoop 执行工具 → 工具返回 render_type → 
前端识别 render_type → 调用对应 React 组件 → 渲染交互式内容
```

### 安全考虑

- HTML 渲染使用 DOMPurify 或类似库做 XSS 过滤
- 不允许内联 script
- 只允许白名单内的 HTML 标签和属性
- CSS 使用 scoped/tailwind 类名，禁止任意 style

---

## 五、待确认问题

1. **P0 工具是否确认？** `render_html` + `create_document` + `visualize_data`
2. **calculate 是否必要？** LLM 本身数学能力在提升，但精确计算仍有价值
3. **execute_python 的安全沙箱？** 需要 Docker/WASM 隔离，实现成本高
4. **前端渲染组件库？** 是否引入 ECharts / Mermaid.js / 自研组件
5. **文档编辑是否持久化？** `create_document` 生成的文档是否保存到后端

---

## 六、确认后下一步

1. 确认工具清单 → 编写 ToolSpec JSON Schema
2. 实现工具 execute() 方法
3. 前端实现对应 React 渲染组件
4. 添加端到端测试
