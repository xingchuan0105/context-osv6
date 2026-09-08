import { Editor } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import { Markdown } from "@tiptap/markdown";
import { Placeholder, UndoRedo } from "@tiptap/extensions";

export function initNoteEditor(mountEl, initialMarkdown, onUpdate) {
  const editor = new Editor({
    element: mountEl,
    extensions: [
      StarterKit.configure({
        heading: { levels: [1, 2] },
        link: { openOnClick: false },
        undoRedo: false,
      }),
      Markdown,
      Placeholder.configure({ placeholder: "开始记录..." }),
      UndoRedo.configure({ depth: 100 }),
    ],
    content: initialMarkdown || "",
    onUpdate({ editor: current }) {
      const markdown = current.storage.markdown?.getMarkdown?.() ?? current.getText();
      onUpdate(markdown);
    },
  });
  return {
    destroy() {
      editor.destroy();
    },
    getMarkdown() {
      return editor.storage.markdown?.getMarkdown?.() ?? editor.getText();
    },
    setMarkdown(markdown) {
      editor.commands.setContent(markdown || "");
    },
    focus() {
      editor.commands.focus();
    },
  };
}
