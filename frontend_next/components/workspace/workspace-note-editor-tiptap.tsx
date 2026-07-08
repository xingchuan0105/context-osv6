"use client";

import { useEffect, useRef, useState, type ChangeEvent, type KeyboardEvent, type MouseEvent, type ReactNode } from "react";
import type { CSSProperties } from "react";

import { Placeholder, UndoRedo } from "@tiptap/extensions";
import { Markdown } from "@tiptap/markdown";
import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";

import { sanitizeWorkspaceHtml } from "./workspace-html-sanitize";
import styles from "./workspace-right-rail.module.css";

type EditorBlockStyle = "p" | "h1" | "h2";

type EditorToolbarState = {
  blockStyle: EditorBlockStyle;
  bold: boolean;
  italic: boolean;
  link: boolean;
  orderedList: boolean;
  unorderedList: boolean;
  canUndo: boolean;
  canRedo: boolean;
};

const DEFAULT_TOOLBAR_STATE: EditorToolbarState = {
  blockStyle: "p",
  bold: false,
  italic: false,
  link: false,
  orderedList: false,
  unorderedList: false,
  canUndo: false,
  canRedo: false,
};

type WorkspaceNoteEditorTiptapProps = {
  contentLabel: string;
  locale: string;
  onChange: (value: string) => void;
  placeholder: string;
  toolbarLabel: string;
  value: string;
};

type EditorInstance = NonNullable<ReturnType<typeof useEditor>>;

function normalizeLinkHref(value: string) {
  if (!value.trim()) {
    return "";
  }

  if (/^[a-z]+:\/\//i.test(value) || value.startsWith("mailto:") || value.startsWith("tel:")) {
    return value.trim();
  }

  return `https://${value.trim()}`;
}

function isLikelyUrl(value: string) {
  return /^(https?:\/\/|mailto:|tel:|www\.)\S+$/i.test(value.trim());
}

function plainTextToHtml(value: string) {
  const normalized = value.replace(/\r\n?/g, "\n").trim();

  if (!normalized) {
    return "";
  }

  const escapeHtml = (s: string) =>
    s
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");

  return normalized
    .split(/\n{2,}/)
    .map((paragraph) => `<p>${paragraph.split("\n").map((line) => escapeHtml(line)).join("<br>")}</p>`)
    .join("");
}

function resolveBlockStyle(editor: ReturnType<typeof useEditor>): EditorBlockStyle {
  if (!editor) {
    return "p";
  }

  if (editor.isActive("heading", { level: 1 })) {
    return "h1";
  }

  if (editor.isActive("heading", { level: 2 })) {
    return "h2";
  }

  return "p";
}

export function WorkspaceNoteEditorTiptap({
  contentLabel,
  locale,
  onChange,
  placeholder,
  toolbarLabel,
  value,
}: WorkspaceNoteEditorTiptapProps) {
  const [toolbarState, setToolbarState] = useState<EditorToolbarState>(DEFAULT_TOOLBAR_STATE);
  const [linkPanelOpen, setLinkPanelOpen] = useState(false);
  const [linkDraft, setLinkDraft] = useState("");
  const [linkPanelPosition, setLinkPanelPosition] = useState({
    arrowLeft: 32,
    left: 12,
    placement: "bottom" as "bottom" | "top",
    top: 52,
  });
  const onChangeRef = useRef(onChange);
  const valueRef = useRef(value);
  const lastSyncedMarkdownRef = useRef(value);
  const editorRef = useRef<EditorInstance | null>(null);
  const editorComposerRef = useRef<HTMLDivElement | null>(null);
  const editorToolbarRef = useRef<HTMLDivElement | null>(null);
  const linkPanelRef = useRef<HTMLDivElement | null>(null);
  const linkInputRef = useRef<HTMLInputElement | null>(null);
  const pendingLinkSelectionRef = useRef<{ from: number; to: number } | null>(null);
  const blockStyleOptions: Array<{ label: string; value: EditorBlockStyle }> = [
    { label: locale === "zh-CN" ? "正文" : "Normal", value: "p" },
    { label: locale === "zh-CN" ? "标题 1" : "Heading 1", value: "h1" },
    { label: locale === "zh-CN" ? "标题 2" : "Heading 2", value: "h2" },
  ];

  function syncToolbarState(nextEditor: EditorInstance | null = editorRef.current) {
    if (!nextEditor) {
      setToolbarState(DEFAULT_TOOLBAR_STATE);
      return;
    }

    setToolbarState({
      blockStyle: resolveBlockStyle(nextEditor),
      bold: nextEditor.isActive("bold"),
      italic: nextEditor.isActive("italic"),
      link: nextEditor.isActive("link"),
      orderedList: nextEditor.isActive("orderedList"),
      unorderedList: nextEditor.isActive("bulletList"),
      canUndo: nextEditor.can().chain().focus().undo().run(),
      canRedo: nextEditor.can().chain().focus().redo().run(),
    });
  }

  function updateLinkPanelPosition(nextEditor: EditorInstance | null = editorRef.current) {
    if (!nextEditor || !editorComposerRef.current) {
      return;
    }

    const composerRect = editorComposerRef.current.getBoundingClientRect();
    const toolbarHeight = editorToolbarRef.current?.offsetHeight ?? 0;
    const panelWidth = linkPanelRef.current?.offsetWidth ?? 320;
    const panelHeight = linkPanelRef.current?.offsetHeight ?? 104;
    const horizontalPadding = 12;
    const pendingSelection = pendingLinkSelectionRef.current;

    if (!pendingSelection) {
      setLinkPanelPosition({
        arrowLeft: 32,
        left: Math.max(horizontalPadding, composerRect.width - panelWidth - horizontalPadding),
        placement: "bottom",
        top: toolbarHeight + 10,
      });
      return;
    }

    const startCoords = nextEditor.view.coordsAtPos(pendingSelection.from);
    const endCoords = nextEditor.view.coordsAtPos(pendingSelection.to);
    const anchorX = ((startCoords.left + endCoords.right) / 2) - composerRect.left;
    const maxLeft = Math.max(horizontalPadding, composerRect.width - panelWidth - horizontalPadding);
    const left = Math.min(Math.max(anchorX - panelWidth / 2, horizontalPadding), maxLeft);
    const arrowLeft = Math.min(Math.max(anchorX - left, 22), panelWidth - 22);
    const selectionTop = Math.min(startCoords.top, endCoords.top) - composerRect.top;
    const selectionBottom = Math.max(startCoords.bottom, endCoords.bottom) - composerRect.top;
    const preferredTop = selectionTop - panelHeight - 8;
    const fallbackTop = selectionBottom + 8;
    const placement = preferredTop >= toolbarHeight + 10 ? "top" : "bottom";
    const top = placement === "top" ? preferredTop : fallbackTop;

    setLinkPanelPosition({ arrowLeft, left, placement, top });
  }

  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        blockquote: false,
        code: false,
        codeBlock: false,
        heading: {
          levels: [1, 2],
        },
        horizontalRule: false,
        link: {
          enableClickSelection: true,
          openOnClick: false,
        },
        strike: false,
        underline: false,
        undoRedo: false,
      }),
      Markdown,
      Placeholder.configure({
        placeholder,
      }),
      UndoRedo.configure({
        depth: 100,
        newGroupDelay: 500,
      }),
    ],
    content: value,
    contentType: "markdown",
    editorProps: {
      attributes: {
        "aria-label": contentLabel,
        class: styles.editorProseMirror,
        spellcheck: "true",
      },
      handleKeyDown(_view, event): boolean {
        const nextEditor = editorRef.current;

        if (!nextEditor) {
          return false;
        }

        if (event.key === "Enter" && event.shiftKey && !event.altKey && !event.ctrlKey && !event.metaKey) {
          return nextEditor.chain().focus().insertContent("<br>", { contentType: "html" }).run();
        }

        if (
          event.key === "Enter" &&
          !event.shiftKey &&
          !event.altKey &&
          !event.ctrlKey &&
          !event.metaKey &&
          nextEditor.isActive("heading")
        ) {
          const { empty, $from } = nextEditor.state.selection;

          if (empty && $from.parentOffset === $from.parent.content.size) {
            return nextEditor.chain().focus().splitBlock().setParagraph().run();
          }
        }

        return false;
      },
      handlePaste(_view, event): boolean {
        const nextEditor = editorRef.current;

        if (!nextEditor || !event.clipboardData) {
          return false;
        }

        const text = event.clipboardData.getData("text/plain");
        const html = event.clipboardData.getData("text/html");

        if (text.trim() && !nextEditor.state.selection.empty && isLikelyUrl(text)) {
          event.preventDefault();
          nextEditor.chain().focus().extendMarkRange("link").setLink({ href: normalizeLinkHref(text) }).run();
          syncToolbarState(nextEditor);
          return true;
        }

        if (html.trim()) {
          const sanitizedHtml = sanitizeWorkspaceHtml(html);

          if (sanitizedHtml.trim()) {
            event.preventDefault();
            nextEditor.commands.insertContent(sanitizedHtml, { contentType: "html" });
            syncToolbarState(nextEditor);
            return true;
          }
        }

        if (text) {
          const sanitizedTextHtml = plainTextToHtml(text);

          if (sanitizedTextHtml) {
            event.preventDefault();
            nextEditor.commands.insertContent(sanitizedTextHtml, { contentType: "html" });
            syncToolbarState(nextEditor);
            return true;
          }
        }

        return false;
      },
    },
    immediatelyRender: false,
    onCreate({ editor: nextEditor }) {
      editorRef.current = nextEditor;
      lastSyncedMarkdownRef.current = nextEditor.getMarkdown();
      syncToolbarState(nextEditor);
    },
    onFocus({ editor: nextEditor }) {
      editorRef.current = nextEditor;
      syncToolbarState(nextEditor);
    },
    onSelectionUpdate({ editor: nextEditor }) {
      editorRef.current = nextEditor;
      syncToolbarState(nextEditor);

      if (linkPanelOpen) {
        if (nextEditor.state.selection.empty && nextEditor.isActive("link")) {
          nextEditor.chain().focus().extendMarkRange("link").run();
          return;
        }

        if (nextEditor.state.selection.empty && !nextEditor.isActive("link")) {
          setLinkPanelOpen(false);
          return;
        }

        pendingLinkSelectionRef.current = {
          from: nextEditor.state.selection.from,
          to: nextEditor.state.selection.to,
        };
        setLinkDraft((nextEditor.getAttributes("link").href as string | undefined) ?? "");
        updateLinkPanelPosition(nextEditor);
      }
    },
    onUpdate({ editor: nextEditor }) {
      editorRef.current = nextEditor;
      const nextMarkdown = nextEditor.getMarkdown();

      lastSyncedMarkdownRef.current = nextMarkdown;
      syncToolbarState(nextEditor);

      if (nextMarkdown !== valueRef.current) {
        onChangeRef.current(nextMarkdown);
      }
    },
  });

  useEffect(() => {
    editorRef.current = editor;
  }, [editor]);

  useEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);

  useEffect(() => {
    valueRef.current = value;
  }, [value]);

  useEffect(() => {
    if (!editor) {
      return;
    }

    const currentMarkdown = editor.getMarkdown();

    if (value === currentMarkdown || value === lastSyncedMarkdownRef.current) {
      syncToolbarState(editor);
      return;
    }

    lastSyncedMarkdownRef.current = value;
    editor.commands.setContent(value, {
      contentType: "markdown",
    });
    syncToolbarState(editor);
  }, [editor, value]);

  useEffect(() => {
    if (!linkPanelOpen) {
      return;
    }

    const rafId = window.requestAnimationFrame(() => {
      updateLinkPanelPosition();
      linkInputRef.current?.focus();
      linkInputRef.current?.select();
    });

    function handlePointerDown(event: globalThis.MouseEvent) {
      const target = event.target as Node;

      if (linkPanelRef.current?.contains(target) || editorComposerRef.current?.contains(target)) {
        return;
      }

      setLinkPanelOpen(false);
    }

    function handleKeyDown(event: globalThis.KeyboardEvent) {
      if (event.key === "Escape") {
        setLinkPanelOpen(false);
      }
    }

    function handleResize() {
      updateLinkPanelPosition();
    }

    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    window.addEventListener("resize", handleResize);
    editor?.view.dom.addEventListener("scroll", handleResize);

    return () => {
      window.cancelAnimationFrame(rafId);
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("resize", handleResize);
      editor?.view.dom.removeEventListener("scroll", handleResize);
    };
  }, [editor, linkPanelOpen]);

  function handleToolbarButtonMouseDown(event: MouseEvent<HTMLButtonElement>) {
    event.preventDefault();
  }

  function openLinkPanel() {
    if (!editor) {
      return;
    }

    if (editor.state.selection.empty && !editor.isActive("link")) {
      return;
    }

    if (editor.isActive("link")) {
      editor.chain().focus().extendMarkRange("link").run();
      syncToolbarState(editor);
    }

    pendingLinkSelectionRef.current = {
      from: editor.state.selection.from,
      to: editor.state.selection.to,
    };
    setLinkDraft((editor.getAttributes("link").href as string | undefined) ?? "");
    setLinkPanelOpen(true);
  }

  function applyLinkFromPanel() {
    if (!editor) {
      return;
    }

    const pendingSelection = pendingLinkSelectionRef.current;
    const normalizedHref = normalizeLinkHref(linkDraft);
    const chain = editor.chain().focus();

    if (pendingSelection) {
      chain.setTextSelection(pendingSelection);
    }

    if (!normalizedHref) {
      chain.extendMarkRange("link").unsetLink().run();
      syncToolbarState(editor);
      setLinkPanelOpen(false);
      return;
    }

    chain.extendMarkRange("link").setLink({ href: normalizedHref }).run();
    syncToolbarState(editor);
    setLinkPanelOpen(false);
  }

  function removeLinkFromPanel() {
    if (!editor) {
      return;
    }

    const pendingSelection = pendingLinkSelectionRef.current;
    const chain = editor.chain().focus();

    if (pendingSelection) {
      chain.setTextSelection(pendingSelection);
    }

    chain.extendMarkRange("link").unsetLink().run();
    syncToolbarState(editor);
    setLinkDraft("");
    setLinkPanelOpen(false);
  }

  function handleLinkInputKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Enter") {
      event.preventDefault();
      applyLinkFromPanel();
      return;
    }

    if (event.key === "Escape") {
      event.preventDefault();
      setLinkPanelOpen(false);
    }
  }

  function handleToolbarAction(action: string) {
    if (!editor) {
      return;
    }

    if (action !== "link") {
      setLinkPanelOpen(false);
    }

    if (action === "undo") {
      editor.chain().focus().undo().run();
      syncToolbarState(editor);
      return;
    }

    if (action === "redo") {
      editor.chain().focus().redo().run();
      syncToolbarState(editor);
      return;
    }

    if (action === "bold") {
      editor.chain().focus().toggleBold().run();
      syncToolbarState(editor);
      return;
    }

    if (action === "italic") {
      editor.chain().focus().toggleItalic().run();
      syncToolbarState(editor);
      return;
    }

    if (action === "ordered-list") {
      editor.chain().focus().toggleOrderedList().run();
      syncToolbarState(editor);
      return;
    }

    if (action === "unordered-list") {
      editor.chain().focus().toggleBulletList().run();
      syncToolbarState(editor);
      return;
    }

    if (action === "clear-format") {
      editor.chain().focus().unsetAllMarks().clearNodes().run();
      syncToolbarState(editor);
      return;
    }

    if (action === "link") {
      if (linkPanelOpen) {
        setLinkPanelOpen(false);
        return;
      }

      openLinkPanel();
      return;
    }
  }

  function handleBlockStyleChange(event: ChangeEvent<HTMLSelectElement>) {
    if (!editor) {
      return;
    }

    setLinkPanelOpen(false);

    const nextStyle = event.target.value as EditorBlockStyle;

    if (nextStyle === "p") {
      editor.chain().focus().setParagraph().run();
      syncToolbarState(editor);
      return;
    }

    editor
      .chain()
      .focus()
      .setHeading({
        level: nextStyle === "h1" ? 1 : 2,
      })
      .run();
    syncToolbarState(editor);
  }

  const toolbarItems: Array<
    | { type: "button"; key: string; label: string; icon: ReactNode; active?: boolean; disabled?: boolean }
    | { type: "select"; key: string; label: string; value: EditorBlockStyle }
  > = [
    {
      type: "button",
      key: "undo",
      label: locale === "zh-CN" ? "撤销" : "Undo",
      disabled: !toolbarState.canUndo,
      icon: (
        <svg aria-hidden="true" className={styles.editorToolIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path d="M9 8 5 12l4 4" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.9" />
          <path d="M6 12h6.25c3.73 0 6.75 3.02 6.75 6.75" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.9" />
        </svg>
      ),
    },
    {
      type: "button",
      key: "redo",
      label: locale === "zh-CN" ? "重做" : "Redo",
      disabled: !toolbarState.canRedo,
      icon: (
        <svg aria-hidden="true" className={styles.editorToolIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path d="m15 8 4 4-4 4" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.9" />
          <path d="M18 12h-6.25C8.02 12 5 15.02 5 18.75" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.9" />
        </svg>
      ),
    },
    {
      type: "select",
      key: "style",
      label: locale === "zh-CN" ? "正文样式" : "Text style",
      value: toolbarState.blockStyle,
    },
    {
      type: "button",
      key: "bold",
      label: locale === "zh-CN" ? "粗体" : "Bold",
      active: toolbarState.bold,
      icon: <span className={styles.editorToolGlyphStrong}>B</span>,
    },
    {
      type: "button",
      key: "italic",
      label: locale === "zh-CN" ? "斜体" : "Italic",
      active: toolbarState.italic,
      icon: <span className={styles.editorToolGlyphEmphasis}>I</span>,
    },
    {
      type: "button",
      key: "link",
      label: locale === "zh-CN" ? "链接" : "Link",
      active: toolbarState.link || linkPanelOpen,
      disabled: !toolbarState.link && (!editor || editor.state.selection.empty),
      icon: (
        <svg aria-hidden="true" className={styles.editorToolIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path d="M10.5 13.5 13.5 10.5" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.9" />
          <path d="M8.25 15.75 6.5 17.5a3 3 0 1 1-4.24-4.24L4 11.5" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.9" />
          <path d="m15.75 8.25 1.75-1.75a3 3 0 1 1 4.24 4.24L20 12.5" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.9" />
        </svg>
      ),
    },
    {
      type: "button",
      key: "ordered-list",
      label: locale === "zh-CN" ? "有序列表" : "Ordered list",
      active: toolbarState.orderedList,
      icon: (
        <svg aria-hidden="true" className={styles.editorToolIconWide} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path d="M10 7h10M10 12h10M10 17h10" strokeLinecap="round" strokeWidth="1.9" />
          <path d="M4.5 6.25h.5v4M4 10.25h2" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.7" />
          <path d="M4 15.25c.25-.67.78-1 1.6-1 .86 0 1.4.42 1.4 1.12 0 .57-.3.96-1.45 2.08L4 19.25h3" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.7" />
        </svg>
      ),
    },
    {
      type: "button",
      key: "unordered-list",
      label: locale === "zh-CN" ? "无序列表" : "Bulleted list",
      active: toolbarState.unorderedList,
      icon: (
        <svg aria-hidden="true" className={styles.editorToolIconWide} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path d="M10 7h10M10 12h10M10 17h10" strokeLinecap="round" strokeWidth="1.9" />
          <circle cx="5" cy="7" r="1.1" fill="currentColor" stroke="none" />
          <circle cx="5" cy="12" r="1.1" fill="currentColor" stroke="none" />
          <circle cx="5" cy="17" r="1.1" fill="currentColor" stroke="none" />
        </svg>
      ),
    },
    {
      type: "button",
      key: "clear-format",
      label: locale === "zh-CN" ? "清除格式" : "Clear formatting",
      icon: (
        <svg aria-hidden="true" className={styles.editorToolIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path d="M5 5h9l5 5-9 9-5-5 4-4" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.9" />
          <path d="M14 5 5 14" strokeLinecap="round" strokeWidth="1.9" />
          <path d="M14.5 18.5h5" strokeLinecap="round" strokeWidth="1.9" />
        </svg>
      ),
    },
  ];

  const hasPendingLinkSelection =
    !!pendingLinkSelectionRef.current && pendingLinkSelectionRef.current.from !== pendingLinkSelectionRef.current.to;
  const canApplyLink = !!editor && (!!linkDraft.trim() || toolbarState.link) && (toolbarState.link || hasPendingLinkSelection);

  return (
    <div className={styles.editorComposer} ref={editorComposerRef}>
      <div className={styles.editorToolbar} ref={editorToolbarRef} role="toolbar" aria-label={toolbarLabel}>
        {toolbarItems.map((item) =>
          item.type === "select" ? (
            <div className={styles.editorToolSelect} key={item.key}>
              <select
                aria-label={item.label}
                className={`${styles.editorToolButton} ${styles.editorToolSelectControl}`}
                onChange={handleBlockStyleChange}
                value={item.value}
              >
                {blockStyleOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
              <svg aria-hidden="true" className={styles.editorToolSelectIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path d="m8 10 4-4 4 4M8 14l4 4 4-4" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.8" />
              </svg>
            </div>
        ) : (
          <button
            aria-controls={item.key === "link" ? "workspace-note-link-panel" : undefined}
            aria-expanded={item.key === "link" ? linkPanelOpen : undefined}
            aria-label={item.label}
            className={`${styles.editorToolButton}${item.active ? ` ${styles.editorToolButtonActive}` : ""}`}
            disabled={item.disabled}
              key={item.key}
              onClick={() => handleToolbarAction(item.key)}
              onMouseDown={handleToolbarButtonMouseDown}
              type="button"
            >
              {item.icon}
            </button>
          ),
        )}
      </div>
      {linkPanelOpen ? (
        <div
          className={styles.editorLinkPanel}
          data-placement={linkPanelPosition.placement}
          id="workspace-note-link-panel"
          ref={linkPanelRef}
          style={{
            "--link-panel-arrow-left": `${linkPanelPosition.arrowLeft}px`,
            left: linkPanelPosition.left,
            top: linkPanelPosition.top,
          } as CSSProperties}
        >
          <div className={styles.editorLinkPanelField}>
            <input
              className={styles.editorLinkInput}
              onChange={(event) => setLinkDraft(event.target.value)}
              onKeyDown={handleLinkInputKeyDown}
              placeholder={locale === "zh-CN" ? "粘贴或输入链接地址" : "Paste or enter a link URL"}
              ref={linkInputRef}
              value={linkDraft}
            />
          </div>
          <div className={styles.editorLinkPanelActions}>
            <button
              className={`${styles.editorLinkPanelButton} ${styles.editorLinkPanelButtonPrimary}`}
              disabled={!canApplyLink}
              onClick={applyLinkFromPanel}
              type="button"
            >
              {locale === "zh-CN" ? "应用" : "Apply"}
            </button>
            {toolbarState.link ? (
              <button
                className={`${styles.editorLinkPanelButton} ${styles.editorLinkPanelButtonDanger}`}
                onClick={removeLinkFromPanel}
                type="button"
              >
                {locale === "zh-CN" ? "移除" : "Remove"}
              </button>
            ) : null}
            <button className={styles.editorLinkPanelButton} onClick={() => setLinkPanelOpen(false)} type="button">
              {locale === "zh-CN" ? "取消" : "Cancel"}
            </button>
          </div>
        </div>
      ) : null}
      <div className={styles.editorTextareaShell}>
        <EditorContent editor={editor} id="workspace-note-content" className={styles.editorTextarea} />
      </div>
    </div>
  );
}
