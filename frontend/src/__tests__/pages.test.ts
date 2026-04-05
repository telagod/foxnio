/**
 * @jest-environment jsdom
 */
import { render, screen } from '@testing-library/svelte/svelte5';
import { tick } from 'svelte';
import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock $app/navigation
const mockGoto = vi.fn();
vi.mock('$app/navigation', () => ({
  goto: (...args: unknown[]) => mockGoto(...args),
}));

// Mock $app/stores
vi.mock('$app/stores', () => {
  const { readable } = require('svelte/store');
  return {
    page: readable({ url: new URL('http://localhost'), params: {} }),
    navigating: readable(null),
    updated: readable(false),
  };
});

// Mock fetch globally
const mockFetch = vi.fn();
global.fetch = mockFetch;

// Import the api singleton so we can clear its cache between tests
import { api } from '$lib/api';
import Dashboard from '../routes/dashboard/+page.svelte';
import Usage from '../routes/usage/+page.svelte';
import ApiKeys from '../routes/apikeys/+page.svelte';
import Admin from '../routes/admin/+page.svelte';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function jsonResponse(body: unknown) {
  return Promise.resolve({
    ok: true,
    status: 200,
    json: () => Promise.resolve(body),
  });
}

function pendingForever() {
  return new Promise(() => {});
}

/** Flush Svelte 5 async reactivity in jsdom */
async function flush() {
  await tick();
  await new Promise((r) => setTimeout(r, 100));
  await tick();
}

// ---------------------------------------------------------------------------
// Dashboard page
// ---------------------------------------------------------------------------
describe('Dashboard page', () => {
  beforeEach(() => {
    mockFetch.mockReset();
    mockGoto.mockReset();
    localStorage.clear();
    localStorage.setItem('token', 'test-token');
    api.clearCache();
  });

  it('renders loading state initially', () => {
    mockFetch.mockImplementation(() => pendingForever());
    render(Dashboard);
    const status = document.querySelector('[role="status"]');
    expect(status).toBeTruthy();
    const spinner = document.querySelector('.animate-spin');
    expect(spinner).toBeTruthy();
  });

  it('redirects to login on API failure', async () => {
    mockFetch.mockRejectedValue(new Error('Network error'));
    render(Dashboard);
    await flush();
    expect(mockGoto).toHaveBeenCalledWith('/login');
  });

  it('renders user info after load', async () => {
    mockFetch.mockImplementation((url: string) => {
      if (url.includes('/user/me')) {
        return jsonResponse({ id: '1', email: 'test@example.com', balance: 1000, role: 'user', status: 'active', created_at: '2025-01-01' });
      }
      if (url.includes('/user/apikeys')) {
        return jsonResponse({ data: [] });
      }
      if (url.includes('/user/usage')) {
        return jsonResponse({ total_requests: 0, total_input_tokens: 0, total_output_tokens: 0, total_cost: 0, total_cost_yuan: 0, days: 30, total_tokens: 0, daily_usage: [] });
      }
      return jsonResponse({});
    });
    const { container } = render(Dashboard);
    await flush();
    expect(container.innerHTML).toContain('test@example.com');
  });
});

// ---------------------------------------------------------------------------
// Usage page
// ---------------------------------------------------------------------------
describe('Usage page', () => {
  beforeEach(() => {
    mockFetch.mockReset();
    localStorage.clear();
    localStorage.setItem('token', 'test-token');
    api.clearCache();
  });

  it('renders loading spinner', () => {
    mockFetch.mockImplementation(() => pendingForever());
    render(Usage);
    const status = document.querySelector('[role="status"]');
    expect(status).toBeTruthy();
    const spinner = document.querySelector('.animate-spin');
    expect(spinner).toBeTruthy();
  });

  it('shows period selector', () => {
    mockFetch.mockImplementation(() => pendingForever());
    render(Usage);
    const select = screen.getByLabelText('Select time period');
    expect(select).toBeTruthy();
  });

  it('shows empty state when no data', async () => {
    mockFetch.mockImplementation(() =>
      jsonResponse({
        days: 7, total_requests: 0, total_input_tokens: 0, total_output_tokens: 0,
        total_tokens: 0, total_cost: 0, total_cost_yuan: 0, daily_usage: [],
      })
    );
    const { container } = render(Usage);
    await flush();
    expect(container.innerHTML).toContain('No Usage Data Yet');
  });
});

// ---------------------------------------------------------------------------
// API Keys page
// ---------------------------------------------------------------------------
describe('API Keys page', () => {
  beforeEach(() => {
    mockFetch.mockReset();
    localStorage.clear();
    localStorage.setItem('token', 'test-token');
    api.clearCache();
  });

  it('renders loading state', () => {
    mockFetch.mockImplementation(() => pendingForever());
    render(ApiKeys);
    const spinner = document.querySelector('.animate-spin');
    expect(spinner).toBeTruthy();
  });

  it('shows Create New Key button', () => {
    mockFetch.mockImplementation(() => pendingForever());
    render(ApiKeys);
    const button = screen.getByRole('button', { name: /create new key/i });
    expect(button).toBeTruthy();
  });

  it('shows empty state when no keys', async () => {
    mockFetch.mockImplementation(() => jsonResponse({ data: [] }));
    const { container } = render(ApiKeys);
    await flush();
    expect(container.innerHTML).toContain('No API Keys Yet');
  });
});

// ---------------------------------------------------------------------------
// Admin page
// ---------------------------------------------------------------------------
describe('Admin page', () => {
  beforeEach(() => {
    mockFetch.mockReset();
    localStorage.clear();
    localStorage.setItem('token', 'test-token');
    api.clearCache();
  });

  it('renders loading state', () => {
    mockFetch.mockImplementation(() => pendingForever());
    render(Admin);
    const status = document.querySelector('[role="status"]');
    expect(status).toBeTruthy();
    const spinner = document.querySelector('.animate-spin');
    expect(spinner).toBeTruthy();
  });

  it('shows refresh button', () => {
    mockFetch.mockImplementation(() => pendingForever());
    render(Admin);
    expect(screen.getByText('刷新')).toBeTruthy();
  });

  it('shows stat cards structure after load', async () => {
    const statsResponse = {
      users: { total: 100, active: 50, new_today: 5, new_this_week: 20, new_this_month: 40 },
      accounts: { total: 10, active: 8, healthy: 7, by_platform: [] },
      api_keys: { total: 30, active: 25, expiring_soon: 2 },
      usage: { total_requests: 5000, total_tokens: 100000, total_cost: 50.0, today_requests: 200, today_tokens: 4000, today_cost: 2.0 },
      updated_at: '2025-06-01T00:00:00Z',
    };
    const chartResponse = { labels: [], datasets: [] };
    const distResponse = { labels: [], data: [], total: 0 };

    mockFetch.mockImplementation((url: string) => {
      if (url.includes('/dashboard/stats')) return jsonResponse(statsResponse);
      if (url.includes('/dashboard/trend')) return jsonResponse(chartResponse);
      if (url.includes('/dashboard/line')) return jsonResponse(chartResponse);
      if (url.includes('/dashboard/pie')) return jsonResponse(chartResponse);
      if (url.includes('/model-distribution')) return jsonResponse(distResponse);
      if (url.includes('/platform-distribution')) return jsonResponse(distResponse);
      return jsonResponse({});
    });

    const { container } = render(Admin);
    await flush();
    expect(container.innerHTML).toContain('总用户');
    expect(container.innerHTML).toContain('活跃账号');
    expect(container.innerHTML).toContain('今日请求');
    expect(container.innerHTML).toContain('累计费用');
  });
});
