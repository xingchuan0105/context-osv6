import React from 'react';
import {
  Library,
  Plus,
  Search,
  BookOpen,
  FileText,
  Folder,
  Settings,
  UserCircle
} from 'lucide-react';

interface SidebarProps {
  activeNotebook: string;
  setActiveNotebook: (notebook: string) => void;
}

export const Sidebar: React.FC<SidebarProps> = ({ activeNotebook, setActiveNotebook }) => {
  const notebooks = [
    { id: '1', title: 'Workspace AI Guidelines', icon: <FileText size={16} /> },
    { id: '2', title: 'Product Requirements', icon: <Folder size={16} /> },
    { id: '3', title: 'Q3 Marketing Strategy', icon: <BookOpen size={16} /> },
    { id: '4', title: 'Competitive Analysis', icon: <FileText size={16} /> },
  ];

  return (
    <div className="w-[260px] h-full flex flex-col bg-[#F9F9F9] border-r border-[#E5E5E5] text-sm">
      {/* Header: Logo + Product Name */}
      <div className="flex items-center gap-2 px-5 py-6">
        <div className="flex items-center justify-center w-8 h-8 rounded-lg bg-zinc-900 text-white">
          <Library size={18} />
        </div>
        <span className="font-semibold text-[15px] tracking-tight text-zinc-900">NotebookAI</span>
      </div>

      {/* New Notebook Button */}
      <div className="px-4 mb-4">
        <button className="flex items-center gap-2 w-full px-3 py-2 text-zinc-700 bg-white border border-[#E5E5E5] rounded-lg shadow-sm hover:bg-zinc-50 transition-colors">
          <Plus size={16} />
          <span className="font-medium">New Notebook</span>
        </button>
      </div>

      {/* Search (Optional, Perplexity has search history, NotebookLM has search inside notebook) */}
      <div className="px-4 mb-6">
        <div className="relative flex items-center w-full h-9 rounded-lg bg-white border border-[#E5E5E5] overflow-hidden group focus-within:ring-1 focus-within:ring-zinc-400 focus-within:border-zinc-400 transition-shadow">
          <Search size={14} className="absolute left-3 text-zinc-400 group-focus-within:text-zinc-600" />
          <input
            type="text"
            placeholder="Search notebooks..."
            className="w-full h-full pl-9 pr-3 text-[13px] bg-transparent outline-none text-zinc-800 placeholder:text-zinc-400"
          />
        </div>
      </div>

      {/* Notebooks List */}
      <div className="flex-1 overflow-y-auto px-2">
        <div className="px-2 mb-2">
          <h3 className="text-xs font-semibold text-zinc-500 tracking-wider uppercase">Your Library</h3>
        </div>
        <div className="space-y-0.5">
          {notebooks.map((nb) => (
            <button
              key={nb.id}
              onClick={() => setActiveNotebook(nb.title)}
              className={`flex items-center gap-2.5 w-full px-2 py-2 rounded-md transition-colors text-left ${
                activeNotebook === nb.title
                  ? 'bg-zinc-200/50 text-zinc-900 font-medium'
                  : 'text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900'
              }`}
            >
              <div className={`${activeNotebook === nb.title ? 'text-zinc-800' : 'text-zinc-400'}`}>
                {nb.icon}
              </div>
              <span className="truncate flex-1 text-[13px]">{nb.title}</span>
            </button>
          ))}
        </div>
      </div>

      {/* Footer / Profile */}
      <div className="p-4 border-t border-[#E5E5E5] flex flex-col gap-1">
        <button className="flex items-center gap-2.5 w-full px-2 py-2 text-zinc-600 hover:text-zinc-900 hover:bg-zinc-100 rounded-md transition-colors">
          <Settings size={16} />
          <span className="text-[13px] font-medium">Settings</span>
        </button>
        <button className="flex items-center gap-2.5 w-full px-2 py-2 text-zinc-600 hover:text-zinc-900 hover:bg-zinc-100 rounded-md transition-colors">
          <UserCircle size={16} />
          <span className="text-[13px] font-medium">Alex Morgan</span>
        </button>
      </div>
    </div>
  );
};
