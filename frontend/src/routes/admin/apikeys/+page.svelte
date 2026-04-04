<svelte:head>
  <title>All API Keys - Admin</title>
</svelte:head>

<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type ApiKey } from '$lib/api';

  let keys = $state<ApiKey[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let search = $state('');
  let comingSoon = $state(false);

  onMount(() => {
    const token = localStorage.getItem('token');
    if (token) api.setToken(token);
    loadKeys();
  });

  async function loadKeys() {
    loading = true; error = null;
    try {
      if (typeof (api as any).getAdminApiKeys === 'function') {
        const res = await (api as any).getAdminApiKeys();
        keys = res.data ?? [];
      } else { comingSoon = true; }
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load API keys';
    } finally { loading = false; }
  }

  let filtered = $derived(
    keys.filter(k => {
      if (!search) return true;
      const s = search.toLowerCase();
      return (k.key?.toLowerCase().includes(s))
        || (k.name?.toLowerCase().includes(s))
        || (k.user_id?.toLowerCase().includes(s))
        || (k.status?.toLowerCase().includes(s));
    })
  );

  function truncateKey(key: string): string {
    if (!key) return '-';
    if (key.length <= 12) return key;
    return key.slice(0, 8) + '...' + key.slice(-4);
  }
  function formatDate(v: string | null): string {
    if (!v) return '-';
    return new Date(v).toLocaleString('zh-CN');
  }
  function statusColor(s: string) {
    if (s === 'active') return 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400';
    if (s === 'revoked') return 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400';
    return 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400';
  }
</script>

<div class="space-y-6">
  <div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
    <div>
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">All API Keys</h1>
      <p class="mt-1 text-gray-500 dark:text-gray-400">Manage all users' API keys</p>
    </div>
    <button onclick={loadKeys} disabled={loading}
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
    <div class="flex h-64 items-center justify-center" role="status" aria-label="Loading API keys">
      <svg class="h-10 w-10 animate-spin text-blue-500" viewBox="0 0 24 24" fill="none">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3"></circle>
        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
      </svg>
    </div>
  {:else if comingSoon}
    <div class="flex flex-col items-center justify-center rounded-xl border border-gray-200 bg-white py-16 shadow-sm dark:border-gray-700 dark:bg-gray-800">
      <div class="flex h-16 w-16 items-center justify-center rounded-full bg-amber-500/10">
        <svg class="h-8 w-8 text-amber-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
      </div>
      <h2 class="mt-4 text-lg font-semibold text-gray-900 dark:text-white">Coming Soon</h2>
      <p class="mt-2 max-w-sm text-center text-sm text-gray-500 dark:text-gray-400">The admin API keys endpoint (<code class="rounded bg-gray-100 px-1.5 py-0.5 text-xs dark:bg-gray-700">getAdminApiKeys</code>) is not yet available. This page will display all users' API keys once the backend endpoint is implemented.</p>
    </div>
  {:else}
    <!-- Search -->
    <div class="relative">
      <svg class="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-gray-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
      <input type="text" bind:value={search} placeholder="Search by key, name, user or status..."
        class="w-full rounded-lg border border-gray-200 bg-white py-2.5 pl-10 pr-4 text-sm text-gray-900 placeholder-gray-400 transition-colors focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-700 dark:bg-gray-800 dark:text-white dark:placeholder-gray-500 dark:focus:border-blue-400 dark:focus:ring-blue-400" />
    </div>

    {#if filtered.length === 0}
      <div class="flex flex-col items-center justify-center rounded-xl border border-gray-200 bg-white py-16 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div class="flex h-16 w-16 items-center justify-center rounded-full bg-gray-100 dark:bg-gray-700">
          <svg class="h-8 w-8 text-gray-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0110 0v4"/></svg>
        </div>
        <h2 class="mt-4 text-lg font-semibold text-gray-900 dark:text-white">No API keys found</h2>
        <p class="mt-2 text-sm text-gray-500 dark:text-gray-400">{search ? 'Try adjusting your search.' : 'No API keys have been created yet.'}</p>
      </div>
    {:else}
      <!-- Desktop table -->
      <div class="hidden overflow-hidden rounded-xl border border-gray-200 bg-white shadow-sm dark:border-gray-700 dark:bg-gray-800 md:block">
        <div class="overflow-x-auto">
          <table class="min-w-full divide-y divide-gray-200 text-sm dark:divide-gray-700">
            <thead>
              <tr class="text-left text-gray-500 dark:text-gray-400">
                <th scope="col" class="px-4 py-3 font-medium">Key</th>
                <th scope="col" class="px-4 py-3 font-medium">Name</th>
                <th scope="col" class="px-4 py-3 font-medium">User</th>
                <th scope="col" class="px-4 py-3 font-medium">Status</th>
                <th scope="col" class="px-4 py-3 font-medium">Created</th>
                <th scope="col" class="px-4 py-3 font-medium">Last Used</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-gray-100 dark:divide-gray-800">
              {#each filtered as key (key.id)}
                <tr class="transition-colors hover:bg-gray-50 dark:hover:bg-gray-700/30">
                  <td class="px-4 py-3 font-mono text-xs text-gray-900 dark:text-white">{truncateKey(key.key)}</td>
                  <td class="px-4 py-3 text-gray-700 dark:text-gray-300">{key.name || '-'}</td>
                  <td class="px-4 py-3 font-mono text-xs text-gray-500 dark:text-gray-400">{key.user_id?.slice(0, 8) || '-'}</td>
                  <td class="px-4 py-3"><span class="inline-flex rounded-full px-2 py-0.5 text-xs font-medium {statusColor(key.status)}">{key.status}</span></td>
                  <td class="px-4 py-3 text-gray-500 dark:text-gray-400">{formatDate(key.created_at)}</td>
                  <td class="px-4 py-3 text-gray-500 dark:text-gray-400">{formatDate(key.last_used_at)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>

      <!-- Mobile cards -->
      <div class="space-y-3 md:hidden">
        {#each filtered as key (key.id)}
          <div class="rounded-xl border border-gray-200 bg-white p-4 shadow-sm dark:border-gray-700 dark:bg-gray-800">
            <div class="flex items-center justify-between">
              <span class="font-mono text-xs text-gray-900 dark:text-white">{truncateKey(key.key)}</span>
              <span class="inline-flex rounded-full px-2 py-0.5 text-xs font-medium {statusColor(key.status)}">{key.status}</span>
            </div>
            <div class="mt-3 space-y-1.5 text-sm">
              <div class="flex justify-between"><span class="text-gray-500 dark:text-gray-400">Name</span><span class="text-gray-900 dark:text-white">{key.name || '-'}</span></div>
              <div class="flex justify-between"><span class="text-gray-500 dark:text-gray-400">User</span><span class="font-mono text-xs text-gray-700 dark:text-gray-300">{key.user_id?.slice(0, 8) || '-'}</span></div>
              <div class="flex justify-between"><span class="text-gray-500 dark:text-gray-400">Created</span><span class="text-gray-700 dark:text-gray-300">{formatDate(key.created_at)}</span></div>
              <div class="flex justify-between"><span class="text-gray-500 dark:text-gray-400">Last Used</span><span class="text-gray-700 dark:text-gray-300">{formatDate(key.last_used_at)}</span></div>
            </div>
          </div>
        {/each}
      </div>

      <div class="text-sm text-gray-500 dark:text-gray-400">Showing {filtered.length} of {keys.length} keys</div>
    {/if}
  {/if}
</div>
