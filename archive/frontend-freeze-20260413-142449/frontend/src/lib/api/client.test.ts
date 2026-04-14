import { describe, it, expect, vi, beforeEach } from 'vitest';
import { api, setAuthToken, clearAuthToken } from '@/lib/api/client';

describe('API Client', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(localStorage.getItem).mockReturnValue(null);
  });

  describe('setAuthToken', () => {
    it('should set auth token in localStorage', () => {
      setAuthToken('test-token');
      expect(localStorage.setItem).toHaveBeenCalledWith('token', 'test-token');
    });
  });

  describe('clearAuthToken', () => {
    it('should clear auth token from localStorage', () => {
      clearAuthToken();
      expect(localStorage.removeItem).toHaveBeenCalledWith('token');
    });
  });

  describe('Axios instance', () => {
    it('should have baseURL configured', () => {
      expect(api.defaults.baseURL).toBeDefined();
    });

    it('should have timeout configured', () => {
      expect(api.defaults.timeout).toBe(30000);
    });

    it('should have headers configured', () => {
      expect(api.defaults.headers['Content-Type']).toBe('application/json');
    });
  });
});
