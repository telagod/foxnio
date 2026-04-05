<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type UserUsageReport } from '$lib/api';

  const emptyReport: UserUsageReport = {
    days: 30, total_requests: 0, total_input_tokens: 0, total_output_tokens: 0,
    total_tokens: 0, total_cost: 0, total_cost_yuan: 0, daily_usage: []
  };

  const periodMap: Record<string, number> = { '7d': 7, '30d': 30, '90d': 90 };

  let report = $state<UserUsageReport>(emptyReport);
  let loading = $state(true);
  let selectedPeriod = $state<'7d' | '30d' | '90d'>('7d');
  let error = $state('');

  onMount(async () => {
    const token = localStorage.getItem('token');
    if (token) api.setToken(token);
    await loadUsage();
  });

  async function loadUsage() {
    loading = true;
    try { report = await api.getUserUsage(periodMap[selectedPeriod]); error = ''; }
    catch (e) { error = e instanceof Error ? e.message : '加载 usage 失败'; console.error('Failed to load usage:', e); }
    finally { loading = false; }
  }

  function formatTokens(num: number): string {
    if (num >= 1_000_000) return `${(num / 1_000_000).toFixed(1)}M`;
    if (num >= 1_000) return `${(num / 1_000).toFixed(1)}K`;
    return `${num}`;
  }
  function formatCostYuan(yuan: number): string { return `¥${yuan.toFixed(2)}`; }
  function formatDate(value: string): string { return new Date(value).toLocaleDateString('zh-CN'); }
  function maxRequests(): number { return Math.max(...report.daily_usage.map((item) => item.requests), 1); }
</script>

<div class="space-y-6 p-6">
  <!-- Header -->
  <div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
    <div>
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Usage Statistics</h1>
      <p class="mt-1 text-gray-500 dark:text-gray-400">真实用量汇总与日维度趋势</p>
    </div>
    <div>
      <label for="period-select" class="sr-only">Select time period</label>
      <select
        id="period-select"
        bind:value={selectedPeriod}
        onchange={loadUsage}
        class="rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm text-gray-900 shadow-sm transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-white"
      >
        <option value="7d">Last 7 days</option>
        <option value="30d">Last 30 days</option>
        <option value="90d">Last 90 days</option>
      </select>
    </div>
  </div>

  {#if error}
    <div class="flex items-start gap-3 rounded-lg border border-red-200 bg-red-50 p-4 text-sm text-red-700 dark:border-red-800 dark:bg-red-900/20 dark:text-red-300" role="alert">
      <svg class="mt-0.5 h-4 w-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
      <span>{error}</span>
    </div>
  {/if}

  {#if loading}
    <div class="flex h-32 items-center justify-center" role="status" aria-label="Loading usage data">
      <svg class="h-10 w-10 animate-spin text-blue-500" viewBox="0 0 24 24" fill="none">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3"></circle>
        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
      </svg>
    </div>
  {:else}
    <!-- Summary cards -->
    <div class="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="flex items-center gap-3">
          <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-blue-500/10">
            <svg class="h-5 w-5 text-blue-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
          </div>
          <div>
            <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Total Requests</h3>
            <p class="mt-1 text-2xl font-bold text-gray-900 dark:text-white">{formatTokens(report.total_requests)}</p>
          </div>
        </div>
      </div>
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="flex items-center gap-3">
          <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-indigo-500/10">
            <svg class="h-5 w-5 text-indigo-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 12h16M4 6h16M4 18h10"/></svg>
          </div>
          <div>
            <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Input Tokens</h3>
            <p class="mt-1 text-2xl font-bold text-gray-900 dark:text-white">{formatTokens(report.total_input_tokens)}</p>
          </div>
        </div>
      </div>
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="flex items-center gap-3">
          <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-emerald-500/10">
            <svg class="h-5 w-5 text-emerald-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 12h16M4 6h16M4 18h10"/></svg>
          </div>
          <div>
            <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Output Tokens</h3>
            <p class="mt-1 text-2xl font-bold text-gray-900 dark:text-white">{formatTokens(report.total_output_tokens)}</p>
          </div>
        </div>
      </div>
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="flex items-center gap-3">
          <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-amber-500/10">
            <svg class="h-5 w-5 text-amber-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2v20M17 5H9.5a3.5 3.5 0 000 7h5a3.5 3.5 0 010 7H6"/></svg>
          </div>
          <div>
            <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Total Cost</h3>
            <p class="mt-1 text-2xl font-bold text-gray-900 dark:text-white">{formatCostYuan(report.total_cost_yuan)}</p>
          </div>
        </div>
      </div>
    </div>

    {#if report.total_requests > 0}
      <!-- Daily bar chart -->
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Daily Requests</h2>
        <div class="mt-4 space-y-3">
          {#each report.daily_usage as item (item.date)}
            <div class="space-y-1">
              <div class="flex items-center justify-between text-sm">
                <span class="text-gray-700 dark:text-gray-300">{formatDate(item.date)}</span>
                <span class="text-gray-500 dark:text-gray-400">{item.requests} requests · {formatCostYuan(item.cost_yuan)}</span>
              </div>
              <div class="h-2 overflow-hidden rounded-full bg-gray-100 dark:bg-gray-700">
                <div
                  class="h-full rounded-full bg-blue-500 transition-all duration-300"
                  style={`width:${(item.requests / maxRequests()) * 100}%;`}
                  role="progressbar"
                  aria-valuenow={item.requests}
                  aria-valuemin={0}
                  aria-valuemax={maxRequests()}
                  aria-label="{item.requests} requests on {formatDate(item.date)}"
                ></div>
              </div>
            </div>
          {/each}
        </div>
      </div>

      <!-- Detail table -->
      <div class="overflow-hidden rounded-xl border border-gray-200 bg-white shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="overflow-x-auto">
          <table class="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
            <thead class="bg-gray-50 dark:bg-gray-700/40">
              <tr>
                <th scope="col" class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500 dark:text-gray-400">Date</th>
                <th scope="col" class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500 dark:text-gray-400">Requests</th>
                <th scope="col" class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500 dark:text-gray-400">Input Tokens</th>
                <th scope="col" class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500 dark:text-gray-400">Output Tokens</th>
                <th scope="col" class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500 dark:text-gray-400">Total Tokens</th>
                <th scope="col" class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500 dark:text-gray-400">Cost</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-gray-200 bg-white dark:divide-gray-700 dark:bg-gray-800">
              {#each report.daily_usage as item (item.date)}
                <tr class="transition-colors hover:bg-gray-50 dark:hover:bg-gray-700/30">
                  <td class="whitespace-nowrap px-6 py-4 text-sm font-medium text-gray-900 dark:text-white">{formatDate(item.date)}</td>
                  <td class="whitespace-nowrap px-6 py-4 text-sm text-gray-600 dark:text-gray-300">{item.requests}</td>
                  <td class="whitespace-nowrap px-6 py-4 text-sm text-gray-600 dark:text-gray-300">{formatTokens(item.input_tokens)}</td>
                  <td class="whitespace-nowrap px-6 py-4 text-sm text-gray-600 dark:text-gray-300">{formatTokens(item.output_tokens)}</td>
                  <td class="whitespace-nowrap px-6 py-4 text-sm text-gray-600 dark:text-gray-300">{formatTokens(item.total_tokens)}</td>
                  <td class="whitespace-nowrap px-6 py-4 text-sm text-gray-600 dark:text-gray-300">{formatCostYuan(item.cost_yuan)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>
    {:else}
      <!-- Empty state -->
      <div class="rounded-xl border border-gray-200 bg-white p-12 text-center shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="mx-auto flex h-16 w-16 items-center justify-center rounded-full bg-gray-100 dark:bg-gray-700">
          <svg class="h-8 w-8 text-gray-400 dark:text-gray-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="20" x2="18" y2="10"/><line x1="12" y1="20" x2="12" y2="4"/><line x1="6" y1="20" x2="6" y2="14"/></svg>
        </div>
        <h3 class="mt-4 text-lg font-semibold text-gray-900 dark:text-white">No Usage Data Yet</h3>
        <p class="mt-2 text-sm text-gray-500 dark:text-gray-400">Start making API requests to see your usage statistics.</p>
      </div>
    {/if}
  {/if}
</div>
