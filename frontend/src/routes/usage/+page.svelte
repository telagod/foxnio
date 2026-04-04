<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type UserUsageReport } from '$lib/api';

  const emptyReport: UserUsageReport = {
    days: 30,
    total_requests: 0,
    total_input_tokens: 0,
    total_output_tokens: 0,
    total_tokens: 0,
    total_cost: 0,
    total_cost_yuan: 0,
    daily_usage: []
  };

  const periodMap: Record<string, number> = {
    '7d': 7,
    '30d': 30,
    '90d': 90
  };

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
    try {
      report = await api.getUserUsage(periodMap[selectedPeriod]);
      error = '';
    } catch (e) {
      error = e instanceof Error ? e.message : '加载 usage 失败';
      console.error('Failed to load usage:', e);
    } finally {
      loading = false;
    }
  }

  function formatTokens(num: number): string {
    if (num >= 1_000_000) return `${(num / 1_000_000).toFixed(1)}M`;
    if (num >= 1_000) return `${(num / 1_000).toFixed(1)}K`;
    return `${num}`;
  }

  function formatCostYuan(yuan: number): string {
    return `¥${yuan.toFixed(2)}`;
  }

  function formatDate(value: string): string {
    return new Date(value).toLocaleDateString('zh-CN');
  }

  function maxRequests(): number {
    return Math.max(...report.daily_usage.map((item) => item.requests), 1);
  }
</script>

<div class="space-y-6 p-6">
  <div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
    <div>
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Usage Statistics</h1>
      <p class="mt-1 text-gray-500 dark:text-gray-400">真实用量汇总与日维度趋势</p>
    </div>

    <select
      bind:value={selectedPeriod}
      onchange={loadUsage}
      class="rounded-md border border-gray-300 px-3 py-2 focus:outline-none focus:ring-2 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-white"
    >
      <option value="7d">Last 7 days</option>
      <option value="30d">Last 30 days</option>
      <option value="90d">Last 90 days</option>
    </select>
  </div>

  {#if error}
    <div class="rounded-lg border border-red-200 bg-red-50 p-4 text-sm text-red-700 dark:border-red-800 dark:bg-red-900/20 dark:text-red-300">
      {error}
    </div>
  {/if}

  {#if loading}
    <div class="flex h-32 items-center justify-center">
      <div class="h-12 w-12 animate-spin rounded-full border-b-2 border-blue-500"></div>
    </div>
  {:else}
    <div class="grid grid-cols-1 gap-6 md:grid-cols-4">
      <div class="rounded-lg bg-white p-6 shadow dark:bg-gray-800">
        <h3 class="text-sm font-medium text-gray-500">Total Requests</h3>
        <p class="mt-2 text-3xl font-bold text-gray-900 dark:text-white">{formatTokens(report.total_requests)}</p>
      </div>

      <div class="rounded-lg bg-white p-6 shadow dark:bg-gray-800">
        <h3 class="text-sm font-medium text-gray-500">Input Tokens</h3>
        <p class="mt-2 text-3xl font-bold text-gray-900 dark:text-white">{formatTokens(report.total_input_tokens)}</p>
      </div>

      <div class="rounded-lg bg-white p-6 shadow dark:bg-gray-800">
        <h3 class="text-sm font-medium text-gray-500">Output Tokens</h3>
        <p class="mt-2 text-3xl font-bold text-gray-900 dark:text-white">{formatTokens(report.total_output_tokens)}</p>
      </div>

      <div class="rounded-lg bg-white p-6 shadow dark:bg-gray-800">
        <h3 class="text-sm font-medium text-gray-500">Total Cost</h3>
        <p class="mt-2 text-3xl font-bold text-gray-900 dark:text-white">{formatCostYuan(report.total_cost_yuan)}</p>
      </div>
    </div>

    {#if report.total_requests > 0}
      <div class="rounded-lg bg-white p-6 shadow dark:bg-gray-800">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Daily Requests</h2>
        <div class="mt-4 space-y-3">
          {#each report.daily_usage as item}
            <div class="space-y-1">
              <div class="flex items-center justify-between text-sm">
                <span class="text-gray-700 dark:text-gray-300">{formatDate(item.date)}</span>
                <span class="text-gray-500 dark:text-gray-400">{item.requests} requests · {formatCostYuan(item.cost_yuan)}</span>
              </div>
              <div class="h-2 overflow-hidden rounded-full bg-gray-100 dark:bg-gray-700">
                <div
                  class="h-full rounded-full bg-blue-500"
                  style={`width:${(item.requests / maxRequests()) * 100}%;`}
                ></div>
              </div>
            </div>
          {/each}
        </div>
      </div>

      <div class="overflow-hidden rounded-lg bg-white shadow dark:bg-gray-800">
        <table class="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
          <thead class="bg-gray-50 dark:bg-gray-700/40">
            <tr>
              <th class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">Date</th>
              <th class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">Requests</th>
              <th class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">Input Tokens</th>
              <th class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">Output Tokens</th>
              <th class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">Total Tokens</th>
              <th class="px-6 py-3 text-left text-xs font-medium uppercase tracking-wider text-gray-500">Cost</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-gray-200 bg-white dark:divide-gray-700 dark:bg-gray-800">
            {#each report.daily_usage as item}
              <tr>
                <td class="whitespace-nowrap px-6 py-4 text-sm text-gray-900 dark:text-white">{formatDate(item.date)}</td>
                <td class="whitespace-nowrap px-6 py-4 text-sm text-gray-500 dark:text-gray-300">{item.requests}</td>
                <td class="whitespace-nowrap px-6 py-4 text-sm text-gray-500 dark:text-gray-300">{formatTokens(item.input_tokens)}</td>
                <td class="whitespace-nowrap px-6 py-4 text-sm text-gray-500 dark:text-gray-300">{formatTokens(item.output_tokens)}</td>
                <td class="whitespace-nowrap px-6 py-4 text-sm text-gray-500 dark:text-gray-300">{formatTokens(item.total_tokens)}</td>
                <td class="whitespace-nowrap px-6 py-4 text-sm text-gray-500 dark:text-gray-300">{formatCostYuan(item.cost_yuan)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {:else}
      <div class="rounded-lg bg-white p-12 text-center shadow dark:bg-gray-800">
        <div class="mb-4 text-6xl">📈</div>
        <h3 class="mb-2 text-lg font-semibold text-gray-900 dark:text-white">No Usage Data Yet</h3>
        <p class="text-gray-500 dark:text-gray-400">Start making API requests to see your usage statistics.</p>
      </div>
    {/if}
  {/if}
</div>
