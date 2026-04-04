<svelte:head>
  <title>Provider Accounts - Admin</title>
</svelte:head>

<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type Account, type PaginatedResponse } from '$lib/api';

  let accounts = $state<Account[]>([]);
  let total = $state(0);
  let loading = $state(true);
  let error = $state<string | null>(null);

  onMount(() => {
    const token = localStorage.getItem('token');
    if (token) api.setToken(token);
    loadAccounts();
  });

  async function loadAccounts() {
    loading = true;
    error = null;
    try {
      const res: PaginatedResponse<Account> = await api.listAccounts({ per_page: 50 });
      accounts = res.data;
      total = res.total;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load accounts';
    } finally {
      loading = false;
    }
  }

  function formatDate(value: string): string {
    if (!value) return '-';
    return new Date(value).toLocaleString('zh-CN');
  }

  function platformColor(provider: string): string {
    switch (provider.toLowerCase()) {
      case 'openai': return 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-300';
      case 'anthropic': return 'bg-orange-100 text-orange-700 dark:bg-orange-900/30 dark:text-orange-300';
      case 'gemini': case 'google': return 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300';
      case 'deepseek': return 'bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-300';
      default: return 'bg-gray-100 text-gray-700 dark:bg-gray-900/30 dark:text-gray-300';
    }
  }

  function statusColor(status: string): string {
    switch (status) {
      case 'active': return 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-300';
      case 'disabled': return 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-300';
      case 'error': return 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-300';
      default: return 'bg-gray-100 text-gray-700 dark:bg-gray-900/30 dark:text-gray-300';
    }
  }

  function statusDot(status: string): string {
    switch (status) {
      case 'active': return 'bg-green-500';
      case 'disabled': return 'bg-red-500';
      case 'error': return 'bg-yellow-500';
      default: return 'bg-gray-500';
    }
  }
</script>

<div class="space-y-6">
  <!-- Header -->
  <div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
    <div>
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Provider Accounts</h1>
      <p class="mt-1 text-gray-500 dark:text-gray-400">Manage AI provider accounts ({total} total)</p>
    </div>
    <button
      onclick={loadAccounts}
      disabled={loading}
      class="inline-flex items-center justify-center gap-2 rounded-lg bg-blue-500 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-600 disabled:cursor-not-allowed disabled:opacity-50"
      aria-label="Refresh accounts"
    >
      <svg class="h-4 w-4" class:animate-spin={loading} fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" stroke-linecap="round" stroke-linejoin="round">
        <path d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"></path>
      </svg>
      Refresh
    </button>
  </div>

  <!-- Error -->
  {#if error}
    <div class="flex items-start gap-3 rounded-lg border border-red-200 bg-red-50 p-4 dark:border-red-800 dark:bg-red-900/20" role="alert">
      <svg class="mt-0.5 h-4 w-4 shrink-0 text-red-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
      <div>
        <div class="text-sm font-medium text-red-800 dark:text-red-200">Failed to load</div>
        <div class="mt-1 text-sm text-red-700 dark:text-red-300">{error}</div>
      </div>
    </div>
  {/if}

  <!-- Loading -->
  {#if loading}
    <div class="flex h-64 items-center justify-center" role="status" aria-label="Loading accounts">
      <svg class="h-10 w-10 animate-spin text-blue-500" viewBox="0 0 24 24" fill="none">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3"></circle>
        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
      </svg>
    </div>
  {:else if accounts.length === 0}
    <!-- Empty -->
    <div class="flex flex-col items-center justify-center rounded-xl border border-gray-200 bg-white py-16 dark:border-gray-700 dark:bg-gray-800">
      <svg class="h-12 w-12 text-gray-300 dark:text-gray-600" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 11-7.778 7.778 5.5 5.5 0 017.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4"/></svg>
      <p class="mt-4 text-sm text-gray-500 dark:text-gray-400">No provider accounts found</p>
    </div>
  {:else}
    <!-- Desktop table -->
    <div class="hidden overflow-hidden rounded-xl border border-gray-200 bg-white shadow-sm dark:border-gray-700 dark:bg-gray-800 md:block">
      <table class="min-w-full divide-y divide-gray-200 text-sm dark:divide-gray-700">
        <thead>
          <tr class="bg-gray-50 text-left text-xs font-medium uppercase tracking-wider text-gray-500 dark:bg-gray-800/50 dark:text-gray-400">
            <th scope="col" class="px-6 py-3">Name</th>
            <th scope="col" class="px-6 py-3">Platform</th>
            <th scope="col" class="px-6 py-3">Status</th>
            <th scope="col" class="px-6 py-3">Priority</th>
            <th scope="col" class="px-6 py-3">Created</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-gray-100 dark:divide-gray-700/50">
          {#each accounts as account (account.id)}
            <tr class="transition-colors hover:bg-gray-50 dark:hover:bg-gray-700/30">
              <td class="whitespace-nowrap px-6 py-4">
                <div class="font-medium text-gray-900 dark:text-white">{account.name}</div>
                <div class="text-xs text-gray-500 dark:text-gray-400">{account.credential_type}</div>
              </td>
              <td class="whitespace-nowrap px-6 py-4">
                <span class={`inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium ${platformColor(account.provider)}`}>
                  {account.provider}
                </span>
              </td>
              <td class="whitespace-nowrap px-6 py-4">
                <span class={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium ${statusColor(account.status)}`}>
                  <span class={`h-1.5 w-1.5 rounded-full ${statusDot(account.status)}`}></span>
                  {account.status}
                </span>
              </td>
              <td class="whitespace-nowrap px-6 py-4 text-gray-600 dark:text-gray-300">{account.priority ?? '-'}</td>
              <td class="whitespace-nowrap px-6 py-4 text-gray-500 dark:text-gray-400">{formatDate(account.created_at)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>

    <!-- Mobile cards -->
    <div class="space-y-3 md:hidden">
      {#each accounts as account (account.id)}
        <div class="rounded-xl border border-gray-200 bg-white p-4 shadow-sm dark:border-gray-700 dark:bg-gray-800">
          <div class="flex items-start justify-between gap-2">
            <div class="min-w-0 flex-1">
              <div class="truncate font-medium text-gray-900 dark:text-white">{account.name}</div>
              <div class="mt-1 text-xs text-gray-500 dark:text-gray-400">{account.credential_type}</div>
            </div>
            <span class={`shrink-0 inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium ${platformColor(account.provider)}`}>
              {account.provider}
            </span>
          </div>
          <div class="mt-3 flex items-center justify-between">
            <span class={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium ${statusColor(account.status)}`}>
              <span class={`h-1.5 w-1.5 rounded-full ${statusDot(account.status)}`}></span>
              {account.status}
            </span>
            <span class="text-xs text-gray-500 dark:text-gray-400">Priority: {account.priority ?? '-'}</span>
          </div>
          <div class="mt-2 text-xs text-gray-400 dark:text-gray-500">{formatDate(account.created_at)}</div>
        </div>
      {/each}
    </div>
  {/if}
</div>
