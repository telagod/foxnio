<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type HealthStatus } from '$lib/api';

  let health: HealthStatus = $state({
    status: 'unknown',
    checks: {},
    timestamp: ''
  });

  let loading = $state(true);
  let error: string | null = $state(null);

  onMount(() => {
    checkHealth();
    const interval = setInterval(checkHealth, 30000);
    return () => clearInterval(interval);
  });

  async function checkHealth() {
    try {
      health = await api.getHealth();
      loading = false;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Unknown error';
      loading = false;
    }
  }

  function getStatusColor(status: string): string {
    if (status === 'healthy') return 'text-emerald-600 dark:text-emerald-400';
    if (status === 'unhealthy') return 'text-red-600 dark:text-red-400';
    return 'text-amber-600 dark:text-amber-400';
  }

  function getStatusBg(status: string): string {
    if (status === 'healthy') return 'bg-emerald-50 dark:bg-emerald-900/20 border-emerald-200 dark:border-emerald-800';
    if (status === 'unhealthy') return 'bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800';
    return 'bg-amber-50 dark:bg-amber-900/20 border-amber-200 dark:border-amber-800';
  }

  function getStatusDot(status: string): string {
    if (status === 'healthy') return 'bg-emerald-500';
    if (status === 'unhealthy') return 'bg-red-500';
    return 'bg-amber-500';
  }
</script>

<svelte:head>
  <title>System Health - FoxNIO</title>
</svelte:head>

<div class="p-6 max-w-4xl mx-auto space-y-6">
  <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
    <div class="flex items-center gap-3">
      <div class="p-2 bg-rose-100 dark:bg-rose-900/30 rounded-lg">
        <svg class="w-6 h-6 text-rose-600 dark:text-rose-400" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M21 8.25c0-2.485-2.099-4.5-4.688-4.5-1.935 0-3.597 1.126-4.312 2.733-.715-1.607-2.377-2.733-4.313-2.733C5.1 3.75 3 5.765 3 8.25c0 7.22 9 12 9 12s9-4.78 9-12z" />
        </svg>
      </div>
      <div>
        <h1 class="text-2xl font-bold text-gray-900 dark:text-white">System Health</h1>
        <p class="text-sm text-gray-500 dark:text-gray-400 mt-0.5">Real-time infrastructure status</p>
      </div>
    </div>

    <button
      onclick={checkHealth}
      class="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium
             bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600
             text-gray-700 dark:text-gray-300 rounded-lg
             hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors
             focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 dark:focus:ring-offset-gray-900"
      aria-label="Refresh health status"
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
        <path stroke-linecap="round" stroke-linejoin="round" d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0 3.181 3.183a8.25 8.25 0 0 0 13.803-3.7M4.031 9.865a8.25 8.25 0 0 1 13.803-3.7l3.181 3.182" />
      </svg>
      Refresh
    </button>
  </div>

  {#if loading}
    <div class="flex items-center justify-center h-64">
      <div class="animate-spin rounded-full h-10 w-10 border-2 border-gray-200 dark:border-gray-700 border-t-blue-500"></div>
    </div>
  {:else if error}
    <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4 flex items-start gap-3">
      <svg class="w-5 h-5 text-red-500 dark:text-red-400 mt-0.5 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
        <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m9-.75a9 9 0 1 1-18 0 9 9 0 0 1 18 0zm-9 3.75h.008v.008H12v-.008z" />
      </svg>
      <div>
        <p class="text-sm font-medium text-red-800 dark:text-red-200">Connection Error</p>
        <p class="text-sm text-red-700 dark:text-red-300 mt-1">{error}</p>
      </div>
    </div>
  {:else}
    <!-- Overall Status Banner -->
    <div class="rounded-xl border {getStatusBg(health.status)} p-5">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-3">
          <span class="relative flex h-3 w-3">
            <span class="animate-ping absolute inline-flex h-full w-full rounded-full {getStatusDot(health.status)} opacity-75"></span>
            <span class="relative inline-flex rounded-full h-3 w-3 {getStatusDot(health.status)}"></span>
          </span>
          <div>
            <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Overall Status</h2>
            <p class="text-sm {getStatusColor(health.status)} font-medium uppercase tracking-wide mt-0.5">
              {health.status}
            </p>
          </div>
        </div>
        <div class="text-xs text-gray-500 dark:text-gray-400 text-right">
          <span>Last checked</span><br />
          <span class="font-medium">{health.timestamp ? new Date(health.timestamp).toLocaleString() : 'N/A'}</span>
        </div>
      </div>
    </div>

    <!-- Component Checks -->
    {#if health.checks && Object.keys(health.checks).length > 0}
      <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 overflow-hidden">
        <div class="px-5 py-4 border-b border-gray-200 dark:border-gray-700">
          <h3 class="text-sm font-semibold text-gray-900 dark:text-white uppercase tracking-wide">Component Checks</h3>
        </div>
        <div class="divide-y divide-gray-100 dark:divide-gray-700">
          {#each Object.entries(health.checks) as [name, check]}
            <div class="flex items-center justify-between px-5 py-3.5 hover:bg-gray-50 dark:hover:bg-gray-700/50 transition-colors">
              <div class="flex items-center gap-3">
                <span class="h-2 w-2 rounded-full {getStatusDot(check.status)}"></span>
                <span class="text-sm font-medium text-gray-900 dark:text-white capitalize">{name}</span>
              </div>
              <span class="text-xs font-medium px-2.5 py-1 rounded-full {getStatusColor(check.status)}
                          {check.status === 'healthy' ? 'bg-emerald-50 dark:bg-emerald-900/20' : check.status === 'unhealthy' ? 'bg-red-50 dark:bg-red-900/20' : 'bg-amber-50 dark:bg-amber-900/20'}">
                {check.status}
              </span>
            </div>
          {/each}
        </div>
      </div>
    {:else}
      <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-10 text-center">
        <svg class="w-10 h-10 text-gray-300 dark:text-gray-600 mx-auto mb-3" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M9 12.75 11.25 15 15 9.75M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0z" />
        </svg>
        <p class="text-sm text-gray-500 dark:text-gray-400">No component checks available</p>
      </div>
    {/if}
  {/if}
</div>
