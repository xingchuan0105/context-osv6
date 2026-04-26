import React from 'react';
import { NotebookMenu } from './NotebookMenu';
import { Lock } from 'lucide-react';

interface NotebookProps {
  notebook: {
    id: number;
    title: string;
    date: string;
    sources: number;
    iconEmoji: string;
  };
}

export const NotebookCard: React.FC<NotebookProps> = ({ notebook }) => {
  return (
    <div className="relative flex flex-col justify-between h-[180px] bg-white border border-zinc-200/80 rounded-2xl p-5 cursor-pointer hover:shadow-md hover:border-zinc-300/80 transition-all duration-200 group">
      {/* Top Section */}
      <div className="flex justify-between items-start">
        {/* Icon (Gray Circular Container) */}
        <div className="w-11 h-11 flex items-center justify-center bg-zinc-100/80 text-zinc-700 rounded-full text-xl">
          {notebook.iconEmoji}
        </div>

        {/* Menu button */}
        <div onClick={(e) => e.stopPropagation()} className="opacity-0 group-hover:opacity-100 transition-opacity">
          <NotebookMenu />
        </div>
      </div>

      {/* Bottom Section */}
      <div className="flex flex-col mt-4">
        <h3 className="text-[15px] font-medium text-zinc-800 leading-snug line-clamp-2 mb-3">
          {notebook.title}
        </h3>

        <div className="flex justify-between items-center text-[12px] font-medium text-zinc-400">
          <span className="flex items-center gap-1.5">
            {notebook.date}
          </span>
          <div className="flex items-center gap-1">
            <Lock size={12} className="text-zinc-300" />
            <span className="text-zinc-400">私有</span>
          </div>
        </div>
      </div>
    </div>
  );
};
