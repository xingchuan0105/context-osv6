/**
 * Admin API Client - 管理员模块 API
 * 
 * 功能：
 * - 管理员登录/登出
 * - 管理员账户管理
 * - 系统指标查询
 * - Token 用量统计
 * - 用户活动统计
 * - 用户管理
 * - 操作日志查询
 */

import axios from 'axios';

// Use relative URL to go through Next.js proxy
const API_BASE_URL = '';

const adminApi = axios.create({
  baseURL: API_BASE_URL,
  timeout: 30000,
  headers: {
    'Content-Type': 'application/json',
  },
});

// Request interceptor to add admin token
adminApi.interceptors.request.use(
  (config) => {
    if (typeof window !== 'undefined') {
      const token = localStorage.getItem('admin_token') || localStorage.getItem('token');
      if (token) {
        config.headers.Authorization = `Bearer ${token}`;
      }
    }
    return config;
  },
  (error) => Promise.reject(error)
);

// Response interceptor for error handling
adminApi.interceptors.response.use(
  (response) => response,
  (error) => Promise.reject(error)
);

// Types
export interface Admin {
  id: string;
  email: string;
  full_name?: string;
  role: 'owner' | 'admin' | 'member';
  email_verified_at?: string | null;
  created_at: string;
}

export interface AdminLoginRequest {
  email: string;
  password: string;
}

export interface AdminLoginResponse {
  success: boolean;
  data?: {
    token: string;
    user: Admin;
  };
  error?: string;
}

export interface MetricsData {
  database: boolean;
  active_tenants: number;
  active_subscriptions: number;
  documents: number;
  generated_at: string;
  observability: {
    summary: {
      eino_graph_compiles_total: number;
      eino_runs_total: number;
      eino_run_errors_total: number;
      eino_alerts_total: number;
      fallback_events_total: number;
    };
    eino_graph_compile_total: Record<string, number>;
    eino_graph_run_total: Record<string, number>;
    eino_graph_run_error_total: Record<string, number>;
    eino_node_latency_ms_total: Record<string, number>;
    eino_alert_total: Record<string, number>;
    fallback_events_total: Record<string, number>;
  };
}

export interface TokenStats {
  period_start: string;
  period_end: string;
  metric_type: string;
  quantity: number;
}

export interface User {
  id: string;
  email: string;
  full_name: string;
  role: string;
  email_verified_at?: string | null;
  created_at: string;
}

export interface OperationLog {
  id: string;
  admin_id: string;
  admin_email: string;
  action: string;
  target_type: string;
  target_id: string | null;
  details: string | null;
  ip_address: string | null;
  created_at: string;
}

export interface PaginatedResponse<T> {
  success: boolean;
  data: T[];
  meta: {
    total: number;
    page: number;
    limit: number;
  };
}

export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}

// Admin Auth API
export const adminAuthApi = {
  login: async (email: string, password: string): Promise<AdminLoginResponse> => {
    const response = await adminApi.post<ApiResponse<{ token: string; user: { id: string; email: string } }>>('/api/auth/login', { email, password });
    if (response.data.success && response.data.data?.token) {
      if (typeof window !== 'undefined') {
        localStorage.setItem('admin_token', response.data.data.token);
      }
      return {
        success: true,
        data: {
          token: response.data.data.token,
          user: {
            id: response.data.data.user.id,
            email: response.data.data.user.email,
            full_name: '',
            role: 'admin',
            email_verified_at: null,
            created_at: new Date().toISOString(),
          },
        },
      };
    }
    return { success: false, error: response.data.error || 'Login failed' };
  },

  logout: async (): Promise<void> => {
    try {
      await adminApi.post('/api/auth/logout');
    } finally {
      if (typeof window !== 'undefined') {
        localStorage.removeItem('admin_token');
      }
    }
  },

  me: async (): Promise<ApiResponse<Admin>> => {
    const response = await adminApi.get<ApiResponse<{ token: string; user: { id: string; email: string } }>>('/api/auth/me');
    if (response.data.success && response.data.data?.user) {
      return {
        success: true,
        data: {
          id: response.data.data.user.id,
          email: response.data.data.user.email,
          full_name: '',
          role: 'admin',
          email_verified_at: null,
          created_at: new Date().toISOString(),
        },
      };
    }
    return { success: false, error: response.data.error || 'Unauthorized' };
  },
};

// Admin Management API
export const adminManagementApi = {
  listAdmins: async (page = 1, limit = 20): Promise<PaginatedResponse<Admin>> => {
    const response = await adminApi.get<PaginatedResponse<Admin>>('/api/v1/admin/admins', {
      params: { page, limit },
    });
    return response.data;
  },

  createAdmin: async (data: {
    email: string;
    password: string;
    full_name?: string;
  }): Promise<ApiResponse<Admin>> => {
    const response = await adminApi.post<ApiResponse<Admin>>('/api/v1/admin/admins', data);
    return response.data;
  },

  deleteAdmin: async (id: string): Promise<ApiResponse<void>> => {
    const response = await adminApi.delete<ApiResponse<void>>(`/api/v1/admin/admins/${id}`);
    return response.data;
  },
};

// Metrics API
export const metricsApi = {
  getMetrics: async (): Promise<ApiResponse<MetricsData>> => {
    const response = await adminApi.get<MetricsData>('/api/v1/admin/health');
    return { success: true, data: response.data };
  },
};

// Token Stats API
export const tokenStatsApi = {
  getTokenStats: async (params?: {
    start_date?: string;
    end_date?: string;
    page?: number;
    limit?: number;
  }): Promise<PaginatedResponse<TokenStats>> => {
    const response = await adminApi.get<PaginatedResponse<TokenStats>>('/api/v1/admin/token-stats', { params });
    return response.data;
  },
};

// User Management API
export const userManagementApi = {
  listUsers: async (params?: {
    page?: number;
    limit?: number;
    search?: string;
  }): Promise<PaginatedResponse<User>> => {
    const response = await adminApi.get<PaginatedResponse<User>>('/api/v1/admin/users', { params });
    return response.data;
  },
};

// Operation Logs API
export const operationLogsApi = {
  getOperationLogs: async (params?: {
    admin_id?: string;
    action?: string;
    start_date?: string;
    end_date?: string;
    page?: number;
    limit?: number;
  }): Promise<PaginatedResponse<OperationLog>> => {
    const response = await adminApi.get<PaginatedResponse<OperationLog>>('/api/v1/admin/logs', { params });
    return response.data;
  },
};

// Helper functions
export function setAdminToken(token: string): void {
  if (typeof window !== 'undefined') {
    localStorage.setItem('admin_token', token);
  }
}

export function clearAdminToken(): void {
  if (typeof window !== 'undefined') {
    localStorage.removeItem('admin_token');
  }
}

export function getAdminToken(): string | null {
  if (typeof window !== 'undefined') {
    return localStorage.getItem('admin_token');
  }
  return null;
}

export function isAdminAuthenticated(): boolean {
  return !!getAdminToken();
}
