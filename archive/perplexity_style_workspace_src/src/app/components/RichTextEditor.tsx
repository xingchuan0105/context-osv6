import React, { useRef, useEffect, useState } from 'react';
import { Undo, Redo, Bold, Italic, List, ListOrdered, Heading1, Heading2, Type } from 'lucide-react';

interface RichTextEditorProps {
  content: string;
  onChange: (content: string) => void;
  placeholder?: string;
}

export function RichTextEditor({ content, onChange, placeholder }: RichTextEditorProps) {
  const editorRef = useRef<HTMLDivElement>(null);
  const [isEmpty, setIsEmpty] = useState(true);

  // Sync incoming content once (or when it explicitly changes outside of editing)
  useEffect(() => {
    if (editorRef.current) {
      if (editorRef.current.innerHTML !== content) {
        editorRef.current.innerHTML = content || '';
      }
      setIsEmpty(!editorRef.current.textContent?.trim());
    }
  }, [content]);

  const handleInput = () => {
    if (editorRef.current) {
      const html = editorRef.current.innerHTML;
      onChange(html);
      setIsEmpty(!editorRef.current.textContent?.trim());
    }
  };

  const execCommand = (command: string, value: string | undefined = undefined) => {
    document.execCommand(command, false, value);
    if (editorRef.current) {
      editorRef.current.focus();
      handleInput(); // Trigger change after formatting
    }
  };

  const handleFormatBlock = (tag: string) => {
    // formatBlock requires `<TAG>` in some browsers and just `TAG` in others.
    // The safest cross-browser is usually just the tag name or `<TAG>`.
    document.execCommand('formatBlock', false, tag);
    if (editorRef.current) {
      editorRef.current.focus();
      handleInput();
    }
  };

  return (
    <div className="flex flex-col flex-1 h-full min-h-0 bg-white overflow-hidden">
      {/* Editor Toolbar */}
      <div className="flex items-center gap-1 px-3 py-2 border-b border-gray-100 bg-[#fcfcfc] shrink-0 sticky top-0 z-10 flex-wrap">
        
        <div className="flex items-center gap-0.5">
          <button 
            title="Undo"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => execCommand('undo')} 
            className="p-1.5 text-gray-500 hover:bg-gray-100 hover:text-gray-900 rounded transition-colors"
          >
            <Undo className="w-4 h-4" />
          </button>
          <button 
            title="Redo"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => execCommand('redo')} 
            className="p-1.5 text-gray-500 hover:bg-gray-100 hover:text-gray-900 rounded transition-colors"
          >
            <Redo className="w-4 h-4" />
          </button>
        </div>

        <div className="w-[1px] h-4 bg-gray-200 mx-1" />

        <div className="flex items-center gap-0.5">
          <button 
            title="Heading 1"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => handleFormatBlock('H1')} 
            className="p-1.5 text-gray-500 hover:bg-gray-100 hover:text-gray-900 rounded transition-colors"
          >
            <Heading1 className="w-4 h-4" />
          </button>
          <button 
            title="Heading 2"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => handleFormatBlock('H2')} 
            className="p-1.5 text-gray-500 hover:bg-gray-100 hover:text-gray-900 rounded transition-colors"
          >
            <Heading2 className="w-4 h-4" />
          </button>
          <button 
            title="Paragraph"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => handleFormatBlock('P')} 
            className="p-1.5 text-gray-500 hover:bg-gray-100 hover:text-gray-900 rounded transition-colors"
          >
            <Type className="w-4 h-4" />
          </button>
        </div>

        <div className="w-[1px] h-4 bg-gray-200 mx-1" />

        <div className="flex items-center gap-0.5">
          <button 
            title="Bold"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => execCommand('bold')} 
            className="p-1.5 text-gray-500 hover:bg-gray-100 hover:text-gray-900 rounded transition-colors"
          >
            <Bold className="w-4 h-4" />
          </button>
          <button 
            title="Italic"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => execCommand('italic')} 
            className="p-1.5 text-gray-500 hover:bg-gray-100 hover:text-gray-900 rounded transition-colors"
          >
            <Italic className="w-4 h-4" />
          </button>
        </div>

        <div className="w-[1px] h-4 bg-gray-200 mx-1" />

        <div className="flex items-center gap-0.5">
          <button 
            title="Bullet List"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => execCommand('insertUnorderedList')} 
            className="p-1.5 text-gray-500 hover:bg-gray-100 hover:text-gray-900 rounded transition-colors"
          >
            <List className="w-4 h-4" />
          </button>
          <button 
            title="Numbered List"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => execCommand('insertOrderedList')} 
            className="p-1.5 text-gray-500 hover:bg-gray-100 hover:text-gray-900 rounded transition-colors"
          >
            <ListOrdered className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Editor Content Area */}
      <div className="relative flex-1 overflow-y-auto overflow-x-hidden p-4 scrollbar-thin scrollbar-thumb-gray-200">
        {isEmpty && !content && (
          <div className="absolute top-4 left-4 text-gray-400 pointer-events-none select-none text-[15px]">
            {placeholder || 'Start typing...'}
          </div>
        )}
        <div 
          ref={editorRef}
          contentEditable
          onInput={handleInput}
          onBlur={handleInput}
          className="w-full min-h-full focus:outline-none text-[15px] leading-relaxed text-gray-800"
          style={{
            outline: 'none',
            whiteSpace: 'pre-wrap',
            wordBreak: 'break-word',
          }}
        />
      </div>

      {/* Internal CSS for the formatted content */}
      <style dangerouslySetInnerHTML={{__html: `
        [contenteditable] h1 { font-size: 1.5rem; font-weight: 700; margin-bottom: 0.5rem; margin-top: 1rem; line-height: 1.2; color: #111; }
        [contenteditable] h1:first-child { margin-top: 0; }
        [contenteditable] h2 { font-size: 1.25rem; font-weight: 600; margin-bottom: 0.5rem; margin-top: 1rem; line-height: 1.3; color: #333; }
        [contenteditable] h2:first-child { margin-top: 0; }
        [contenteditable] p { margin-bottom: 0.75rem; min-height: 1.5em; }
        [contenteditable] p:last-child { margin-bottom: 0; }
        [contenteditable] ul { list-style-type: disc; padding-left: 1.5rem; margin-bottom: 0.75rem; }
        [contenteditable] ol { list-style-type: decimal; padding-left: 1.5rem; margin-bottom: 0.75rem; }
        [contenteditable] li { margin-bottom: 0.25rem; }
        [contenteditable] b, [contenteditable] strong { font-weight: 600; color: #111; }
        [contenteditable] i, [contenteditable] em { font-style: italic; }
      `}} />
    </div>
  );
}