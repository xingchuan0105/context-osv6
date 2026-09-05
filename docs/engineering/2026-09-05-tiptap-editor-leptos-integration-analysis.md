# Tiptap 富文本/Markdown 编辑器在 Leptos (Rust Web) 下的集成方案与风险评估

日期：2026-09-05  
执行切片：E1.5  
目标归属：W3.2（工作区笔记编辑器 `/dashboard/:id`）

---

## 1. 现状与依赖调研

在现行 `frontend_next` 中，工作区笔记编辑器实现位于 `frontend_next/components/workspace/workspace-note-editor-tiptap.tsx`。
其核心功能由以下包提供：
- `@tiptap/core`：ProseMirror 的现代封装，纯 JavaScript / TypeScript 核心对象 `Editor`。
- `@tiptap/starter-kit`：段落 (p)、标题 (h1, h2)、无序列表 (bulletList)、有序列表 (orderedList)、粗体 (bold)、斜体 (italic)、链接 (link)。
- `@tiptap/markdown`：基于 `prosemirror-markdown`，提供双向 Markdown 序列化与反序列化 (`editor.storage.markdown.getMarkdown()`)。
- `@tiptap/extensions`：`Placeholder`、`UndoRedo`。
- `@tiptap/react`：仅提供 React 的 `useEditor` 钩子与 `<EditorContent />` 组件，代码量极少，仅做 DOM 挂载和状态订阅。

**核心发现**：Tiptap 的底层引擎 **完全不依赖 React**。它是一个纯 DOM 驱动的富文本编辑器框架。

---

## 2. 为什么严禁“手写 contenteditable”

在 Web 端实现富文本与 Markdown 混排编辑器时，直接用原生 `<div contenteditable>` 存在以下已知致命风险：
1. **中文 IME 输入法合成断裂**：中文输入法在 composition 期间直接修改 DOM，手写事件监听会破坏选区，导致文字重复或拼音字母泄漏。
2. **跨浏览器换行规范不一致**：Chrome 插入 `<div><br></div>`，Safari 插入 `<p><br></p>` 或 `<div>`, Firefox 插入 `<br>`，导致 Markdown 序列化极其脆弱。
3. **Undo/Redo 历史栈撕裂**：原生 `execCommand` 已废弃，原生 Undo 栈无法捕获程序化的 Markdown 转换。
4. **粘贴清洗（Paste Sanitation）**：从 Word、网页或飞书复制的内容会带入大量垃圾样式与不可控标签。

因此，**必须保持复用经过实战检验的 Tiptap / ProseMirror 核心，严禁为了纯 Rust 而手写 contenteditable**。

---

## 3. Leptos + Tiptap 架构集成方案 (Vanilla JS Bridge)

### 3.1 边界设计原则
- **Leptos 掌管**：外壳容器、工具栏按钮（Bold/Italic/List/Undo/Redo）、工作区状态（保存中/已保存）、i18n 文案、快捷键转发。
- **Tiptap 掌管**：DOM 编辑区内的 ProseMirror 文档模型、输入法合成保护、Markdown 解析与序列化、本地选区计算。

### 3.2 最小 Bridge 接口 (JS Bundle)
利用现有的 `esbuild` 将纯 `@tiptap/core` + `@tiptap/starter-kit` + `@tiptap/markdown` 打包为单个独立的 ESM 模块（例如 `tiptap-editor-bridge.mjs`，托管于 `/pkg/`）：

```typescript
export interface NoteEditorBridge {
  destroy(): void;
  setMarkdown(markdown: string): void;
  getMarkdown(): string;
  focus(): void;
  toggleBold(): void;
  toggleItalic(): void;
  setHeading(level: 1 | 2): void;
  setParagraph(): void;
  toggleBulletList(): void;
  toggleOrderedList(): void;
  undo(): void;
  redo(): void;
  getState(): {
    bold: boolean;
    italic: boolean;
    blockStyle: 'p' | 'h1' | 'h2';
    canUndo: boolean;
    canRedo: boolean;
  };
}

export function initNoteEditor(
  mountEl: HTMLElement,
  initialMarkdown: string,
  onUpdate: (markdown: string) => void,
  onSelectionChange: (state: unknown) => void,
): NoteEditorBridge {
  const editor = new Editor({
    element: mountEl,
    extensions: [
      StarterKit.configure({
        heading: { levels: [1, 2] },
        link: { openOnClick: false },
        undoRedo: false,
      }),
      Markdown,
      Placeholder.configure({ placeholder: '开始记录...' }),
      UndoRedo.configure({ depth: 100 }),
    ],
    content: initialMarkdown,
    onUpdate({ editor }) {
      onUpdate(editor.storage.markdown.getMarkdown());
    },
    onSelectionUpdate({ editor }) {
      onSelectionChange(editor.getAttributes(...));
    }
  });

  return {
    destroy: () => editor.destroy(),
    setMarkdown: (md) => editor.commands.setContent(md),
    getMarkdown: () => editor.storage.markdown.getMarkdown(),
    // ...
  };
}
```

### 3.3 Leptos 组件挂载模式
在 `frontend_rust/crates/web-ui/src/components/notes/note_editor.rs` 中：
```rust
#[component]
pub fn NoteEditor(
    initial_content: String,
    on_change: Callback<String>,
) -> impl IntoView {
    let editor_container_ref = NodeRef::<leptos::html::Div>::new();
    
    #[cfg(target_arch = "wasm32")]
    {
        Effect::new(move |_| {
            if let Some(container) = editor_container_ref.get() {
                // 通过 js-sys / wasm-bindgen 动态调用 initNoteEditor
                // 监听 Rust 回调，双向同步变更
            }
        });
    }

    view! {
        <div class="note-editor-wrapper">
            <div class="note-editor-toolbar">
                // 按钮调用 editor_bridge 方法
            </div>
            <div node_ref=editor_container_ref class="note-editor-content" />
        </div>
    }
}
```

---

## 4. 关键风险与应对策略

| 风险项 | 影响度 | 应对措施 |
|---|---|---|
| **Markdown 往返一致性 (Roundtrip)** | 中 | 复用相同的 `@tiptap/markdown`，保证标题、列表、代码块、链接的语法解析与 Next.js 完全一致。 |
| **SSR / Hydration 闪烁** | 低 | 笔记编辑属于交互性组件（`/dashboard/[id]` 登录后），SSR 输出占位骨架或只读纯文本，客户端水合完成后无缝接管为 Tiptap。 |
| **打包体积** | 低 | 剥离了 React 与 React-DOM 绑定后，纯 Tiptap + ProseMirror 压缩后约 50-70KB（Brotli 压缩后更小），远低于完整 Next 前端包。 |
| **内存泄露** | 中 | 切换笔记或离开页面时，在 Leptos `on_cleanup` 中显式调用 `bridge.destroy()`，释放 ProseMirror 的 DOM 监听与模型节点。 |

---

## 5. 结论

**E1.5 验证通过**：
1. Tiptap 在 Rust/Leptos 下的集成技术路径明确，无需造轮子，无需手写 contenteditable。
2. 采用纯 JS Bridge（剔除 React 绑定）的方案既保全了 ProseMirror 的全部排版、快捷键、撤销与 IME 稳定性，又完美契合 Leptos 的组件生命周期。
3. 阻断项为 **0**，可以在 E3.2（工作区控制台）中放心按此方案推进。
