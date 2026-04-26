import React, { useState } from 'react';
import { Paperclip, ArrowUp, Lightbulb, Plus, SwitchCamera } from 'lucide-react';
import { motion, AnimatePresence } from 'motion/react';

export const ChatBox: React.FC = () => {
  const [isFocused, setIsFocused] = useState(false);
  const [inputValue, setInputValue] = useState('');
  const [isPro, setIsPro] = useState(false);

  return (
    <div className="absolute bottom-0 left-0 w-full bg-gradient-to-t from-white via-white/95 to-transparent pb-8 pt-16 px-8 flex flex-col items-center justify-end z-20 pointer-events-none">
      <div className="w-full max-w-3xl pointer-events-auto flex flex-col gap-4 relative">

        {/* Suggested Prompts (Perplexity Style) */}
        <AnimatePresence>
          {!isFocused && !inputValue && (
            <motion.div
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -10, transition: { duration: 0.15 } }}
              className="flex gap-2 mb-1 overflow-x-auto scrollbar-hide w-full flex-wrap justify-center"
            >
              <button className="flex items-center gap-1.5 px-3 py-1.5 bg-white border border-zinc-200 text-zinc-600 rounded-full text-[13px] font-medium hover:bg-zinc-50 transition-colors shadow-sm whitespace-nowrap">
                <Lightbulb size={14} className="text-amber-500" />
                Summarize all sources
              </button>
              <button className="flex items-center gap-1.5 px-3 py-1.5 bg-white border border-zinc-200 text-zinc-600 rounded-full text-[13px] font-medium hover:bg-zinc-50 transition-colors shadow-sm whitespace-nowrap">
                <Lightbulb size={14} className="text-emerald-500" />
                Find contradictions
              </button>
              <button className="flex items-center gap-1.5 px-3 py-1.5 bg-white border border-zinc-200 text-zinc-600 rounded-full text-[13px] font-medium hover:bg-zinc-50 transition-colors shadow-sm whitespace-nowrap">
                <Lightbulb size={14} className="text-blue-500" />
                Extract main topics
              </button>
            </motion.div>
          )}
        </AnimatePresence>

        {/* The Chat Input (Perplexity Style) */}
        <motion.div
          layout
          className={`flex flex-col relative w-full bg-[#FAFAFA] border ${isFocused ? 'border-zinc-300 ring-4 ring-zinc-100/50' : 'border-[#E5E5E5]'} rounded-3xl transition-all duration-300 shadow-[0_4px_16px_rgba(0,0,0,0.04)] hover:shadow-[0_8px_24px_rgba(0,0,0,0.06)]`}
        >
          <div className="flex items-center px-4 py-3">
            <button className="text-zinc-400 hover:text-zinc-600 hover:bg-zinc-100 rounded-full p-2 transition-colors flex-shrink-0 -ml-2">
              <Plus size={22} />
            </button>
            <textarea
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              onFocus={() => setIsFocused(true)}
              onBlur={() => setIsFocused(false)}
              placeholder="Ask about this notebook..."
              className="w-full min-h-[44px] max-h-60 px-3 py-2.5 bg-transparent text-[15px] resize-none outline-none text-zinc-800 placeholder:text-zinc-400 font-medium leading-relaxed"
              rows={1}
            />
          </div>

          <div className="flex items-center justify-between px-4 pb-3 pt-1 border-t-0">
            {/* Left Tools (Focus/Pro) */}
            <div className="flex items-center gap-2">
              <button className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-semibold text-zinc-500 hover:text-zinc-700 hover:bg-zinc-100 rounded-full transition-colors">
                <Paperclip size={14} />
                Focus
              </button>
              <button
                onClick={() => setIsPro(!isPro)}
                className={`flex items-center gap-1.5 px-3 py-1.5 text-xs font-semibold rounded-full transition-colors ${isPro ? 'bg-blue-50 text-blue-600 hover:bg-blue-100' : 'text-zinc-500 hover:text-zinc-700 hover:bg-zinc-100'}`}
              >
                <SwitchCamera size={14} />
                Pro
              </button>
            </div>

            {/* Right Tools & Send */}
            <div className="flex items-center gap-3">
              <button
                className={`p-2.5 rounded-full flex items-center justify-center transition-all ${
                  inputValue.length > 0
                    ? 'bg-zinc-900 text-white hover:bg-zinc-800 shadow-md transform hover:scale-105 active:scale-95'
                    : 'bg-[#E5E5E5] text-white cursor-not-allowed'
                }`}
              >
                <ArrowUp size={18} strokeWidth={2.5} />
              </button>
            </div>
          </div>
        </motion.div>

        {/* Footer info text */}
        <p className="text-center text-[11px] text-zinc-400">
          NotebookAI can make mistakes. Consider verifying important information.
        </p>
      </div>
    </div>
  );
};
