import React, { useState } from 'react';
import { Plus, Search, MoreVertical, Pin, Trash2, MessageSquare } from 'lucide-react';

interface Thread {
  id: number;
  title: string;
}

interface SidebarLeftProps {
  threads: Thread[];
  activeThreadId: number;
  onSelectThread: (id: number) => void;
  onNewThread: () => void;
}

export function SidebarLeft({ threads, activeThreadId, onSelectThread, onNewThread }: SidebarLeftProps) {
  const [activeMenu, setActiveMenu] = useState<number | null>(null);

  return (
    <div className="w-64 border-r border-gray-200 bg-[#f9f9f9] flex flex-col h-full shrink-0">
      {/* Search / New Thread */}
      <div className="p-4 flex flex-col gap-3">
        <button 
          onClick={onNewThread}
          className="flex items-center gap-2 bg-gray-900 hover:bg-black text-white font-medium py-2 px-4 rounded-full transition-colors w-full justify-center shadow-sm"
        >
          <Plus className="w-5 h-5" />
          <span>New Thread</span>
        </button>
        <div className="relative group">
          <Search className="w-4 h-4 absolute left-3 top-2.5 text-gray-400 group-hover:text-gray-900 transition-colors" />
          <input
            type="text"
            placeholder="Search threads"
            className="w-full bg-white border border-gray-300 rounded-full py-2 pl-9 pr-4 text-sm focus:outline-none focus:ring-2 focus:ring-gray-900 focus:border-transparent transition-shadow"
          />
        </div>
      </div>

      {/* Sessions List */}
      <div className="flex-1 overflow-y-auto px-2 pb-4 scrollbar-thin scrollbar-thumb-gray-300 scrollbar-track-transparent">
        <div className="text-xs font-semibold text-gray-500 uppercase tracking-wider px-3 mb-2 mt-2">
          Threads
        </div>
        
        <div className="space-y-0.5">
          {threads.map((session) => (
            <div
              key={session.id}
              onClick={() => onSelectThread(session.id)}
              className={`group relative flex items-center justify-between gap-2.5 px-3 py-2 text-sm text-gray-700 hover:bg-gray-200 rounded-lg cursor-pointer transition-colors ${
                activeThreadId === session.id ? 'bg-gray-200 font-medium text-gray-900' : ''
              }`}
              onMouseLeave={() => setActiveMenu(null)}
            >
              <span className="truncate flex-1">{session.title}</span>
              
              <button 
                className="p-1 text-gray-400 hover:text-gray-900 opacity-0 group-hover:opacity-100 transition-opacity"
                onClick={(e) => {
                  e.stopPropagation();
                  setActiveMenu(activeMenu === session.id ? null : session.id);
                }}
              >
                <MoreVertical className="w-4 h-4" />
              </button>

              {activeMenu === session.id && (
                <div className="absolute right-2 top-full mt-1 w-32 bg-white border border-gray-200 rounded-lg shadow-xl z-20 py-1 overflow-hidden">
                  <button className="w-full text-left px-3 py-1.5 text-xs hover:bg-gray-100 flex items-center gap-2 text-gray-700">
                    <Pin className="w-3.5 h-3.5" />
                    Pin
                  </button>
                  <button className="w-full text-left px-3 py-1.5 text-xs hover:bg-gray-100 flex items-center gap-2 text-red-600">
                    <Trash2 className="w-3.5 h-3.5" />
                    Delete
                  </button>
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
