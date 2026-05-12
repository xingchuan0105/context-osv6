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

## 五、已确认决策

| 问题 | 决策 |
|------|------|
| 1. P0 工具 | **确认** — `render_html` + `create_document` + `visualize_data` |
| 2. `calculate` | **需要** — 精确计算仍有价值 |
| 3. `execute_python` | **不需要** — 安全沙箱成本高，暂不做 |
| 4. 前端组件库 | **用成熟库** — ECharts、Mermaid.js 等，不自研 |
| 5. 文档持久化 | **不持久化** — `create_document` 纯前端展示 |

---

## 六、实现计划

### Phase 1: 后端工具实现
- [ ] `render_html` — 返回结构化组件数据
- [ ] `create_document` — 返回文档结构化数据
- [ ] `visualize_data` — 返回图表配置数据
- [ ] `calculate` — 数学计算（安全 eval）

### Phase 2: 前端渲染组件
- [ ] ECharts 图表组件
- [ ] Mermaid 图表组件
- [ ] 文档/卡片/看板组件
- [ ] 工具结果识别与路由

### Phase 3: 集成测试
- [ ] ChatAgent 工具调用端到端测试

---

## 七、全量工具清单（统一实现时参考）

### ChatAgent 工具（非 RAG/非搜索）

| # | 工具名 | 类别 | 用途 | 优先级 |
|---|--------|------|------|--------|
| 1 | `load_skill` | 基础 | 加载领域 skill 文件 | P1 |
| 2 | `compact_history` | 基础 | 压缩对话历史 | P1 |
| 3 | `render_html` | 渲染 | 交互式 HTML 组件（图表/表格/看板等） | **P0** |
| 4 | `create_document` | 渲染 | 生成可编辑结构化文档 | **P0** |
| 5 | `visualize_data` | 渲染 | 数据可视化（柱状图/折线图/饼图等） | **P0** |
| 6 | `calculate` | 计算 | 精确数学计算 | P1 |
| 7 | `get_datetime` | 计算 | 获取当前时间/时区转换 | P1 |
| 8 | `parse_csv` | 处理 | 解析 CSV 数据 | P2 |
| 9 | `render_mermaid` | 渲染 | Mermaid 图表（流程图/时序图） | P2 |

### RagAgent 工具（RAG 检索）

| # | 工具名 | 用途 | 状态 |
|---|--------|------|------|
| 1 | `load_skill` | 加载领域 skill 文件 | Stub |
| 2 | `compact_history` | 压缩对话历史 | Stub |
| 3 | `dense_retrieval` | 语义检索 | Stub（需接 RagRuntime） |
| 4 | `lexical_retrieval` | 关键词检索 | Stub（需接 RagRuntime） |
| 5 | `graph_retrieval` | 知识图谱遍历 | Stub（需接 RagRuntime） |
| 6 | `doc_summary` | 文档摘要 | Stub（需接 RagRuntime） |
| 7 | `index_lookup` | 精确块查找 | Stub（需接 RagRuntime） |
| 8 | `doc_metadata` | 文档元数据 | Stub（需接 RagRuntime） |

### WebSearchAgent 工具（网络搜索）

| # | 工具名 | 用途 | 状态 |
|---|--------|------|------|
| 1 | `load_skill` | 加载领域 skill 文件 | Stub |
| 2 | `compact_history` | 压缩对话历史 | Stub |
| 3 | `brave_search` | Brave 搜索 | Stub（需接 SearchProvider） |
| 4 | `fetch_full_page` | 获取网页全文 | Stub（需接 SearchProvider） |

---

## 八、统一实现优先级

### 第一批（立即）
- ChatAgent: `render_html`, `create_document`, `visualize_data`
- ChatAgent: `calculate`

### 第二批（随后）
- RagAgent: 6 个 RAG 工具接 RagRuntime
- WebSearchAgent: 2 个搜索工具接 SearchProvider

### 第三批（可选）
- `get_datetime`, `parse_csv`, `render_mermaid`
