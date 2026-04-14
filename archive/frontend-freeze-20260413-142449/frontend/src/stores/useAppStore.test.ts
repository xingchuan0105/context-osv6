import { describe, it, expect } from 'vitest';
import { useAppStore } from './useAppStore';

describe('AppStore', () => {
  beforeEach(() => {
    // Reset store before each test
    useAppStore.setState({
      user: null,
      currentWorkspace: null,
      isAuthenticated: false,
    });
  });

  describe('User state', () => {
    it('should have initial user as null', () => {
      const { user } = useAppStore.getState();
      expect(user).toBeNull();
    });

    it('should set user correctly', () => {
      const { setUser } = useAppStore.getState();
      const testUser = { id: '123', email: 'test@example.com', full_name: 'Test' };
      
      setUser(testUser);
      
      const { user } = useAppStore.getState();
      expect(user).toEqual(testUser);
    });

    it('should set isAuthenticated when user is set', () => {
      const { setUser } = useAppStore.getState();
      const testUser = { id: '123', email: 'test@example.com' };
      
      setUser(testUser);
      
      const { isAuthenticated } = useAppStore.getState();
      expect(isAuthenticated).toBe(true);
    });

    it('should clear user', () => {
      const { setUser, clearUser } = useAppStore.getState();
      const testUser = { id: '123', email: 'test@example.com' };
      
      setUser(testUser);
      clearUser();
      
      const { user, isAuthenticated } = useAppStore.getState();
      expect(user).toBeNull();
      expect(isAuthenticated).toBe(false);
    });
  });

  describe('Workspace state', () => {
    it('should have initial workspace as null', () => {
      const { currentWorkspace } = useAppStore.getState();
      expect(currentWorkspace).toBeNull();
    });

    it('should set current workspace', () => {
      const { setCurrentWorkspace } = useAppStore.getState();
      const workspace = { id: 'kb-123', title: 'My Workspace', user_id: 'user-123', created_at: '2024-01-01' };
      
      setCurrentWorkspace(workspace);
      
      const { currentWorkspace } = useAppStore.getState();
      expect(currentWorkspace).toEqual(workspace);
    });

    it('should clear current workspace', () => {
      const { setCurrentWorkspace, clearCurrentWorkspace } = useAppStore.getState();
      const workspace = { id: 'kb-123', title: 'My Workspace', user_id: 'user-123', created_at: '2024-01-01' };
      
      setCurrentWorkspace(workspace);
      clearCurrentWorkspace();
      
      const { currentWorkspace } = useAppStore.getState();
      expect(currentWorkspace).toBeNull();
    });
  });

  describe('UI state', () => {
    it('should toggle search dialog', () => {
      const { toggleSearchDialog, searchDialogOpen } = useAppStore.getState();
      
      expect(searchDialogOpen).toBe(false);
      
      toggleSearchDialog();
      
      const { searchDialogOpen: open } = useAppStore.getState();
      expect(open).toBe(true);
    });

    it('should set sidebar collapsed', () => {
      const { setSidebarCollapsed } = useAppStore.getState();
      
      setSidebarCollapsed(true);
      
      const { sidebarCollapsed } = useAppStore.getState();
      expect(sidebarCollapsed).toBe(true);
    });
  });
});
