/**
 * @jest-environment jsdom
 */
import { render, screen } from '@testing-library/svelte/svelte5';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import Dashboard from '../routes/admin/+page.svelte';
import ApiKeys from '../routes/apikeys/+page.svelte';

// Mock fetch
const mockFetch = vi.fn();
global.fetch = mockFetch;

describe('Dashboard', () => {
  beforeEach(() => {
    mockFetch.mockReset();
  });

  it('shows loading state initially', () => {
    mockFetch.mockImplementation(() => new Promise(() => {})); // Never resolves
    render(Dashboard);
    const spinner = document.querySelector('.animate-spin');
    expect(spinner).toBeTruthy();
  });

  it('renders the refresh button', () => {
    mockFetch.mockImplementation(() => new Promise(() => {})); // Never resolves
    render(Dashboard);
    expect(screen.getByText('刷新')).toBeTruthy();
  });

  it('renders the dashboard title', () => {
    mockFetch.mockImplementation(() => new Promise(() => {})); // Never resolves
    render(Dashboard);
    expect(screen.getByText('管理控制面')).toBeTruthy();
  });
});

describe('API Keys', () => {
  beforeEach(() => {
    mockFetch.mockReset();
  });

  it('shows loading state initially', () => {
    mockFetch.mockImplementation(() => new Promise(() => {}));
    render(ApiKeys);
    const spinner = document.querySelector('.animate-spin');
    expect(spinner).toBeTruthy();
  });

  it('renders the page title', () => {
    mockFetch.mockImplementation(() => new Promise(() => {}));
    render(ApiKeys);
    expect(screen.getByText('API Keys')).toBeTruthy();
  });

  it('has create new key button', () => {
    mockFetch.mockImplementation(() => new Promise(() => {}));
    render(ApiKeys);
    // The button should always be present in the header
    const button = screen.getByRole('button', { name: /create new key/i });
    expect(button).toBeTruthy();
  });
});
