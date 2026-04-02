<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';

  interface Usage {
    date: string;
    requests: number;
    input_tokens: number;
    output_tokens: number;
    cost: number;
  }

  let usage: Usage[] = [];
  let loading = true;
  let selectedPeriod = '7d';

  onMount(async () => {
    // 从 localStorage 恢复 token
    const token = localStorage.getItem('token');
    if (token) api.setToken(token);
    
    await loadUsage();
  });

  async function loadUsage() {
    try {
      const data = await api.getUserUsage();
      // 适配数据格式
      usage = (data as any).daily_usage || [];
    } catch (e) {
      console.error('Failed to load usage:', e);
    } finally {
      loading = false;
    }
  }

  function formatTokens(num: number): string {
    if (num >= 1000000) return (num / 1000000).toFixed(1) + 'M';
    if (num >= 1000) return (num / 1000).toFixed(1) + 'K';
    return num.toString();
  }

  function formatCost(cents: number): string {
    return '$' + (cents / 100).toFixed(2);
  }

  function getTotalRequests(): number {
    return usage.reduce((sum, u) => sum + u.requests, 0);
  }

  function getTotalTokens(): number {
    return usage.reduce((sum, u) => sum + u.input_tokens + u.output_tokens, 0);
  }

  function getTotalCost(): number {
    return usage.reduce((sum, u) => sum + u.cost, 0);
  }
</script>

<div class="p-6">
  <div class="flex items-center justify-between mb-6">
    <h1 class="text-2xl font-bold">📊 Usage Statistics</h1>
    
    <select
      bind:value={selectedPeriod}
      class="px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
    >
      <option value="7d">Last 7 days</option>
      <option value="30d">Last 30 days</option>
      <option value="90d">Last 90 days</option>
    </select>
  </div>

  {#if loading}
    <div class="flex items-center justify-center h-32">
      <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-500"></div>
    </div>
  {:else}
    <!-- Summary Cards -->
    <div class="grid grid-cols-1 md:grid-cols-3 gap-6 mb-6">
      <div class="bg-white shadow rounded-lg p-6">
        <h3 class="text-sm font-medium text-gray-500">Total Requests</h3>
        <p class="text-3xl font-bold mt-2">{formatTokens(getTotalRequests())}</p>
      </div>

      <div class="bg-white shadow rounded-lg p-6">
        <h3 class="text-sm font-medium text-gray-500">Total Tokens</h3>
        <p class="text-3xl font-bold mt-2">{formatTokens(getTotalTokens())}</p>
      </div>

      <div class="bg-white shadow rounded-lg p-6">
        <h3 class="text-sm font-medium text-gray-500">Total Cost</h3>
        <p class="text-3xl font-bold mt-2">{formatCost(getTotalCost())}</p>
      </div>
    </div>

    <!-- Usage Table -->
    {#if usage.length > 0}
      <div class="bg-white shadow rounded-lg overflow-hidden">
        <table class="min-w-full divide-y divide-gray-200">
          <thead class="bg-gray-50">
            <tr>
              <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Date</th>
              <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Requests</th>
              <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Input Tokens</th>
              <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Output Tokens</th>
              <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Cost</th>
            </tr>
          </thead>
          <tbody class="bg-white divide-y divide-gray-200">
            {#each usage as u}
              <tr>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
                  {new Date(u.date).toLocaleDateString()}
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                  {u.requests}
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                  {formatTokens(u.input_tokens)}
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                  {formatTokens(u.output_tokens)}
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                  {formatCost(u.cost)}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {:else}
      <div class="bg-white shadow rounded-lg p-12 text-center">
        <div class="text-6xl mb-4">📈</div>
        <h3 class="text-lg font-semibold mb-2">No Usage Data Yet</h3>
        <p class="text-gray-500">Start making API requests to see your usage statistics</p>
      </div>
    {/if}
  {/if}
</div>
