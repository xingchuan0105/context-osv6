import React from 'react';
import { NotebookMenu } from './NotebookMenu';

interface NotebookListProps {
  notebook: {
    id: number;
    title: string;
    date: string;
    sources: number;
    role: string;
    iconEmoji: string;
  };
}

export const NotebookListRow: React.FC<NotebookListProps> = ({ notebook }) => {
  return (
    <div className="grid grid-cols-12 gap-4 py-3.5 border-b border-zinc-100 hover:bg-zinc-50 px-4 items-center transition-colors group cursor-pointer">
      <div className="col-span-6 flex items-center gap-3 pr-4">
        {/* Simple gray background for icon to match Perplexity style */}
        <div className="w-8 h-8 flex-shrink-0 flex items-center justify-center bg-zinc-100 text-zinc-700 rounded-md text-[15px]">
          {notebook.iconEmoji}
        </div>
        <span className="text-[14.5px] font-medium text-zinc-800 truncate">
          {notebook.title}
        </span>
      </div>

      <div className="col-span-2 text-[14px] text-zinc-500">
        {notebook.sources} 个来源
      </div>

      <div className="col-span-2 text-[14px] text-zinc-500">
        {notebook.date}
      </div>

      <div className="col-span-2 flex items-center justify-between">
        <span className="text-[14px] text-zinc-500">{notebook.role}</span>
        <div onClick={(e) => e.stopPropagation()} className="opacity-0 group-hover:opacity-100 transition-opacity">
          <NotebookMenu />
        </div>
      </div>
    </div>
  );
};
