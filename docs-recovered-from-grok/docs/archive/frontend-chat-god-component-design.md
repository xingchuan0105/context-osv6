# Frontend Chat God Component 拆分设计方案

> ⚠️ **ARCHIVED 2026-06-13** — 本文档已实现并归档。
> 实际拆分：`workspace-chat-pane.tsx` 2514 → 174 行，抽出 `hooks/use-chat-session.ts`、`components/workspace/chat-composer.tsx`、`components/workspace/chat-message-list.tsx`。修复 `useEffect` 依赖导致的 OOM 渲染循环。
> 现行交付记录：`docs/agents/ARCHITECTURE-REVIEW-2026-06-09-SUMMARY.md`（§2.6）。

> Status: **Implemented** (2026-06-09, commit `ba05601`)
> Author: 架构审核讨论（候选 6：Chat Pane God Component）  
> Date: 2026-06-09  
> Related: `workspace-chat-pane.tsx` (2514 行), `shared-workspace-surface.tsx` (502 行)

---

## 1. 背景与动机

### 1.1 问题陈述

`WorkspaceChatPane`（`components/workspace/workspace-chat-pane.tsx`）是一个 **2514 行的 God Component**。它内嵌了以下 7 个独立关注点：

| 关注点 | 代码范围 | 说明 |
|--------|----------|------|
| A. 聊天会话管理 | ~140 行 | 加载历史消息、切换 session、session ID 同步 |
| B. SSE 流生命周期 | ~70 行 | `streamWorkspaceChat` 调用、`handleStreamEvent` 事件分发 |
| C. Typewriter 动画引擎 | ~90 行 | 队列管理、定时器调度、reduce-motion 适配、done 事件延迟 |
| D. 消息累积 | ~120 行 | `ensureStreamingAssistant`、`updateStreamingAssistant`、`finalizeStreamingDone` |
| E. Progress UI 状态 | ~40 行 | 活动累积、mode 追踪、卡片显隐 |
| F. Composer UI | ~120 行 | 输入框、mode menu、resize handle、keyboard 快捷键 |
| G. 消息渲染 | ~350 行 | Markdown、citation tokenizer、answer blocks、tool results、HTML fallback |

这些关注点全部耦合在同一个组件的作用域内，通过 11 个 `useState` 和 10 个 `useRef` 共享状态。

### 1.2 核心痛点

1. **无法单元测试**：流协议处理逻辑（`handleStreamEvent`）与 React 渲染生命周期交织，无法脱离组件进行测试。
2. **状态双轨制**：`streamingMessageId`（state）与 `streamingMessageIdRef`（ref）并存，存在同步风险。
3. **代码重复**：`shared-workspace-surface.tsx`（502 行）被迫独立实现了一套简化版流处理（`handleStreamEvent` + `handleSubmit`），不共享任何逻辑。
4. **新功能阻力大**：任何涉及流协议的改动（如新增事件类型）都需要在 2514 行中逐行排查影响面。

### 1.3 拆分原则

> **按"关注点"拆分，不是按"视觉区域"拆分。**

目标是让 `WorkspaceChatPane` 从一个"做所有事的组件"变为一个"编排小组件的薄壳"。真正复杂的行为应该隐藏在 deep module（自定义 hook）后面。

---

## 2. 核心设计：`useChatSession` Hook

### 2.1 定位

`useChatSession` 是**聊天引擎的单一接口**。它封装了关注点 A+B+C+D+E（会话管理 + 流生命周期 + typewriter + 消息累积 + progress），将 460 行内部逻辑隐藏在一个小接口后面。

### 2.2 接口

```typescript
// hooks/use-chat-session.ts

export type ChatMessage = {
  id: string;
  role: "user" | "assistant";
  mode: WorkspaceChatMode | null;
  content: string;
  answerBlocks: AnswerBlock[];
  citations: Citation[];
  degradeTrace: DegradeTraceItem[];
  guarded: boolean;
  messageId: number | null;
  pending?: boolean;
  sessionId: string | null;
  toolResults: ToolResult[];
};

export type UseChatSessionOptions = {
  token: string;
  workspaceId: string;
  sessionId: string | null;
  selectedSourceIds: string[];
  effectiveChatMode: WorkspaceChatMode;
  locale: "zh-CN" | "en";
  onSessionChange?: (sessionId: string | null) => void;
  onSessionActivity?: () => void;
};

export type UseChatSessionResult = {
  // 状态（只读）
  messages: ChatMessage[];
  isStreaming: boolean;
  streamingMessageId: string | null;
  progress: {
    activities: ProgressEntry[];
    mode: WorkspaceChatMode | null;
    collapsed: boolean;
  };
  error: string | null;

  // 操作
  send: (query: string) => void;
  stop: () => void;
  toggleProgressCollapsed: () => void;
  updateMessage: (messageId: string, updater: (msg: ChatMessage) => ChatMessage) => void;
  loadSession: (sessionId: string) => Promise<void>;
  reset: () => void;
};

export function useChatSession(options: UseChatSessionOptions): UseChatSessionResult;
```

### 2.3 内部职责划分

`useChatSession` 内部由三个私有 hook 组成：

```
useChatSession
├── useMessageHistory      // 关注点 A：加载/管理消息列表
├── useChatStream          // 关注点 B+C+D：SSE 连接 + typewriter + 消息累积
└── useProgressTracker     // 关注点 E：progress 活动累积
```

**为什么拆成三个内部 hook 而不是一个**：
- 测试时可以单独测试 `useChatStream` 的事件处理逻辑（mock SSE 流）；
- `useProgressTracker` 可以独立测试活动累积规则；
- 但对外只暴露 `useChatSession` 一个接口（deep module）。

### 2.4 关键行为：Typewriter 动画

当前 `WorkspaceChatPane` 使用 refs 管理 typewriter 状态（`streamTypewriterQueueRef`、`streamDisplayedTextRef`、`pendingDoneEventRef`），以避免闭包问题。提取到 hook 后：

- 继续使用 refs 作为**定时器回调的缓存层**；
- 通过 `setMessages` 将结果同步到 React state；
- `useChatSession` 的消费者看不到 refs，只读到最终的 `messages`。

```typescript
// 内部实现示意（非最终代码）
function useChatStream(...) {
  const queueRef = useRef("");
  const displayedRef = useRef("");
  const pendingDoneRef = useRef<PendingDoneEvent | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // 暴露给 useChatSession 的回调
  const enqueueText = useCallback((text: string) => {
    // ... 队列逻辑
  }, []);

  const finalizeDone = useCallback((event: PendingDoneEvent) => {
    // ... done 处理 + typewriter 收尾
  }, []);

  return { enqueueText, finalizeDone, ... };
}
```

### 2.5 关键行为：消息累积

当前 `updateStreamingAssistant` 遍历 `messages` 数组查找匹配项，逻辑复杂（支持 `targetId` fallback 和 `pending` assistant 匹配）。提取到 hook 后：

- `useMessageHistory` 提供 `upsertAssistant(messageId, updater)`；
- `useChatStream` 在收到 `answer_start`/`token`/`citations`/`done` 时调用 `upsertAssistant`；
- 匹配逻辑集中在一处，组件不感知。

---

## 3. `WorkspaceChatPane` 拆分后的架构

### 3.1 目标结构

```
WorkspaceChatPane（薄壳，~300 行）
├── useChatSession()           // 聊天引擎
├── ChatComposer（组件）       // 输入框 + mode menu + resize
├── ChatMessageList（组件）    // 消息列表滚动容器
│   └── ChatMessage（组件）    // 单条消息（user / assistant）
│       └── AssistantAnswerContent（已有，保持不变）
├── ResearchProgressCard（已有，保持不变）
└── ToolResultsPanel（已有，保持不变）
```

### 3.2 薄壳组件的职责

`WorkspaceChatPane` 拆分后只负责：

1. **调用 `useChatSession`** 获取状态和操作；
2. **管理纯 UI 状态**：`draft`、`showModeMenu`、`modeMenuActiveIndex`、`composerTextareaHeight`、`isComposerResizing`；
3. **事件桥接**：将 `ChatComposer` 的提交事件转发为 `useChatSession.send(draft)`；
4. **渲染编排**：将 `messages`、`progress`、`isStreaming` 传递给子组件。

### 3.3 状态归属表

| 状态 | 当前位置 | 拆分后位置 | 理由 |
|------|----------|------------|------|
| `messages` | `useState` in pane | `useChatSession` | 流生命周期核心 |
| `isStreaming` | `useState` in pane | `useChatSession` | 流生命周期核心 |
| `streamingMessageId` | `useState` + `useRef` in pane | `useChatSession`（内部 ref + state） | 流生命周期核心 |
| `progressActivities` | `useState` in pane | `useChatSession` | progress 追踪 |
| `progressMode` | `useState` + `useRef` in pane | `useChatSession` | progress 追踪 |
| `progressCollapsed` | `useState` in pane | `useChatSession` | 虽为 UI 状态，但与流强相关 |
| `error` | `useState` in pane | `useChatSession` | 流错误处理 |
| `draft` | `useState` in pane | **保留在 pane** | 纯 UI 输入状态 |
| `showModeMenu` | `useState` in pane | **保留在 pane** | 纯 UI 弹窗状态 |
| `modeMenuActiveIndex` | `useState` in pane | **保留在 pane** | 纯 UI 导航状态 |
| `composerTextareaHeight` | `useState` in pane | **保留在 pane** | 纯 UI 尺寸状态 |
| `isComposerResizing` | `useState` in pane | **保留在 pane** | 纯 UI 交互状态 |

### 3.4 `ChatComposer` 组件提取

从 `WorkspaceChatPane` 中提取 `ChatComposer`，接收以下 props：

```typescript
type ChatComposerProps = {
  draft: string;
  onDraftChange: (draft: string) => void;
  isStreaming: boolean;
  effectiveChatMode: WorkspaceChatMode;
  locale: "zh-CN" | "en";
  onSubmit: () => void;
  onStop?: () => void;
};
```

内部管理：`showModeMenu`、`modeMenuActiveIndex`、`composerTextareaHeight`、`isComposerResizing`、`textareaRef`。

**为什么提取**：Composer 有独立的交互逻辑（resize、mode menu、keyboard shortcuts），与消息列表和流处理无关。

### 3.5 `ChatMessageList` 组件提取

从 `WorkspaceChatPane` 中提取 `ChatMessageList`，接收以下 props：

```typescript
type ChatMessageListProps = {
  messages: ChatMessage[];
  progress: { activities: ProgressEntry[]; mode: WorkspaceChatMode | null; collapsed: boolean };
  isStreaming: boolean;
  locale: "zh-CN" | "en";
  onToggleProgressCollapsed: () => void;
  onSelectCitation: (request: WorkspaceCitationRequest) => void;
  onOpenWebSources: (request: WorkspaceWebSourcesRequest) => void;
  onCopyMessage: (content: string) => void;
  onEditMessage: (content: string) => void;
  onSubmitFeedback: (messageId: string, rating: "up" | "down") => void;
};
```

**为什么提取**：消息列表的渲染逻辑（message bubbles、actions、guard notices、web sources button）与 composer 和流引擎无关。

---

## 4. `SharedWorkspaceSurface` 的复用策略

### 4.1 现状分析

`SharedWorkspaceSurface` 的流处理逻辑（line 162-197）是一个**简化版**：

```typescript
// 当前实现（简化）
function handleStreamEvent(event) {
  switch (event.kind) {
    case "token": setStreamingAnswer(c => c + event.content); break;
    case "citations": setCitations(event.citations); break;
    case "done": /* finalize answer */ break;
    case "error": /* show error */ break;
    default: break;
  }
}
```

与 `WorkspaceChatPane` 的差异：

| 维度 | WorkspaceChatPane | SharedWorkspaceSurface |
|------|-------------------|------------------------|
| 消息历史 | ✅ 多轮对话 | ❌ 单次问答 |
| Typewriter | ✅ 有 | ❌ 无（直接 setState） |
| Progress Card | ✅ 有 | ❌ 无 |
| Answer Blocks | ✅ 支持 | ❌ 只取文本 |
| Tool Results | ✅ 渲染 | ❌ 忽略 |
| Citations | ✅ 内联 + 尾部 | ❌ 简化处理 |

### 4.2 复用方案

`SharedWorkspaceSurface` **不直接使用 `useChatSession`**，因为两者的流处理模型差异过大（多轮 vs 单轮、typewriter vs 实时、progress vs 无）。强行统一会让 `useChatSession` 接口膨胀。

推荐方案：**提取底层 `useChatStreamBase` hook**，只封装 SSE 连接 + 事件分发：

```typescript
// hooks/use-chat-stream-base.ts
// 最低公共层：SSE 连接管理 + AbortController + 原始事件回调

export function useChatStreamBase(options: {
  token: string;
  request: ChatRequest;
  onEvent: (event: WorkspaceChatStreamEvent) => void;
  onError: (error: Error) => void;
}): {
  start: () => void;
  stop: () => void;
  isStreaming: boolean;
};
```

- `WorkspaceChatPane`（通过 `useChatSession`）和 `SharedWorkspaceSurface` 都可以使用 `useChatStreamBase`；
- `useChatSession` 在内部调用 `useChatStreamBase`，然后在其上叠加 message history + typewriter + progress；
- `SharedWorkspaceSurface` 直接调用 `useChatStreamBase`，保持其简化模型。

**备选方案**：如果 `useChatStreamBase` 的抽象价值不高（它主要是 `streamWorkspaceChat` 的 thin wrapper），则**不提取底层 hook**，只提取 `useChatSession`。`SharedWorkspaceSurface` 保持现状，未来若功能扩展再考虑复用。

**推荐**：采用**备选方案**。`useChatSession` 是解决 God Component 的核心；`SharedWorkspaceSurface` 的 502 行和简化流模型不构成紧迫债务，不必为了"统一"而引入额外抽象。

---

## 5. 测试策略

### 5.1 `useChatSession` 单元测试

`useChatSession` 是纯 hook，可通过 `@testing-library/react` 的 `renderHook` 测试：

| 测试用例 | 验证内容 |
|----------|----------|
| `send` 添加 user message | `messages` 末尾出现 user message |
| `answer_start` 创建 pending assistant | `messages` 末尾出现 pending assistant |
| `token` 累积到 assistant content | assistant content 逐步增长 |
| `citations` 附加到 assistant | assistant citations 更新 |
| `done`  finalize assistant | assistant pending=false，answerBlocks 填充 |
| `error` 中断流 | `isStreaming=false`，`error` 有值 |
| typewriter 队列延迟 | token 不会立即出现在 content 中（reduce-motion 关闭时） |
| typewriter reduce-motion | token 立即出现在 content 中 |
| `stop` 中止请求 | AbortController 触发，流停止 |
| `loadSession` 加载历史 | messages 被替换为历史消息 |

### 5.2 组件测试

拆分后的组件测试更简单：

| 组件 | 测试内容 |
|------|----------|
| `ChatComposer` | 输入、提交、mode menu 打开/选择、keyboard shortcut、resize |
| `ChatMessageList` | 消息渲染、progress card 渲染、空状态 |
| `WorkspaceChatPane` | 集成：调用 `useChatSession`、状态桥接、事件传递 |

---

## 6. 实施步骤

### Phase 1：新建 `useChatSession`（不改动现有组件）

- [ ] 新建 `hooks/use-chat-session.ts`
- [ ] 实现 `useMessageHistory`（内部）
- [ ] 实现 `useChatStream`（内部）：SSE 事件处理 + typewriter + 消息累积
- [ ] 实现 `useProgressTracker`（内部）
- [ ] 组装 `useChatSession` 接口
- [ ] 新增单元测试（`use-chat-session.test.ts`），覆盖 §5.1 的用例

### Phase 2：拆分 `WorkspaceChatPane`

- [ ] 新建 `components/workspace/chat-composer.tsx`
- [ ] 新建 `components/workspace/chat-message-list.tsx`
- [ ] 修改 `WorkspaceChatPane`：
  - [ ] 调用 `useChatSession` 替代内部状态
  - [ ] 提取纯 UI 状态到组件层
  - [ ] 使用 `ChatComposer` 和 `ChatMessageList`
- [ ] 验证：流功能完全一致（E2E smoke 测试通过）

### Phase 3：清理与验证

- [ ] 删除 `WorkspaceChatPane` 中不再使用的内部函数和 state
- [ ] 运行 E2E 测试（`chat_smoke`, `journey` 相关 spec）
- [ ] 运行 Vitest 单元测试
- [ ] 代码 review：确认 `WorkspaceChatPane` 行数 < 500 行

---

## 7. 风险评估

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| Typewriter 动画在提取后行为变化 | 中 | 用户体验差异 | Phase 1 中单独测试 typewriter 逻辑；Phase 2 中对比 E2E 录像 |
| `updateStreamingAssistant` 的匹配逻辑在 hook 中出错 | 中 | 消息错位/丢失 | 单元测试覆盖 `answer_start` + `token` + `done` 的完整链路 |
| `SharedWorkspaceSurface` 意外受影响 | 低 | 共享链接页面故障 | Phase 2 只改动 `WorkspaceChatPane`，不触碰 `SharedWorkspaceSurface` |
| 子组件 props drilling 过深 | 低 | 组件接口复杂 | 若出现，用 React Context 传递 locale / callback refs |

---

## 8. 目标指标

| 指标 | 当前 | 目标 |
|------|------|------|
| `WorkspaceChatPane` 行数 | 2514 | < 500 |
| `WorkspaceChatPane` 中 `useState` 数量 | 11 | 5（纯 UI 状态） |
| `WorkspaceChatPane` 中 `useRef` 数量 | 10 | 2（DOM refs） |
| 流相关代码的可测试性 | 0%（无法单测） | 100%（通过 `useChatSession` 单测覆盖） |
| `shared-workspace-surface` 与 pane 的代码复用 | 0% | 不强制统一，保持独立简化模型 |
