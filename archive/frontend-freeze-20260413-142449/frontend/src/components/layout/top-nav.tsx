'use client';

import Image from 'next/image';
import { Search, ChevronDown, User, Settings, LogOut } from 'lucide-react';
import { useAppStore } from '@/stores/useAppStore';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';

export function TopNav() {
  const { user, currentWorkspace, toggleSearchDialog } = useAppStore();

  return (
    <header className="h-14 border-b border-border bg-card flex items-center px-4 justify-between shrink-0">
      {/* Logo */}
      <div className="flex items-center gap-2">
        <div className="relative flex items-center justify-center">
          <Image
            src="/images/logo.png"
            alt="Context OS"
            width={32}
            height={32}
            className="w-8 h-8 object-contain rounded-md"
            priority
          />
        </div>
        <span className="font-semibold text-lg text-foreground">Context OS</span>
      </div>

      {/* Workspace Switcher */}
      <div className="flex-1 flex justify-center">
        <button className="flex items-center gap-2 px-3 py-1.5 rounded-lg hover:bg-accent transition-colors">
          <span className="text-sm text-foreground/90">
            {currentWorkspace?.title || '选择工作区'}
          </span>
          <ChevronDown className="w-4 h-4 text-muted-foreground" />
        </button>
      </div>

      {/* Right section */}
      <div className="flex items-center gap-2">
        {/* Search Button */}
        <Button
          variant="ghost"
          size="sm"
          onClick={toggleSearchDialog}
          className="text-muted-foreground hover:text-foreground/90"
        >
          <Search className="w-4 h-4 mr-2" />
          <span className="text-sm">搜索</span>
          <kbd className="ml-2 px-1.5 py-0.5 text-xs bg-accent rounded text-muted-foreground/80">
            ⌘K
          </kbd>
        </Button>

        {/* User Menu */}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="sm" className="relative h-8 w-8 rounded-full">
              <div className="w-8 h-8 rounded-full bg-indigo-500/20 flex items-center justify-center text-indigo-400 font-medium">
                {user?.full_name?.[0] || user?.email?.[0] || 'U'}
              </div>
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent className="w-56" align="end" forceMount>
            <DropdownMenuLabel className="font-normal">
              <div className="flex flex-col space-y-1">
                <p className="text-sm font-medium leading-none">{user?.full_name || '用户'}</p>
                <p className="text-xs leading-none text-muted-foreground">{user?.email}</p>
              </div>
            </DropdownMenuLabel>
            <DropdownMenuSeparator />
            <DropdownMenuItem>
              <User className="mr-2 h-4 w-4" />
              <span>个人设置</span>
            </DropdownMenuItem>
            <DropdownMenuItem>
              <Settings className="mr-2 h-4 w-4" />
              <span>主题设置</span>
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem className="text-red-400">
              <LogOut className="mr-2 h-4 w-4" />
              <span>退出登录</span>
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </header>
  );
}
