// API 客户端 - 优化版
// 支持：分页、请求去重、防抖

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:8080';

export interface User {
  id: string;
  email: string;
  balance: number;
  role: string;
  status: string;
  created_at: string;
}

export interface ApiKey {
  id: string;
  user_id: string;
  key: string;
  name: string | null;
  status: string;
  created_at: string;
  last_used_at: string | null;
}

export interface Account {
  id: string;
  name: string;
  provider: string;
  credential_type: string;
  status: string;
  priority?: number;
  last_error?: string | null;
  created_at: string;
}

export interface Usage {
  id: string;
  user_id: string;
  model: string;
  input_tokens: number;
  output_tokens: number;
  cost: number;
  created_at: string;
}

// 分页参数接口
export interface PaginationParams {
  page?: number;
  per_page?: number;
  status?: string;
  provider?: string;
  search?: string;
}

// 分页响应接口
export interface PaginatedResponse<T> {
  data: T[];
  total: number;
  page: number;
  per_page: number;
  total_pages: number;
}

// 防抖函数
export function debounce<T extends (...args: any[]) => any>(
  fn: T,
  delay: number
): (...args: Parameters<T>) => void {
  let timeoutId: ReturnType<typeof setTimeout> | null = null;
  
  return function (this: any, ...args: Parameters<T>) {
    if (timeoutId) {
      clearTimeout(timeoutId);
    }
    timeoutId = setTimeout(() => {
      fn.apply(this, args);
      timeoutId = null;
    }, delay);
  };
}

// 节流函数
export function throttle<T extends (...args: any[]) => any>(
  fn: T,
  limit: number
): (...args: Parameters<T>) => void {
  let inThrottle = false;
  
  return function (this: any, ...args: Parameters<T>) {
    if (!inThrottle) {
      fn.apply(this, args);
      inThrottle = true;
      setTimeout(() => {
        inThrottle = false;
      }, limit);
    }
  };
}

class ApiClient {
  private token: string | null = null;
  // 请求去重：存储正在进行的请求
  private pendingRequests = new Map<string, Promise<any>>();
  // 缓存：短期缓存 GET 请求结果
  private cache = new Map<string, { data: any; expires: number }>();
  private cacheTTL = 5000; // 5秒缓存

  setToken(token: string) {
    this.token = token;
    // Token 变化时清除缓存
    this.cache.clear();
  }

  // 生成请求唯一键
  private getRequestKey(path: string, options: RequestInit = {}): string {
    const method = options.method || 'GET';
    const body = options.body ? JSON.stringify(options.body) : '';
    return `${method}:${path}:${body}`;
  }

  // 清除缓存
  clearCache(pattern?: string) {
    if (pattern) {
      // 清除匹配的缓存
      for (const key of this.cache.keys()) {
        if (key.includes(pattern)) {
          this.cache.delete(key);
        }
      }
    } else {
      this.cache.clear();
    }
  }

  private async request<T>(
    path: string,
    options: RequestInit = {}
  ): Promise<T> {
    const key = this.getRequestKey(path, options);
    const method = options.method || 'GET';

    // GET 请求检查缓存
    if (method === 'GET') {
      const cached = this.cache.get(key);
      if (cached && cached.expires > Date.now()) {
        return cached.data;
      }
    }

    // 请求去重：如果已有相同请求在进行，返回同一个 Promise
    if (this.pendingRequests.has(key)) {
      return this.pendingRequests.get(key)!;
    }

    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      ...(options.headers as Record<string, string>),
    };

    if (this.token) {
      headers['Authorization'] = `Bearer ${this.token}`;
    }

    const requestPromise = (async () => {
      try {
        const response = await fetch(`${API_BASE}${path}`, {
          ...options,
          headers,
        });

        if (!response.ok) {
          const error = await response.json().catch(() => ({ error: 'Unknown error' }));
          throw new Error(error.error || `HTTP ${response.status}`);
        }

        const data = await response.json();

        // GET 请求缓存结果
        if (method === 'GET') {
          this.cache.set(key, {
            data,
            expires: Date.now() + this.cacheTTL,
          });
        } else {
          // 非 GET 请求清除相关缓存
          this.clearCache(path);
        }

        return data;
      } finally {
        // 请求完成后移除
        this.pendingRequests.delete(key);
      }
    })();

    this.pendingRequests.set(key, requestPromise);
    return requestPromise;
  }

  // Auth
  async login(email: string, password: string): Promise<{ token: string; user: User }> {
    return this.request('/api/v1/auth/login', {
      method: 'POST',
      body: JSON.stringify({ email, password }),
    });
  }

  async register(email: string, password: string): Promise<{ token: string; user: User }> {
    return this.request('/api/v1/auth/register', {
      method: 'POST',
      body: JSON.stringify({ email, password }),
    });
  }

  // User
  async getMe(): Promise<User> {
    return this.request('/api/v1/user/me');
  }

  // API Keys
  async listApiKeys(): Promise<{ data: ApiKey[] }> {
    return this.request('/api/v1/user/apikeys');
  }

  async createApiKey(name?: string): Promise<ApiKey> {
    return this.request('/api/v1/user/apikeys', {
      method: 'POST',
      body: JSON.stringify({ name }),
    });
  }

  async deleteApiKey(id: string): Promise<void> {
    return this.request(`/api/v1/user/apikeys/${id}`, {
      method: 'DELETE',
    });
  }

  // Admin - Users
  async listUsers(params?: PaginationParams): Promise<PaginatedResponse<User>> {
    const query = this.buildQuery(params);
    return this.request(`/api/v1/admin/users?${query}`);
  }

  // Admin - Accounts (带分页)
  async listAccounts(params?: PaginationParams): Promise<PaginatedResponse<Account>> {
    const query = this.buildQuery(params);
    return this.request(`/api/v1/admin/accounts?${query}`);
  }

  // 获取单个账号详情
  async getAccount(id: string): Promise<Account> {
    return this.request(`/api/v1/admin/accounts/${id}`);
  }

  // 创建账号
  async createAccount(account: Partial<Account> & { credential: string }): Promise<Account> {
    return this.request('/api/v1/admin/accounts', {
      method: 'POST',
      body: JSON.stringify(account),
    });
  }

  // 更新账号
  async updateAccount(id: string, updates: Partial<Account>): Promise<Account> {
    return this.request(`/api/v1/admin/accounts/${id}`, {
      method: 'PATCH',
      body: JSON.stringify(updates),
    });
  }

  // 删除账号
  async deleteAccount(id: string): Promise<void> {
    return this.request(`/api/v1/admin/accounts/${id}`, {
      method: 'DELETE',
    });
  }

  // 批量操作
  async batchCreateAccounts(accounts: Array<Partial<Account> & { credential: string }>): Promise<{
    success: boolean;
    total: number;
    succeeded: number;
    failed: number;
    account_ids: string[];
    errors: string[];
  }> {
    return this.request('/api/v1/admin/accounts/batch', {
      method: 'POST',
      body: JSON.stringify({ accounts }),
    });
  }

  async batchDeleteAccounts(ids: string[]): Promise<{
    success: boolean;
    total: number;
    succeeded: number;
    failed: number;
  }> {
    return this.request('/api/v1/admin/accounts/batch-delete', {
      method: 'POST',
      body: JSON.stringify({ account_ids: ids }),
    });
  }

  // 构建查询字符串
  private buildQuery(params?: PaginationParams): string {
    if (!params) return '';
    
    const searchParams = new URLSearchParams();
    
    if (params.page) searchParams.set('page', params.page.toString());
    if (params.per_page) searchParams.set('per_page', params.per_page.toString());
    if (params.status) searchParams.set('status', params.status);
    if (params.provider) searchParams.set('provider', params.provider);
    if (params.search) searchParams.set('search', params.search);
    
    return searchParams.toString();
  }

  // Alerts - 使用管理员路径
  async listAlertRules(): Promise<{ rules: AlertRule[] }> {
    return this.request('/api/v1/admin/alerts/rules');
  }

  async createAlertRule(rule: Partial<AlertRule>): Promise<AlertRule> {
    return this.request('/api/v1/admin/alerts/rules', {
      method: 'POST',
      body: JSON.stringify(rule),
    });
  }

  async updateAlertRule(id: string, rule: Partial<AlertRule>): Promise<AlertRule> {
    return this.request(`/api/v1/admin/alerts/rules/${id}`, {
      method: 'PUT',
      body: JSON.stringify(rule),
    });
  }

  async deleteAlertRule(id: string): Promise<void> {
    return this.request(`/api/v1/admin/alerts/rules/${id}`, {
      method: 'DELETE',
    });
  }

  // Alert channels
  async listAlertChannels(): Promise<{ channels: AlertChannel[] }> {
    return this.request('/api/v1/admin/alerts/channels');
  }

  async createAlertChannel(channel: Partial<AlertChannel>): Promise<AlertChannel> {
    return this.request('/api/v1/admin/alerts/channels', {
      method: 'POST',
      body: JSON.stringify(channel),
    });
  }

  async deleteAlertChannel(id: string): Promise<void> {
    return this.request(`/api/v1/admin/alerts/channels/${id}`, {
      method: 'DELETE',
    });
  }

  // Alert history
  async listAlertHistory(): Promise<{ history: AlertHistory[] }> {
    return this.request('/api/v1/admin/alerts/history');
  }

  // Alert stats
  async getAlertStats(): Promise<AlertStats> {
    return this.request('/api/v1/admin/alerts/stats');
  }
}

export interface AlertRule {
  id: string;
  name: string;
  type: 'usage' | 'balance' | 'error_rate' | 'latency';
  threshold: number;
  operator: 'gt' | 'lt' | 'eq' | 'gte' | 'lte';
  duration_minutes: number;
  channels: ('email' | 'webhook' | 'slack')[];
  enabled: boolean;
  created_at: string;
  last_triggered_at: string | null;
}

export interface AlertChannel {
  id: string;
  name: string;
  type: 'email' | 'webhook' | 'slack' | 'dingtalk' | 'feishu';
  config: Record<string, any>;
  enabled: boolean;
  created_at: string;
}

export interface AlertHistory {
  id: string;
  rule_id: string;
  rule_name: string;
  triggered_at: string;
  resolved_at: string | null;
  status: 'firing' | 'resolved';
  message: string;
}

export interface AlertStats {
  total_rules: number;
  active_rules: number;
  total_alerts_today: number;
  total_alerts_week: number;
}

export const api = new ApiClient();
