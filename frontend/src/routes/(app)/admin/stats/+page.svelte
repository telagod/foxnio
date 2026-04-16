<svelte:head>
  <title>Statistics - Admin</title>
</svelte:head>

<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type AdminDashboardStats } from '$lib/api';

  const emptyStats: AdminDashboardStats = {
    users: { total: 0, active: 0, new_today: 0, new_this_week: 0, new_this_month: 0 },
    accounts: { total: 0, active: 0, healthy: 0, by_platform: [] },
    api_keys: { total: 0, active: 0, expiring_soon: 0 },
    usage: { total_requests: 0, total_tokens: 0, total_cost: 0, today_requests: 0, today_tokens: 0, today_cost: 0 },
    ops: {
      active_users_24h: 0,
      error_rate_1h: 0,
      avg_response_time_ms: 0,
      cache_hit_rate: 0,
      batch_operations_total: 0,
      batch_errors_total: 0,
      latest_fast_import_throughput: 0,
      latest_fast_import_preview_throughput: 0,
      latest_fast_import_size: 0,
      latest_fast_import_preview_size: 0
    },
    updated_at: ''
  };

  let stats = $state<AdminDashboardStats>(emptyStats);
  let loading = $state(true);
  let error = $state<string | null>(null);

  onMount(() => {
    const token = localStorage.getItem('token');
    if (token) api.setToken(token);
    loadStats();
  });

  async function loadStats() {
    loading = true; error = null;
    try {
      stats = await api.getAdminDashboardStats();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load statistics';
    } finally { loading = false; }
  }

  function fmt(n: number): string {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
    if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
    return `${n}`;
  }
  function yuan(v: number): string { return `¥${v.toFixed(2)}`; }
  function percent(v: number): string { return `${(v * 100).toFixed(1)}%`; }
  function ms(v: number): string { return `${v.toFixed(1)} ms`; }
  function formatDate(v: string): string { return v ? new Date(v).toLocaleString('zh-CN') : '-'; }
</script>

<div class="space-y-6">
  <!-- Header -->
  <div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
    <div>
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Statistics</h1>
      <p class="mt-1 text-gray-500 dark:text-gray-400">System-wide statistics and metrics</p>
    </div>
    <button onclick={loadStats} disabled={loading}
      class="inline-flex items-center justify-center gap-2 rounded-lg bg-blue-500 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-600 disabled:cursor-not-allowed disabled:opacity-50"
      aria-label="Refresh">
      <svg class="h-4 w-4" class:animate-spin={loading} fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" stroke-linecap="round" stroke-linejoin="round">
        <path d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"></path>
      </svg>
      Refresh
    </button>
  </div>

  {#if error}
    <div class="flex items-start gap-3 rounded-lg border border-red-200 bg-red-50 p-4 dark:border-red-800 dark:bg-red-900/20" role="alert">
      <svg class="mt-0.5 h-4 w-4 shrink-0 text-red-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
      <div>
        <div class="text-sm font-medium text-red-800 dark:text-red-200">Load failed</div>
        <div class="mt-1 text-sm text-red-700 dark:text-red-300">{error}</div>
      </div>
    </div>
  {/if}

  {#if loading}
    <div class="flex h-64 items-center justify-center" role="status" aria-label="Loading statistics">
      <svg class="h-10 w-10 animate-spin text-blue-500" viewBox="0 0 24 24" fill="none">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3"></circle>
        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
      </svg>
    </div>
  {:else}
    <!-- Summary cards -->
    <div class="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-5">
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="flex items-center gap-3">
          <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-blue-500/10">
            <svg class="h-5 w-5 text-blue-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4-4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 00-3-3.87"/><path d="M16 3.13a4 4 0 010 7.75"/></svg>
          </div>
          <div>
            <div class="text-sm text-gray-500 dark:text-gray-400">Total Users</div>
            <div class="mt-1 text-2xl font-bold text-gray-900 dark:text-white">{fmt(stats.users.total)}</div>
          </div>
        </div>
      </div>
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="flex items-center gap-3">
          <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-emerald-500/10">
            <svg class="h-5 w-5 text-emerald-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 11.08V12a10 10 0 11-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>
          </div>
          <div>
            <div class="text-sm text-gray-500 dark:text-gray-400">Active Accounts</div>
            <div class="mt-1 text-2xl font-bold text-gray-900 dark:text-white">{fmt(stats.accounts.active)}</div>
          </div>
        </div>
      </div>
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="flex items-center gap-3">
          <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-violet-500/10">
            <svg class="h-5 w-5 text-violet-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0110 0v4"/></svg>
          </div>
          <div>
            <div class="text-sm text-gray-500 dark:text-gray-400">Total API Keys</div>
            <div class="mt-1 text-2xl font-bold text-gray-900 dark:text-white">{fmt(stats.api_keys.total)}</div>
          </div>
        </div>
      </div>
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="flex items-center gap-3">
          <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-amber-500/10">
            <svg class="h-5 w-5 text-amber-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
          </div>
          <div>
            <div class="text-sm text-gray-500 dark:text-gray-400">Total Requests</div>
            <div class="mt-1 text-2xl font-bold text-gray-900 dark:text-white">{fmt(stats.usage.total_requests)}</div>
          </div>
        </div>
      </div>
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="flex items-center gap-3">
          <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-rose-500/10">
            <svg class="h-5 w-5 text-rose-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2v20M17 5H9.5a3.5 3.5 0 000 7h5a3.5 3.5 0 010 7H6"/></svg>
          </div>
          <div>
            <div class="text-sm text-gray-500 dark:text-gray-400">Total Cost</div>
            <div class="mt-1 text-2xl font-bold text-gray-900 dark:text-white">{yuan(stats.usage.total_cost)}</div>
          </div>
        </div>
      </div>
    </div>

    <!-- System Overview -->
    <h2 class="text-lg font-semibold text-gray-900 dark:text-white">System Overview</h2>

    <div class="grid grid-cols-1 gap-6 xl:grid-cols-2">
      <!-- Users -->
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="flex items-center gap-2">
          <svg class="h-5 w-5 text-blue-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4-4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 00-3-3.87"/><path d="M16 3.13a4 4 0 010 7.75"/></svg>
          <h3 class="text-base font-semibold text-gray-900 dark:text-white">Users</h3>
        </div>
        <div class="mt-4 grid grid-cols-2 gap-4 sm:grid-cols-3">
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">Total</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{fmt(stats.users.total)}</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">Active</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{fmt(stats.users.active)}</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">New Today</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{fmt(stats.users.new_today)}</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">New This Week</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{fmt(stats.users.new_this_week)}</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">New This Month</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{fmt(stats.users.new_this_month)}</div>
          </div>
        </div>
      </div>

      <!-- Accounts -->
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="flex items-center gap-2">
          <svg class="h-5 w-5 text-emerald-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 11-7.778 7.778 5.5 5.5 0 017.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4"/></svg>
          <h3 class="text-base font-semibold text-gray-900 dark:text-white">Accounts</h3>
        </div>
        <div class="mt-4 grid grid-cols-2 gap-4 sm:grid-cols-3">
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">Total</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{fmt(stats.accounts.total)}</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">Active</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{fmt(stats.accounts.active)}</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">Healthy</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{fmt(stats.accounts.healthy)}</div>
          </div>
        </div>
        {#if stats.accounts.by_platform.length > 0}
          <div class="mt-4 overflow-x-auto">
            <table class="min-w-full divide-y divide-gray-200 text-sm dark:divide-gray-700">
              <thead>
                <tr class="text-left text-gray-500 dark:text-gray-400">
                  <th scope="col" class="pb-2 pr-4 font-medium">Platform</th>
                  <th scope="col" class="pb-2 pr-4 font-medium">Count</th>
                  <th scope="col" class="pb-2 font-medium">Healthy</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-gray-100 dark:divide-gray-800">
                {#each stats.accounts.by_platform as p (p.platform)}
                  <tr class="transition-colors hover:bg-gray-50 dark:hover:bg-gray-700/30">
                    <td class="py-2 pr-4 font-medium text-gray-900 dark:text-white">{p.platform}</td>
                    <td class="py-2 pr-4 text-gray-600 dark:text-gray-300">{p.count}</td>
                    <td class="py-2 text-gray-600 dark:text-gray-300">{p.healthy_count}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      </div>

      <!-- API Keys -->
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="flex items-center gap-2">
          <svg class="h-5 w-5 text-violet-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0110 0v4"/></svg>
          <h3 class="text-base font-semibold text-gray-900 dark:text-white">API Keys</h3>
        </div>
        <div class="mt-4 grid grid-cols-2 gap-4 sm:grid-cols-3">
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">Total</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{fmt(stats.api_keys.total)}</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">Active</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{fmt(stats.api_keys.active)}</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">Expiring Soon</div>
            <div class="mt-2 text-2xl font-bold text-amber-600 dark:text-amber-400">{fmt(stats.api_keys.expiring_soon)}</div>
          </div>
        </div>
      </div>

      <!-- Usage -->
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="flex items-center gap-2">
          <svg class="h-5 w-5 text-amber-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
          <h3 class="text-base font-semibold text-gray-900 dark:text-white">Usage</h3>
        </div>
        <div class="mt-4 grid grid-cols-2 gap-4 sm:grid-cols-3">
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">Total Requests</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{fmt(stats.usage.total_requests)}</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">Total Tokens</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{fmt(stats.usage.total_tokens)}</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">Total Cost</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{yuan(stats.usage.total_cost)}</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">Today Requests</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{fmt(stats.usage.today_requests)}</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">Today Tokens</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{fmt(stats.usage.today_tokens)}</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">Today Cost</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{yuan(stats.usage.today_cost)}</div>
          </div>
        </div>
      </div>

      <!-- Ops -->
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800 xl:col-span-2">
        <div class="flex items-center gap-2">
          <svg class="h-5 w-5 text-cyan-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 3v18h18"/><path d="M7 14l4-4 3 3 5-7"/></svg>
          <h3 class="text-base font-semibold text-gray-900 dark:text-white">Ops & Batch Performance</h3>
        </div>
        <div class="mt-4 grid grid-cols-2 gap-4 lg:grid-cols-5">
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">Active Users (24h)</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{fmt(stats.ops.active_users_24h)}</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">Error Rate (1h)</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{percent(stats.ops.error_rate_1h)}</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">Avg Response (1h)</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{ms(stats.ops.avg_response_time_ms)}</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">Cache Hit Rate</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{percent(stats.ops.cache_hit_rate)}</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">Batch Errors</div>
            <div class="mt-2 text-2xl font-bold text-rose-600 dark:text-rose-400">{fmt(stats.ops.batch_errors_total)}</div>
          </div>
        </div>

        <div class="mt-4 overflow-x-auto">
          <table class="min-w-full divide-y divide-gray-200 text-sm dark:divide-gray-700">
            <thead>
              <tr class="text-left text-gray-500 dark:text-gray-400">
                <th scope="col" class="pb-2 pr-4 font-medium">Metric</th>
                <th scope="col" class="pb-2 pr-4 font-medium">Value</th>
                <th scope="col" class="pb-2 font-medium">Meaning</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-gray-100 dark:divide-gray-800">
              <tr class="transition-colors hover:bg-gray-50 dark:hover:bg-gray-700/30">
                <td class="py-2 pr-4 font-medium text-gray-900 dark:text-white">Batch operations total</td>
                <td class="py-2 pr-4 text-gray-600 dark:text-gray-300">{fmt(stats.ops.batch_operations_total)}</td>
                <td class="py-2 text-gray-600 dark:text-gray-300">运营侧累计批量动作次数</td>
              </tr>
              <tr class="transition-colors hover:bg-gray-50 dark:hover:bg-gray-700/30">
                <td class="py-2 pr-4 font-medium text-gray-900 dark:text-white">Latest fast import throughput</td>
                <td class="py-2 pr-4 text-gray-600 dark:text-gray-300">{stats.ops.latest_fast_import_throughput.toFixed(1)} items/s</td>
                <td class="py-2 text-gray-600 dark:text-gray-300">可信数据源快导入最近一次吞吐</td>
              </tr>
              <tr class="transition-colors hover:bg-gray-50 dark:hover:bg-gray-700/30">
                <td class="py-2 pr-4 font-medium text-gray-900 dark:text-white">Latest preview throughput</td>
                <td class="py-2 pr-4 text-gray-600 dark:text-gray-300">{stats.ops.latest_fast_import_preview_throughput.toFixed(1)} items/s</td>
                <td class="py-2 text-gray-600 dark:text-gray-300">预检链路最近一次吞吐</td>
              </tr>
              <tr class="transition-colors hover:bg-gray-50 dark:hover:bg-gray-700/30">
                <td class="py-2 pr-4 font-medium text-gray-900 dark:text-white">Latest fast import size</td>
                <td class="py-2 pr-4 text-gray-600 dark:text-gray-300">{fmt(stats.ops.latest_fast_import_size)}</td>
                <td class="py-2 text-gray-600 dark:text-gray-300">最近一次快导入批次规模</td>
              </tr>
              <tr class="transition-colors hover:bg-gray-50 dark:hover:bg-gray-700/30">
                <td class="py-2 pr-4 font-medium text-gray-900 dark:text-white">Latest preview size</td>
                <td class="py-2 pr-4 text-gray-600 dark:text-gray-300">{fmt(stats.ops.latest_fast_import_preview_size)}</td>
                <td class="py-2 text-gray-600 dark:text-gray-300">最近一次预检批次规模</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>

    <!-- Footer -->
    <div class="text-sm text-gray-500 dark:text-gray-400">Last updated: {formatDate(stats.updated_at)}</div>
  {/if}
</div>
