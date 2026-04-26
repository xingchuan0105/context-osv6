import React from 'react';
import { Settings, User } from 'lucide-react';

export const Header: React.FC = () => {
  return (
    <header className="flex items-center justify-between px-8 py-5 w-full bg-white border-b border-zinc-100">
      {/* Logo */}
      <div className="flex items-center gap-2.5 text-zinc-900 cursor-pointer">
        <svg
          viewBox="0 0 24 24"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
          className="w-7 h-7"
        >
          <path
            d="M3 17V7c0-1.1.9-2 2-2h14c1.1 0 2 .9 2 2v10c0 1.1-.9 2-2 2H5c-1.1 0-2-.9-2-2z"
            stroke="currentColor"
            strokeWidth="2.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <path
            d="M8 9h8"
            stroke="currentColor"
            strokeWidth="2.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <path
            d="M8 13h5"
            stroke="currentColor"
            strokeWidth="2.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
        <span className="font-semibold text-xl tracking-tight mt-0.5">NotebookLM</span>
      </div>

      {/* Right Actions */}
      <div className="flex items-center gap-5">
        {/* Settings */}
        <button className="flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium text-zinc-600 border border-zinc-200 rounded-full hover:bg-zinc-50 hover:text-zinc-900 transition-colors shadow-sm">
          <Settings size={15} />
          设置
        </button>

        {/* Avatar */}
        <button className="w-[34px] h-[34px] rounded-full bg-zinc-100 text-zinc-600 flex items-center justify-center font-medium shadow-inner ring-2 ring-transparent hover:ring-zinc-200 hover:bg-zinc-200 transition-all outline-none">
          <User size={18} strokeWidth={2.5} />
        </button>
      </div>
    </header>
  );
};
