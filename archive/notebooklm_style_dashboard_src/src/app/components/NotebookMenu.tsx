import React from 'react';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import { MoreVertical } from 'lucide-react';

export const NotebookMenu: React.FC = () => {
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <button className="p-1.5 text-zinc-400 hover:text-zinc-800 hover:bg-zinc-200/50 rounded-full transition-colors outline-none data-[state=open]:bg-zinc-200/50 data-[state=open]:text-zinc-800">
          <MoreVertical size={18} />
        </button>
      </DropdownMenu.Trigger>

      <DropdownMenu.Portal>
        <DropdownMenu.Content
          align="end"
          className="min-w-[140px] bg-white rounded-xl shadow-lg border border-zinc-100 p-1.5 z-50 animate-in fade-in zoom-in-95 duration-100"
        >
          <DropdownMenu.Item className="flex items-center gap-2 text-[14px] px-3 py-2 outline-none cursor-pointer hover:bg-zinc-50 rounded-lg text-zinc-700 font-medium transition-colors">
            重命名
          </DropdownMenu.Item>
          <DropdownMenu.Item className="flex items-center gap-2 text-[14px] px-3 py-2 outline-none cursor-pointer hover:bg-red-50 hover:text-red-600 rounded-lg text-red-600 font-medium transition-colors">
            删除
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
};
