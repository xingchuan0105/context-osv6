import { create } from 'zustand';
import type { Admin } from '@/lib/api/admin';
import { adminAuthApi, setAdminToken, clearAdminToken, getAdminToken } from '@/lib/api/admin';

interface AdminState {
  admin: Admin | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  error: string | null;
  setAdmin: (admin: Admin | null) => void;
  login: (email: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
  checkAuth: () => Promise<void>;
  clearError: () => void;
}

export const useAdminStore = create<AdminState>((set) => ({
  admin: null,
  isAuthenticated: false,
  isLoading: false,
  error: null,

  setAdmin: (admin) => set({ admin, isAuthenticated: !!admin }),

  login: async (email, password) => {
    set({ isLoading: true, error: null });
    try {
      const response = await adminAuthApi.login(email, password);
      if (response.success && response.data) {
        setAdminToken(response.data.token);
        set({ admin: response.data.user, isAuthenticated: true, isLoading: false });
      } else {
        set({ error: response.error || 'Login failed', isLoading: false });
      }
    } catch (error) {
      set({ error: error instanceof Error ? error.message : 'Login failed', isLoading: false });
    }
  },

  logout: async () => {
    try {
      await adminAuthApi.logout();
    } finally {
      clearAdminToken();
      set({ admin: null, isAuthenticated: false });
    }
  },

  checkAuth: async () => {
    const token = getAdminToken();
    if (!token) {
      set({ isAuthenticated: false, admin: null });
      return;
    }

    set({ isLoading: true });
    try {
      const response = await adminAuthApi.me();
      if (response.success && response.data) {
        set({ admin: response.data, isAuthenticated: true, isLoading: false });
      } else {
        clearAdminToken();
        set({ admin: null, isAuthenticated: false, isLoading: false });
      }
    } catch {
      clearAdminToken();
      set({ admin: null, isAuthenticated: false, isLoading: false });
    }
  },

  clearError: () => set({ error: null }),
}));
