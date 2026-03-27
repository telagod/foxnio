/**
 * @jest-environment jsdom
 */
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import Dashboard from '../src/routes/admin/+page.svelte';
import ApiKeys from '../src/routes/apikeys/+page.svelte';

// Mock fetch
global.fetch = vi.fn();

describe('Dashboard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows loading state initially', () => {
    render(Dashboard);
    expect(screen.getByRole('status') || document.querySelector('.animate-spin')).toBeTruthy();
  });

  it('displays stats after loading', async () => {
    (global.fetch as any).mockResolvedValueOnce({
      ok: true,
      json: async () => ({
        total_users: 100,
        total_accounts: 50,
        total_requests_today: 1000,
        total_revenue: 50000,
        active_users: 75,
        active_accounts: 40,
      }),
    });

    render(Dashboard);

    await waitFor(() => {
      expect(screen.getByText('100')).toBeTruthy();
      expect(screen.getByText('50')).toBeTruthy();
      expect(screen.getByText('1.0K')).toBeTruthy();
    });
  });

  it('shows error message on fetch failure', async () => {
    (global.fetch as any).mockRejectedValueOnce(new Error('Network error'));

    render(Dashboard);

    await waitFor(() => {
      expect(screen.getByText(/error loading data/i)).toBeTruthy();
    });
  });

  it('refreshes stats on button click', async () => {
    (global.fetch as any).mockResolvedValue({
      ok: true,
      json: async () => ({
        total_users: 100,
        total_accounts: 50,
        total_requests_today: 1000,
        total_revenue: 50000,
        active_users: 75,
        active_accounts: 40,
      }),
    });

    render(Dashboard);

    await waitFor(() => {
      expect(screen.getByText('Refresh')).toBeTruthy();
    });

    const refreshButton = screen.getByText('Refresh');
    await fireEvent.click(refreshButton);

    expect(global.fetch).toHaveBeenCalledTimes(2);
  });
});

describe('API Keys', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows empty state when no keys', async () => {
    (global.fetch as any).mockResolvedValueOnce({
      ok: true,
      json: async () => ({ data: [] }),
    });

    render(ApiKeys);

    await waitFor(() => {
      expect(screen.getByText(/no api keys yet/i)).toBeTruthy();
    });
  });

  it('displays API keys list', async () => {
    const mockKeys = [
      {
        id: '1',
        key: 'sk-test123',
        name: 'Test Key',
        status: 'active',
        created_at: '2024-01-01T00:00:00Z',
        last_used_at: null,
      },
    ];

    (global.fetch as any).mockResolvedValueOnce({
      ok: true,
      json: async () => ({ data: mockKeys }),
    });

    render(ApiKeys);

    await waitFor(() => {
      expect(screen.getByText('Test Key')).toBeTruthy();
      expect(screen.getByText('sk-test123')).toBeTruthy();
    });
  });

  it('filters keys by search term', async () => {
    const mockKeys = [
      { id: '1', key: 'sk-prod', name: 'Production', status: 'active', created_at: '2024-01-01', last_used_at: null },
      { id: '2', key: 'sk-dev', name: 'Development', status: 'active', created_at: '2024-01-01', last_used_at: null },
    ];

    (global.fetch as any).mockResolvedValueOnce({
      ok: true,
      json: async () => ({ data: mockKeys }),
    });

    render(ApiKeys);

    await waitFor(() => {
      expect(screen.getByText('Production')).toBeTruthy();
      expect(screen.getByText('Development')).toBeTruthy();
    });

    const searchInput = screen.getByPlaceholderText(/search/i);
    await fireEvent.input(searchInput, { target: { value: 'prod' } });

    await waitFor(() => {
      expect(screen.getByText('Production')).toBeTruthy();
      expect(screen.queryByText('Development')).toBeFalsy();
    });
  });

  it('shows create modal on button click', async () => {
    (global.fetch as any).mockResolvedValueOnce({
      ok: true,
      json: async () => ({ data: [] }),
    });

    render(ApiKeys);

    await waitFor(() => {
      expect(screen.getByText(/create new key/i)).toBeTruthy();
    });

    const createButton = screen.getByText(/create new key/i);
    await fireEvent.click(createButton);

    expect(screen.getByText(/create new api key/i)).toBeTruthy();
  });

  it('creates new key on form submit', async () => {
    (global.fetch as any)
      .mockResolvedValueOnce({
        ok: true,
        json: async () => ({ data: [] }),
      })
      .mockResolvedValueOnce({
        ok: true,
        json: async () => ({ id: '1', key: 'sk-new', name: 'New Key' }),
      })
      .mockResolvedValueOnce({
        ok: true,
        json: async () => ({ data: [{ id: '1', key: 'sk-new', name: 'New Key', status: 'active', created_at: '2024-01-01', last_used_at: null }] }),
      });

    render(ApiKeys);

    await waitFor(() => {
      expect(screen.getByText(/create new key/i)).toBeTruthy();
    });

    const createButton = screen.getByText(/create new key/i);
    await fireEvent.click(createButton);

    const nameInput = screen.getByPlaceholderText(/production key/i);
    await fireEvent.input(nameInput, { target: { value: 'New Key' } });

    const submitButton = screen.getByText('Create');
    await fireEvent.click(submitButton);

    await waitFor(() => {
      expect(global.fetch).toHaveBeenCalledWith(
        '/api/v1/user/apikeys',
        expect.objectContaining({ method: 'POST' })
      );
    });
  });
});
