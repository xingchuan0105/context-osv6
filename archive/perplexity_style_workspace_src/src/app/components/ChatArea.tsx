import React, { useState, useRef, useEffect } from 'react';
import { Plus, ArrowUp, Copy, Edit, RotateCcw, FilePlus, Database, MessageCircle, Globe, X } from 'lucide-react';

const MESSAGES = [
  {
    role: 'user',
    content: 'Can you summarize the main findings from the uploaded Project Scope document?',
  },
  {
    role: 'ai',
    content: 'Based on the Project Scope document, the main findings are:\n\n1. **Core Objective**: The primary goal is to launch a unified dashboard by Q4 that integrates marketing and sales data.\n2. **Budget & Timeline**: The allocated budget is $250,000, with a strict deadline of November 15th for the beta release.\n3. **Key Dependencies**: The project heavily relies on the new API endpoints being delivered by the backend team by early September.\n\nWould you like me to elaborate on the risks outlined in section 4?',
    citations: [1, 2],
  },
];

export function ChatArea({ activeThreadId = 1 }: { activeThreadId?: number }) {
  const [showPlusMenu, setShowPlusMenu] = useState(false);
  const [inputValue, setInputValue] = useState('');
  const [activeTags, setActiveTags] = useState<string[]>([]);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const displayMessages = activeThreadId === 1 ? MESSAGES : [];

  const handleAddTag = (tag: string) => {
    if (!activeTags.includes(tag)) {
      setActiveTags([...activeTags, tag]);
    }
    setShowPlusMenu(false);
    // Focus textarea after adding tag
    setTimeout(() => {
      textareaRef.current?.focus();
    }, 0);
  };

  const removeTag = (tagToRemove: string) => {
    setActiveTags(activeTags.filter(t => t !== tagToRemove));
  };

  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      textareaRef.current.style.height = `${textareaRef.current.scrollHeight}px`;
    }
  }, [inputValue]);

  return (
    <div className="flex-1 flex flex-col bg-white border-r border-gray-200 shadow-[inset_0_2px_4px_rgba(0,0,0,0.02)] relative z-0 h-full">
      {/* Chat History */}
      <div className="flex-1 overflow-y-auto p-6 space-y-8 scrollbar-thin scrollbar-thumb-gray-200">
        {displayMessages.length === 0 ? (
          <div className="h-full flex flex-col items-center justify-center text-gray-400 space-y-4">
            <MessageCircle className="w-12 h-12 text-gray-300" />
            <p>Start a new conversation or ask a question.</p>
          </div>
        ) : (
          displayMessages.map((msg, idx) => (
            <div key={idx} className={`flex flex-col gap-2 max-w-3xl mx-auto w-full`}>
              <div className={`flex gap-4 w-full ${msg.role === 'user' ? 'justify-end' : ''}`}>
              <div className={`space-y-2 ${msg.role === 'user' ? 'max-w-[80%]' : 'flex-1'}`}>
                <div className={`p-4 rounded-2xl text-[15px] leading-relaxed ${
                  msg.role === 'user' 
                    ? 'bg-gray-100 text-gray-800' 
                    : 'text-gray-800'
                }`}>
                  {msg.content.split('\n').map((line, i) => (
                    <React.Fragment key={i}>
                      {line}
                      {i !== msg.content.split('\n').length - 1 && <br />}
                    </React.Fragment>
                  ))}
                </div>
                
                {/* Citations / Sources Pills */}
                {msg.role === 'ai' && msg.citations && (
                  <div className="flex items-center gap-2 mt-2">
                    <span className="text-xs text-gray-500 font-medium">Sources:</span>
                    {msg.citations.map((c) => (
                      <span key={c} className="inline-flex items-center justify-center w-5 h-5 rounded-full bg-gray-100 text-gray-600 text-xs font-semibold cursor-pointer hover:bg-gray-200 transition-colors border border-gray-200">
                        {c}
                      </span>
                    ))}
                  </div>
                )}

                {/* Bottom Action Buttons */}
                <div className={`flex items-center gap-3 mt-1 ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}>
                  {msg.role === 'user' ? (
                    <>
                      <button className="flex items-center gap-1.5 text-xs text-gray-400 hover:text-gray-900 transition-colors px-1 py-0.5 rounded cursor-pointer">
                        <Copy className="w-3.5 h-3.5" />
                        <span>Copy</span>
                      </button>
                      <button className="flex items-center gap-1.5 text-xs text-gray-400 hover:text-gray-900 transition-colors px-1 py-0.5 rounded cursor-pointer">
                        <Edit className="w-3.5 h-3.5" />
                        <span>Edit</span>
                      </button>
                    </>
                  ) : (
                    <>
                      <button className="flex items-center gap-1.5 text-xs text-gray-400 hover:text-gray-900 transition-colors px-1 py-0.5 rounded cursor-pointer">
                        <Copy className="w-3.5 h-3.5" />
                        <span>Copy</span>
                      </button>
                      <button className="flex items-center gap-1.5 text-xs text-gray-400 hover:text-gray-900 transition-colors px-1 py-0.5 rounded cursor-pointer">
                        <FilePlus className="w-3.5 h-3.5" />
                        <span>Add to note</span>
                      </button>
                      <button className="flex items-center gap-1.5 text-xs text-gray-400 hover:text-gray-900 transition-colors px-1 py-0.5 rounded cursor-pointer">
                        <RotateCcw className="w-3.5 h-3.5" />
                        <span>Regenerate</span>
                      </button>
                    </>
                  )}
                </div>
              </div>
            </div>
          </div>
          ))
        )}
        {/* Empty space for scrolling past the input box */}
        <div className="h-32"></div>
      </div>

      {/* Chat Input Container */}
      <div className="absolute bottom-0 left-0 right-0 p-4 bg-gradient-to-t from-white via-white to-transparent pointer-events-none">
        <div className="max-w-3xl mx-auto w-full relative bg-white border border-gray-300 rounded-2xl shadow-lg focus-within:ring-2 focus-within:ring-gray-900 focus-within:border-transparent transition-all pointer-events-auto">
          
          <div className="flex flex-col p-3 pb-12">
            {/* Tags area */}
            <div className="flex flex-wrap gap-2 mb-1">
              {activeTags.map(tag => (
                <div key={tag} className="inline-flex items-center gap-1 px-2 py-1 bg-gray-900 text-white text-xs font-medium rounded-md shadow-sm animate-in fade-in zoom-in duration-200">
                  <span>@{tag}</span>
                  <button 
                    onClick={() => removeTag(tag)}
                    className="hover:bg-white/20 rounded-full p-0.5 transition-colors"
                  >
                    <X className="w-3 h-3" />
                  </button>
                </div>
              ))}
            </div>
            
            <textarea
              ref={textareaRef}
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              className="w-full max-h-48 min-h-[40px] resize-none bg-transparent text-gray-800 text-sm focus:outline-none placeholder-gray-400"
              placeholder={activeTags.length > 0 ? "" : "Ask a question about your sources..."}
              rows={1}
            ></textarea>
          </div>
          
          <div className="absolute bottom-3 left-3 right-3 flex justify-between items-center">
            <div className="relative">
              <button 
                className="p-1.5 text-gray-500 hover:bg-gray-100 hover:text-gray-800 rounded-md transition-colors"
                onClick={() => setShowPlusMenu(!showPlusMenu)}
              >
                <Plus className={`w-5 h-5 transition-transform ${showPlusMenu ? 'rotate-45' : ''}`} />
              </button>
              
              {showPlusMenu && (
                <div className="absolute bottom-full mb-2 left-0 w-32 bg-white border border-gray-200 rounded-xl shadow-2xl py-1 z-30">
                  <button 
                    onClick={() => handleAddTag('RAG')}
                    className="w-full text-left px-3 py-2 text-sm hover:bg-gray-50 flex items-center gap-2 text-gray-700"
                  >
                    <Database className="w-4 h-4" />
                    RAG
                  </button>
                  <button 
                    onClick={() => handleAddTag('Chat')}
                    className="w-full text-left px-3 py-2 text-sm hover:bg-gray-50 flex items-center gap-2 text-gray-700"
                  >
                    <MessageCircle className="w-4 h-4" />
                    Chat
                  </button>
                  <button 
                    onClick={() => handleAddTag('Web')}
                    className="w-full text-left px-3 py-2 text-sm hover:bg-gray-50 flex items-center gap-2 text-gray-700"
                  >
                    <Globe className="w-4 h-4" />
                    Web
                  </button>
                </div>
              )}
            </div>
            
            <button className="bg-gray-900 hover:bg-black text-white p-2 rounded-full transition-colors flex items-center justify-center w-8 h-8 group shadow-sm disabled:opacity-50">
              <ArrowUp className="w-4 h-4 group-hover:-translate-y-0.5 transition-transform" />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
