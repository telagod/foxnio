<script lang="ts">
  import { onMount } from 'svelte';

  let health = {
    status: 'unknown',
    checks: {},
    timestamp: ''
  };

  let loading = true;
  let error = null;

  onMount(async () => {
    await checkHealth();
    // 每 30 秒检查一次
    setInterval(checkHealth, 30000);
  });

  async function checkHealth() {
    try {
      const response = await fetch('/health');
      health = await response.json();
      loading = false;
    } catch (e) {
      error = e.message;
      loading = false;
    }
  }

  function getStatusColor(status: string): string {
    if (status === 'healthy') return 'text-green-500';
    if (status === 'unhealthy') return 'text-red-500';
    return 'text-yellow-500';
  }
</script>

<div class="p-6">
  <h1 class="text-2xl font-bold mb-6">🏥 System Health</h1>

  {#if loading}
    <div class="flex items-center justify-center h-32">
      <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-500"></div>
    </div>
  {:else if error}
    <div class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded">
      Error: {error}
    </div>
  {:else}
    <div class="bg-white shadow rounded-lg p-6">
      <div class="flex items-center justify-between mb-4">
        <h2 class="text-xl font-semibold">Overall Status</h2>
        <span class={getStatusColor(health.status)} class="text-lg font-bold uppercase">
          {health.status}
        </span>
      </div>

      <div class="border-t pt-4">
        <h3 class="font-semibold mb-3">Component Checks</h3>
        <div class="space-y-3">
          {#each Object.entries(health.checks) as [name, check]}
            <div class="flex items-center justify-between p-3 bg-gray-50 rounded">
              <span class="font-medium capitalize">{name}</span>
              <span class={getStatusColor(check.status)} class="text-sm">
                {check.status}
              </span>
            </div>
          {/each}
        </div>
      </div>

      <div class="mt-4 text-sm text-gray-500">
        Last checked: {new Date(health.timestamp).toLocaleString()}
      </div>
    </div>
  {/if}

  <div class="mt-6">
    <button
      on:click={checkHealth}
      class="bg-blue-500 hover:bg-blue-600 text-white px-4 py-2 rounded"
    >
      Refresh
    </button>
  </div>
</div>
