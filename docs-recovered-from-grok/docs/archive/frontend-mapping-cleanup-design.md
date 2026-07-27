# Raw→Domain Mapping 透传层清理设计方案

> ⚠️ **ARCHIVED 2026-06-13** — 本文档已实现并归档。
> 实际删除：`lib/workspace/client.ts` 的 `RawWorkspace*` 类型与 `mapWorkspace*` 函数（共 8 个）、`lib/workspace/stream.ts` 的孤儿 `ChatEvent`。`WorkspaceChatMessage.answer_blocks/citations` 改为 optional。
> 现行交付记录：`docs/agents/ARCHITECTURE-REVIEW-2026-06-09-SUMMARY.md`（§2.5）。

> Status: **Implemented** (2026-06-09, commit `ba05601`)
> Author: 架构审核讨论（候选 7：Raw→Domain Mapping 透传层）  
> Date: 2026-06-09  
> Related: `lib/workspace/client.ts` (780 行), `lib/workspace/stream.ts` (560 行)

---

## 1. 背景与动机

### 1.1 `client.ts`：双类型层次 + 1:1 映射函数

`lib/workspace/client.ts` 维护了两套平行的类型层次：

- **`RawWorkspace*`**（line 142-259）：后端响应的原始形状，字段名与后端 API 一致（`id`, `notebook_id`）
- **`Workspace*`**（line 9-136）：前端 domain 形状，字段名与前端概念对齐（`workspace_id`, `workspace_name`）

以及 8 个 `mapWorkspace*` 函数（line 306-414），执行 1:1 字段复制 + null-coalescing：

```typescript
function mapWorkspaceChatMessage(message: RawWorkspaceChatMessage): WorkspaceChatMessage {
  return {
    id: message.id,
    session_id: message.session_id,
    role: message.role,
    content: message.content,
    answer_blocks: message.answer_blocks ?? [],
    agent_id: message.agent_id ?? null,
    // ... 纯机械复制
  };
}
```

**映射层的问题**：
1. **无领域价值**：映射函数不做任何有意义的转换（不计算、不过滤、不聚合），只是复制字段并填充默认值；
2. **三重维护负担**：后端新增字段时，需要同步修改 `Raw*` 类型、`Workspace*` 类型、`map*` 函数，一处遗漏即产生 bug；
3. **已有数据丢失案例**：`tool_results` 字段在 `WorkspaceChatMessage` 中存在，但在 `RawWorkspaceChatMessage` 中缺失，导致后端返回的 `tool_results` 在反序列化时被静默丢弃。

### 1.2 `stream.ts`：多余的 `ChatEvent` 类型

`lib/workspace/stream.ts` 定义了两个事件类型：

- **`ChatEvent`**（line 149-209）：后端 SSE shape，字段名 `event`
- **`WorkspaceChatStreamEvent`**（line 211-271）：前端 shape，字段名 `kind`

经 grep 确认：**`ChatEvent` 零外部 consumer**，没有任何代码导入或使用它。`decodeChatEvent` 函数直接解析原始 JSON 为 `WorkspaceChatStreamEvent`，不经过 `ChatEvent`。

这是一个**定义了但从未被使用的孤儿类型**。

---

## 2. 设计原则

| 原则 | 说明 |
|------|------|
| **删除透传层** | 如果映射函数的复杂度 ≈ 被映射的数据结构，它就是透传层。 |
| **默认值下推** | optional 字段的默认值（`?? []`、`?? null`）应在组件消费时处理，而非在 API 层统一填充。 |
| **字段重命名内联** | `id` → `workspace_id` 的转换保留在 API 函数内部，不需要独立的映射函数。 |
| **类型与运行时一致** | 让 TypeScript 类型直接反映后端 API 的响应形状，减少中间抽象。 |

---

## 3. `client.ts` 清理方案

### 3.1 删除内容

| 删除项 | 行数 | 说明 |
|--------|------|------|
| `RawWorkspace` 类型 | ~13 行 | 原始 workspace shape |
| `RawWorkspaceResponse` 类型 | ~3 行 | 原始响应包装 |
| `RawWorkspaceSession` 类型 | ~9 行 | 原始 session shape |
| `RawWorkspaceChatMessage` 类型 | ~12 行 | 原始 message shape |
| `RawWorkspaceSource` 类型 | ~7 行 | 原始 source shape |
| `RawWorkspaceNote` 类型 | ~10 行 | 原始 note shape |
| `RawWorkspaceParsedPreview*` 类型 | ~15 行 | 原始 preview shape |
| `RawWorkspaceSourceContentResponse` 类型 | ~3 行 | 原始 content shape |
| `RawWorkspaceCitationLookupResponse` 类型 | ~10 行 | 原始 citation shape |
| `mapWorkspace` 函数 | ~15 行 | 1:1 映射 |
| `mapWorkspaceSession` 函数 | ~12 行 | 1:1 映射 |
| `mapWorkspaceChatMessage` 函数 | ~15 行 | 1:1 映射 |
| `mapWorkspaceSource` 函数 | ~10 行 | 1:1 映射 |
| `mapWorkspaceNote` 函数 | ~13 行 | 1:1 映射 |
| `mapWorkspaceParsedPreviewResponse` 函数 | ~13 行 | 1:1 映射 |
| `mapWorkspaceSourceContentResponse` 函数 | ~7 行 | 1:1 映射 |
| `mapWorkspaceCitationLookupResponse` 函数 | ~13 行 | 1:1 映射 |

**总计删除**：~230 行（类型 82 行 + 映射函数 110 行 + 相关类型 38 行）。

### 3.2 修改 `Workspace*` 类型定义

将 `Workspace*` 类型中的 required 字段恢复为 optional（与后端一致），让默认值处理下推到消费层：

```typescript
// Before（当前）
export type WorkspaceChatMessage = {
  id: number;
  session_id: string;
  role: string;
  content: string;
  answer_blocks: AnswerBlock[];        // required，但后端是 optional
  agent_id?: string | null;
  agent_name?: string | null;
  agent_icon?: string | null;
  citations: Citation[];               // required，但后端是 optional
  tool_results?: ToolResult[] | null;
  created_at: string;
};

// After（清理后）
export type WorkspaceChatMessage = {
  id: number;
  session_id: string;
  role: string;
  content: string;
  answer_blocks?: AnswerBlock[];       // optional，与后端一致
  agent_id?: string | null;
  agent_name?: string | null;
  agent_icon?: string | null;
  citations?: Citation[];              // optional，与后端一致
  tool_results?: ToolResult[] | null;
  created_at: string;
};
```

### 3.3 API 函数的内联转换

对于字段重命名（`id` → `workspace_id`、`notebook_id` → `workspace_id`），在 API 函数内部做最小内联转换：

```typescript
// Before（当前）
export async function getWorkspace(token: string, workspace_id: string): Promise<WorkspaceResponse> {
  const resp = await request<RawWorkspaceResponse>(...);
  return { workspace: mapWorkspace(resp.notebook) };
}

// After（清理后）
export async function getWorkspace(token: string, workspace_id: string): Promise<WorkspaceResponse> {
  const resp = await request<{ notebook: Workspace }>(...);
  return {
    workspace: {
      ...resp.notebook,
      workspace_id: resp.notebook.id,  // 内联重命名
    }
  };
}
```

```typescript
// Before（当前）
export async function listWorkspaceSessions(token: string, workspace_id: string): Promise<WorkspaceSessionListResponse> {
  const resp = await request<RawWorkspaceSessionListResponse>(...);
  return { sessions: resp.sessions.map(mapWorkspaceSession) };
}

// After（清理后）
export async function listWorkspaceSessions(token: string, workspace_id: string): Promise<WorkspaceSessionListResponse> {
  const resp = await request<{ sessions: Array<{ notebook_id: string; title?: string | null; ... }> }>(...);
  return {
    sessions: resp.sessions.map((s) => ({
      ...s,
      workspace_id: s.notebook_id,  // 内联重命名
    }))
  };
}
```

对于无字段重命名的 API（如 `listWorkspaceNotes`、`lookupWorkspaceCitation`），直接消费后端响应：

```typescript
// Before（当前）
export async function listWorkspaceNotes(...) {
  const resp = await request<RawWorkspaceNoteListResponse>(...);
  return { notes: resp.notes.map(mapWorkspaceNote) };
}

// After（清理后）
export async function listWorkspaceNotes(...) {
  const resp = await request<WorkspaceNoteListResponse>(...);
  return resp;  // 无需转换，直接透传
}
```

### 3.4 消费层的默认值处理

将 `?? []`、`?? null` 等默认值从 API 层下推到组件消费层：

```typescript
// 在 workspace-chat-pane.tsx 的 mapTranscriptMessage 中
function mapTranscriptMessage(message: WorkspaceChatMessage): PaneMessage {
  return {
    ...
    answerBlocks: message.answer_blocks ?? [],  // 默认值在这里处理
    citations: message.citations ?? [],
    toolResults: message.tool_results ?? [],
    ...
  };
}
```

---

## 4. `stream.ts` 清理方案

### 4.1 删除 `ChatEvent` 类型

```typescript
// 删除以下全部内容（line 149-209）
export type ChatEvent =
  | { event: "start"; request_id: string; session_id: string; }
  | { event: "activity"; ... }
  | { event: "answer_start"; ... }
  | { event: "trace"; ... }
  | { event: "token"; ... }
  | { event: "reasoning_summary_delta"; ... }
  | { event: "citations"; ... }
  | { event: "done"; ... }
  | { event: "error"; ... };
```

`decodeChatEvent` 函数保持不变——它直接解析原始 JSON 为 `WorkspaceChatStreamEvent`，不使用 `ChatEvent`。

### 4.2 验证无 consumer

`grep "ChatEvent" frontend_next/` 结果：
- `stream.ts:149` — 定义处
- 零外部导入

确认可安全删除。

---

## 5. 收益

| 指标 | Before | After |
|------|--------|-------|
| `client.ts` 行数 | 780 | ~550（-230 行） |
| `stream.ts` 行数 | 560 | ~500（-60 行） |
| 类型定义套数 | 2 套（Raw + Workspace） | 1 套（Workspace） |
| 映射函数数量 | 8 个 | 0 个 |
| 后端字段变更时的修改点 | 3 处（Raw 类型、Workspace 类型、map 函数） | 1 处（Workspace 类型） |
| `tool_results` 数据丢失风险 | 有（Raw 类型缺失字段） | 无（单一类型定义） |

---

## 6. 风险评估

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| `Workspace*` 类型 optional 字段导致组件中出现 `undefined` 错误 | 中 | 运行时错误 | TypeScript 编译会捕获大部分；消费层已使用 `??` 处理默认值（如 `mapTranscriptMessage`） |
| 字段重命名遗漏（如 `notebook_id` 未改为 `workspace_id`） | 低 | 数据不正确 | 编译时 `WorkspaceSession.workspace_id` 类型检查会捕获 |
| 删除 `ChatEvent` 导致某处编译失败 | 极低 | 编译错误 | grep 已确认零 consumer |

---

## 7. 实施 Checklist

### `client.ts`

- [ ] 修改 `Workspace*` 类型定义：将 required 但后端 optional 的字段恢复为 optional
- [ ] 添加 `tool_results` 到 `WorkspaceChatMessage`（如果确认后端已返回）
- [ ] 删除所有 `RawWorkspace*` 类型定义
- [ ] 删除所有 `mapWorkspace*` 函数
- [ ] 修改 API 函数：使用 `Workspace*` 类型替代 `Raw*` 类型
- [ ] 在需要字段重命名的 API 函数中添加内联转换（`id` → `workspace_id`，`notebook_id` → `workspace_id`）
- [ ] 检查消费层（`workspace-chat-pane.tsx`、`workspace-history-pane.tsx` 等）是否需要添加 `??` 默认值
- [ ] 运行 TypeScript 编译，修复所有类型错误
- [ ] 运行 Vitest 单元测试
- [ ] 运行 E2E 测试验证 API 调用正常

### `stream.ts`

- [ ] 删除 `ChatEvent` 类型定义（line 149-209）
- [ ] 运行 TypeScript 编译确认无错误
- [ ] 运行 `stream.test.ts` 测试

---

## 8. 可选：Zod 验证（未来扩展）

本次方案**不引入 Zod**，因为当前改动已经足够消除透传层。但如果未来需要更严格的运行时验证（如后端 API 版本升级时），可以考虑用 Zod schema 替代 TypeScript 类型：

```typescript
// 未来扩展（不在本次实施）
import { z } from "zod";

const WorkspaceChatMessageSchema = z.object({
  id: z.number(),
  session_id: z.string(),
  role: z.string(),
  content: z.string(),
  answer_blocks: z.array(AnswerBlockSchema).optional(),
  citations: z.array(CitationSchema).optional(),
  tool_results: z.array(ToolResultSchema).optional().nullable(),
  created_at: z.string(),
});

export type WorkspaceChatMessage = z.infer<typeof WorkspaceChatMessageSchema>;
```

Zod 可以在一步中完成类型定义 + 运行时验证 + 默认值填充，彻底消除"类型与实际数据不一致"的风险。但这需要新增依赖，本次不涉及。
