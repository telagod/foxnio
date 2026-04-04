<script lang="ts">
  import { onMount } from 'svelte';
  import {
    api,
    type AdminDashboardStats,
    type ChartData,
    type ChartDataset,
    type DistributionData
  } from '$lib/api';

  const emptyStats: AdminDashboardStats = {
    users: {
      total: 0,
      active: 0,
      new_today: 0,
      new_this_week: 0,
      new_this_month: 0
    },
    accounts: {
      total: 0,
      active: 0,
      healthy: 0,
      by_platform: []
    },
    api_keys: {
      total: 0,
      active: 0,
      expiring_soon: 0
    },
    usage: {
      total_requests: 0,
      total_tokens: 0,
      total_cost: 0,
      today_requests: 0,
      today_tokens: 0,
      today_cost: 0
    },
    updated_at: ''
  };

  const emptyChart: ChartData = { labels: [], datasets: [] };
  const emptyDistribution: DistributionData = { labels: [], data: [], total: 0 };

  let stats = $state<AdminDashboardStats>(emptyStats);
  let trend = $state<ChartData>(emptyChart);
  let latency = $state<ChartData>(emptyChart);
  let outcome = $state<ChartData>(emptyChart);
  let modelDistribution = $state<DistributionData>(emptyDistribution);
  let platformDistribution = $state<DistributionData>(emptyDistribution);
  let loading = $state(true);
  let error = $state<string | null>(null);

  onMount(() => {
    const token = localStorage.getItem('token');
    if (token) api.setToken(token);

    loadDashboard();
    const interval = setInterval(loadDashboard, 30000);
    return () => clearInterval(interval);
  });

  async function loadDashboard() {
    try {
      const [
        nextStats,
        nextTrend,
        nextLatency,
        nextOutcome,
        nextModelDistribution,
        nextPlatformDistribution
      ] = await Promise.all([
        api.getAdminDashboardStats(),
        api.getAdminDashboardTrend(),
        api.getAdminDashboardLine(),
        api.getAdminDashboardPie(),
        api.getAdminDashboardModelDistribution(),
        api.getAdminDashboardPlatformDistribution()
      ]);

      stats = nextStats;
      trend = nextTrend;
      latency = nextLatency;
      outcome = nextOutcome;
      modelDistribution = nextModelDistribution;
      platformDistribution = nextPlatformDistribution;
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : '加载 dashboard 失败';
      console.error('Failed to load admin dashboard:', e);
    } finally {
      loading = false;
    }
  }

  function formatNumber(num: number): string {
    if (num >= 1_000_000) return `${(num / 1_000_000).toFixed(1)}M`;
    if (num >= 1_000) return `${(num / 1_000).toFixed(1)}K`;
    return `${num}`;
  }

  function formatCurrencyYuan(value: number): string {
    return `¥${value.toFixed(2)}`;
  }

  function formatDate(value: string): string {
    if (!value) return '-';
    return new Date(value).toLocaleString('zh-CN');
  }

  function latest(dataset?: ChartDataset): number {
    if (!dataset || dataset.data.length === 0) return 0;
    return dataset.data[dataset.data.length - 1];
  }

  function getSparklinePoints(data: number[], width = 100, height = 42): string {
    if (data.length === 0) return '';
    if (data.length === 1) return `0,${height / 2} ${width},${height / 2}`;

    const max = Math.max(...data, 1);
    return data
      .map((value, index) => {
        const x = (index / (data.length - 1)) * width;
        const y = height - (value / max) * height;
        return `${x},${y}`;
      })
      .join(' ');
  }

  function getDistributionRows(distribution: DistributionData) {
    return distribution.labels.map((label, index) => {
      const value = distribution.data[index] || 0;
      const ratio = distribution.total > 0 ? (value / distribution.total) * 100 : 0;
      return { label, value, ratio };
    });
  }

  function getPieRows() {
    const dataset = outcome.datasets[0];
    const colors = Array.isArray(dataset?.backgroundColor) ? dataset.backgroundColor : [];
    return outcome.labels.map((label, index) => {
      const value = dataset?.data[index] || 0;
      const total = dataset?.data.reduce((sum, item) => sum + item, 0) || 0;
      const ratio = total > 0 ? (value / total) * 100 : 0;
      return {
        label,
        value,
        ratio,
        color: colors[index] || '#94a3b8'
      };
    });
  }
</script>

<div class="space-y-6">
  <div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
    <div>
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">管理控制面</h1>
      <p class="mt-1 text-gray-500 dark:text-gray-400">系统总览、趋势与分布数据</p>
    </div>

    <button
      onclick={loadDashboard}
      disabled={loading}
      class="flex items-center justify-center gap-2 rounded-lg bg-blue-500 px-4 py-2 text-white transition-colors hover:bg-blue-600 disabled:cursor-not-allowed disabled:opacity-50"
    >
      <svg class="h-4 w-4" class:animate-spin={loading} fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"></path>
      </svg>
      刷新
    </button>
  </div>

  {#if error}
    <div class="rounded-lg border border-red-200 bg-red-50 p-4 dark:border-red-800 dark:bg-red-900/20">
      <div class="text-sm font-medium text-red-800 dark:text-red-200">加载失败</div>
      <div class="mt-1 text-sm text-red-700 dark:text-red-300">{error}</div>
    </div>
  {/if}

  {#if loading}
    <div class="flex h-64 items-center justify-center">
      <div class="h-12 w-12 animate-spin rounded-full border-b-2 border-blue-500"></div>
    </div>
  {:else}
    <div class="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="text-sm text-gray-500 dark:text-gray-400">总用户</div>
        <div class="mt-2 text-3xl font-bold text-gray-900 dark:text-white">{formatNumber(stats.users.total)}</div>
        <div class="mt-2 text-xs text-gray-500 dark:text-gray-400">今日新增 {formatNumber(stats.users.new_today)}</div>
      </div>
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="text-sm text-gray-500 dark:text-gray-400">活跃账号</div>
        <div class="mt-2 text-3xl font-bold text-gray-900 dark:text-white">{formatNumber(stats.accounts.active)}</div>
        <div class="mt-2 text-xs text-gray-500 dark:text-gray-400">健康账号 {formatNumber(stats.accounts.healthy)}</div>
      </div>
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="text-sm text-gray-500 dark:text-gray-400">今日请求</div>
        <div class="mt-2 text-3xl font-bold text-gray-900 dark:text-white">{formatNumber(stats.usage.today_requests)}</div>
        <div class="mt-2 text-xs text-gray-500 dark:text-gray-400">今日 Token {formatNumber(stats.usage.today_tokens)}</div>
      </div>
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="text-sm text-gray-500 dark:text-gray-400">累计费用</div>
        <div class="mt-2 text-3xl font-bold text-gray-900 dark:text-white">{formatCurrencyYuan(stats.usage.total_cost)}</div>
        <div class="mt-2 text-xs text-gray-500 dark:text-gray-400">更新时间 {formatDate(stats.updated_at)}</div>
      </div>
    </div>

    <div class="grid grid-cols-1 gap-4 lg:grid-cols-3">
      {#each trend.datasets as dataset}
        <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
          <div class="flex items-start justify-between gap-3">
            <div>
              <div class="text-sm text-gray-500 dark:text-gray-400">{dataset.label}</div>
              <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">
                {formatNumber(Math.round(latest(dataset)))}
              </div>
            </div>
            <div class="text-xs text-gray-500 dark:text-gray-400">{trend.labels.length} 天</div>
          </div>
          <div class="mt-4">
            <svg viewBox="0 0 100 42" class="h-16 w-full overflow-visible">
              <polyline
                fill="none"
                stroke={dataset.color || '#3b82f6'}
                stroke-width="2.5"
                points={getSparklinePoints(dataset.data)}
              />
            </svg>
          </div>
          <div class="mt-2 flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
            <span>{trend.labels[0] || '-'}</span>
            <span>{trend.labels[trend.labels.length - 1] || '-'}</span>
          </div>
        </div>
      {/each}
    </div>

    <div class="grid grid-cols-1 gap-6 xl:grid-cols-2">
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="flex items-start justify-between gap-3">
          <div>
            <h2 class="text-lg font-semibold text-gray-900 dark:text-white">响应时间趋势</h2>
            <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">来自 usage metadata 的日均响应时间</p>
          </div>
          <div class="text-sm font-medium text-emerald-600 dark:text-emerald-400">
            {Math.round(latest(latency.datasets[0]))} ms
          </div>
        </div>
        <div class="mt-6">
          <svg viewBox="0 0 100 42" class="h-20 w-full overflow-visible">
            <polyline
              fill="none"
              stroke={latency.datasets[0]?.borderColor || '#10b981'}
              stroke-width="2.5"
              points={getSparklinePoints(latency.datasets[0]?.data || [])}
            />
          </svg>
        </div>
        <div class="mt-2 flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
          <span>{latency.labels[0] || '-'}</span>
          <span>{latency.labels[latency.labels.length - 1] || '-'}</span>
        </div>
      </div>

      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">请求结果分布</h2>
        <div class="mt-4 space-y-4">
          {#each getPieRows() as row}
            <div class="space-y-2">
              <div class="flex items-center justify-between text-sm">
                <span class="font-medium text-gray-700 dark:text-gray-300">{row.label}</span>
                <span class="text-gray-500 dark:text-gray-400">{row.value} / {row.ratio.toFixed(1)}%</span>
              </div>
              <div class="h-2 overflow-hidden rounded-full bg-gray-100 dark:bg-gray-700">
                <div class="h-full rounded-full" style={`width:${row.ratio}%;background:${row.color};`}></div>
              </div>
            </div>
          {/each}
        </div>
      </div>
    </div>

    <div class="grid grid-cols-1 gap-6 xl:grid-cols-2">
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">模型分布</h2>
        <div class="mt-4 space-y-4">
          {#each getDistributionRows(modelDistribution) as row}
            <div class="space-y-2">
              <div class="flex items-center justify-between text-sm">
                <span class="font-medium text-gray-700 dark:text-gray-300">{row.label}</span>
                <span class="text-gray-500 dark:text-gray-400">{row.value}</span>
              </div>
              <div class="h-2 overflow-hidden rounded-full bg-gray-100 dark:bg-gray-700">
                <div class="h-full rounded-full bg-blue-500" style={`width:${row.ratio}%;`}></div>
              </div>
            </div>
          {/each}
        </div>
      </div>

      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">平台分布</h2>
        <div class="mt-4 space-y-4">
          {#each getDistributionRows(platformDistribution) as row}
            <div class="space-y-2">
              <div class="flex items-center justify-between text-sm">
                <span class="font-medium text-gray-700 dark:text-gray-300">{row.label}</span>
                <span class="text-gray-500 dark:text-gray-400">{row.value}</span>
              </div>
              <div class="h-2 overflow-hidden rounded-full bg-gray-100 dark:bg-gray-700">
                <div class="h-full rounded-full bg-emerald-500" style={`width:${row.ratio}%;`}></div>
              </div>
            </div>
          {/each}
        </div>
      </div>
    </div>

    <div class="grid grid-cols-1 gap-6 xl:grid-cols-2">
      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">平台健康度</h2>
        <div class="mt-4 overflow-x-auto">
          <table class="min-w-full divide-y divide-gray-200 text-sm dark:divide-gray-700">
            <thead>
              <tr class="text-left text-gray-500 dark:text-gray-400">
                <th class="pb-3 pr-4">平台</th>
                <th class="pb-3 pr-4">账号数</th>
                <th class="pb-3 pr-4">健康账号</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-gray-100 dark:divide-gray-800">
              {#each stats.accounts.by_platform as platform}
                <tr>
                  <td class="py-3 pr-4 font-medium text-gray-900 dark:text-white">{platform.platform}</td>
                  <td class="py-3 pr-4 text-gray-600 dark:text-gray-300">{platform.count}</td>
                  <td class="py-3 pr-4 text-gray-600 dark:text-gray-300">{platform.healthy_count}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>

      <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">关键计数</h2>
        <div class="mt-4 grid grid-cols-2 gap-4">
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">API Key 总数</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{stats.api_keys.total}</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">活跃 API Key</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{stats.api_keys.active}</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">7 天内到期</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{stats.api_keys.expiring_soon}</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-700/40">
            <div class="text-xs text-gray-500 dark:text-gray-400">本月新增用户</div>
            <div class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{stats.users.new_this_month}</div>
          </div>
        </div>
      </div>
    </div>

    <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
      <h2 class="mb-4 text-lg font-semibold text-gray-900 dark:text-white">快速入口</h2>
      <div class="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <a href="/admin/users" class="rounded-lg bg-gray-50 p-4 text-center transition-colors hover:bg-gray-100 dark:bg-gray-700/50 dark:hover:bg-gray-700">
          <div class="text-2xl">👥</div>
          <div class="mt-2 text-sm font-medium text-gray-700 dark:text-gray-300">用户</div>
        </a>
        <a href="/admin/accounts" class="rounded-lg bg-gray-50 p-4 text-center transition-colors hover:bg-gray-100 dark:bg-gray-700/50 dark:hover:bg-gray-700">
          <div class="text-2xl">🔑</div>
          <div class="mt-2 text-sm font-medium text-gray-700 dark:text-gray-300">账号</div>
        </a>
        <a href="/apikeys" class="rounded-lg bg-gray-50 p-4 text-center transition-colors hover:bg-gray-100 dark:bg-gray-700/50 dark:hover:bg-gray-700">
          <div class="text-2xl">🗝️</div>
          <div class="mt-2 text-sm font-medium text-gray-700 dark:text-gray-300">API Keys</div>
        </a>
        <a href="/usage" class="rounded-lg bg-gray-50 p-4 text-center transition-colors hover:bg-gray-100 dark:bg-gray-700/50 dark:hover:bg-gray-700">
          <div class="text-2xl">📊</div>
          <div class="mt-2 text-sm font-medium text-gray-700 dark:text-gray-300">Usage</div>
        </a>
      </div>
    </div>
  {/if}
</div>
