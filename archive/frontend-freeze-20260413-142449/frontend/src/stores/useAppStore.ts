/**
 * useAppStore - 全局应用状态管理
 *
 * 管理内容：
 * - 用户认证状态（user, isAuthenticated）
 * - 当前工作区（currentWorkspace），对应后端 PRD 中的 notebook
 * - UI 状态（搜索对话框、侧边栏折叠）
 *
 * 使用方式：
 * const { user, currentWorkspace, setUser } = useAppStore();
 */

import { create } from 'zustand';
import type { User, KnowledgeBase } from '@/types';

interface AppState {
  // Auth
  user: User | null;
  isAuthenticated: boolean;

  // Workspace (UI term) / Notebook (backend term)
  currentWorkspace: KnowledgeBase | null;

  // UI
  searchDialogOpen: boolean;
  sidebarCollapsed: boolean;

  // Actions
  setUser: (user: User | null) => void;
  clearUser: () => void;
  setCurrentWorkspace: (workspace: KnowledgeBase | null) => void;
  clearCurrentWorkspace: () => void;
  toggleSearchDialog: () => void;
  setSearchDialogOpen: (open: boolean | ((prev: boolean) => boolean)) => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
}

export const useAppStore = create<AppState>((set) => ({
  // Initial state
  user: null,
  isAuthenticated: false,
  currentWorkspace: null,
  searchDialogOpen: false,
  sidebarCollapsed: false,

  // Actions
  setUser: (user) => set({ user, isAuthenticated: !!user }),

  clearUser: () => set({ user: null, isAuthenticated: false, currentWorkspace: null }),

  setCurrentWorkspace: (workspace) => set({ currentWorkspace: workspace }),

  clearCurrentWorkspace: () => set({ currentWorkspace: null }),

  toggleSearchDialog: () => set((state) => ({ searchDialogOpen: !state.searchDialogOpen })),

  setSearchDialogOpen: (open) =>
    set((state) => ({
      searchDialogOpen: typeof open === 'function' ? open(state.searchDialogOpen) : open,
    })),

  setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),
}));
