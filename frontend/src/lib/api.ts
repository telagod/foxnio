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
    const headers: HeadersInit = {
      'Content-Type': 'application/json',
      ...options.headers,
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

  // Alerts
  async listAlertRules(): Promise<{ rules: AlertRule[] }> {
    return this.request('/api/v1/alerts/rules');
  }

  async createAlertRule(rule: Partial<AlertRule>): Promise<AlertRule> {
    return this.request('/api/v1/alerts/rules', {
      method: 'POST',
      body: JSON.stringify(rule),
    });
  }

  async updateAlertRule(id: string, rule: Partial<AlertRule>): Promise<AlertRule> {
    return this.request(`/api/v1/alerts/rules/${id}`, {
      method: 'PUT',
      body: JSON.stringify(rule),
    });
  }

  async deleteAlertRule(id: string): Promise<void> {
    return this.request(`/api/v1/alerts/rules/${id}`, {
      method: 'DELETE',
    });
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

export const api = new ApiClient();
