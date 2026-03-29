// API 客户端

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

class ApiClient {
  private token: string | null = null;

  setToken(token: string) {
    this.token = token;
  }

  private async request<T>(
    path: string,
    options: RequestInit = {}
  ): Promise<T> {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      ...(options.headers as Record<string, string>),
    };

    if (this.token) {
      headers['Authorization'] = `Bearer ${this.token}`;
    }

    const response = await fetch(`${API_BASE}${path}`, {
      ...options,
      headers,
    });

    if (!response.ok) {
      const error = await response.json().catch(() => ({ error: 'Unknown error' }));
      throw new Error(error.error || `HTTP ${response.status}`);
    }

    return response.json();
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

  // Admin
  async listUsers(): Promise<{ data: User[] }> {
    return this.request('/api/v1/admin/users');
  }

  async listAccounts(): Promise<{ data: Account[] }> {
    return this.request('/api/v1/admin/accounts');
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
