<svelte:head>
  <title>User Management - Admin</title>
</svelte:head>

<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type User, type PaginatedResponse } from '$lib/api';

  let users = $state<User[]>([]);
  let total = $state(0);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let search = $state('');
  let searchTimeout: ReturnType<typeof setTimeout> | null = null;

  onMount(() => {
    const token = localStorage.getItem('token');
    if (token) api.setToken(token);
    loadUsers();
  });

  async function loadUsers() {
    loading = true;
    error = null;
    try {
      const params: Record<string, unknown> = { per_page: 50 };
      if (search.trim()) params.search = search.trim();
      const res: PaginatedResponse<User> = await api.listUsers(params);
      users = res.data;
      total = res.total;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load users';
    } finally {
      loading = false;
    }
  }

  function onSearchInput(e: Event) {
    const target = e.target as HTMLInputElement;
    search = target.value;
    if (searchTimeout) clearTimeout(searchTimeout);
    searchTimeout = setTimeout(() => loadUsers(), 350);
  }

  function formatDate(value: string): string {
    if (!value) return '-';
    return new Date(value).toLocaleString('zh-CN');
  }

  function formatBalance(value: number): string {
    return `¥${value.toFixed(2)}`;
  }
</script>
<div class="space-y-6">
  <!-- Header -->
  <div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
    <div>
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">User Management</h1>
      <p class="mt-1 text-gray-500 dark:text-gray-400">Manage all registered users ({total} total)</p>
    </div>
    <button
      onclick={loadUsers}
      disabled={loading}
      class="inline-flex items-center justify-center gap-2 rounded-lg bg-blue-500 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-600 disabled:cursor-not-allowed disabled:opacity-50"
      aria-label="Refresh users"
    >
      <svg class="h-4 w-4" class:animate-spin={loading} fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" stroke-linecap="round" stroke-linejoin="round">
        <path d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"></path>
      </svg>
      Refresh
    </button>
  </div>

  <!-- Search -->
  <div class="relative">
    <svg class="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-gray-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="M21 21l-4.35-4.35"/></svg>
    <input
      type="text"
      placeholder="Search by email..."
      value={search}
      oninput={onSearchInput}
      class="w-full rounded-lg border border-gray-200 bg-white py-2.5 pl-10 pr-4 text-sm text-gray-900 placeholder-gray-400 transition-colors focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-700 dark:bg-gray-800 dark:text-white dark:placeholder-gray-500 dark:focus:border-blue-400 dark:focus:ring-blue-400 sm:max-w-sm"
    />
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
    <div class="flex h-64 items-center justify-center" role="status" aria-label="Loading users">
      <svg class="h-10 w-10 animate-spin text-blue-500" viewBox="0 0 24 24" fill="none">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3"></circle>
        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
      </svg>
    </div>
  {:else if users.length === 0}
    <!-- Empty -->
    <div class="flex flex-col items-center justify-center rounded-xl border border-gray-200 bg-white py-16 dark:border-gray-700 dark:bg-gray-800">
      <svg class="h-12 w-12 text-gray-300 dark:text-gray-600" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4-4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 00-3-3.87"/><path d="M16 3.13a4 4 0 010 7.75"/></svg>
      <p class="mt-4 text-sm text-gray-500 dark:text-gray-400">{search ? 'No users match your search' : 'No users found'}</p>
    </div>
  {:else}
    <!-- Desktop table -->
    <div class="hidden overflow-hidden rounded-xl border border-gray-200 bg-white shadow-sm dark:border-gray-700 dark:bg-gray-800 md:block">
      <table class="min-w-full divide-y divide-gray-200 text-sm dark:divide-gray-700">
        <thead>
          <tr class="bg-gray-50 text-left text-xs font-medium uppercase tracking-wider text-gray-500 dark:bg-gray-800/50 dark:text-gray-400">
            <th scope="col" class="px-6 py-3">Email</th>
            <th scope="col" class="px-6 py-3">Role</th>
            <th scope="col" class="px-6 py-3">Status</th>
            <th scope="col" class="px-6 py-3">Balance</th>
            <th scope="col" class="px-6 py-3">Created</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-gray-100 dark:divide-gray-700/50">
          {#each users as user (user.id)}
            <tr class="transition-colors hover:bg-gray-50 dark:hover:bg-gray-700/30">
              <td class="whitespace-nowrap px-6 py-4 font-medium text-gray-900 dark:text-white">{user.email}</td>
              <td class="whitespace-nowrap px-6 py-4">
                <span class={`inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium ${user.role === 'admin' ? 'bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-300' : 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300'}`}>
                  {user.role}
                </span>
              </td>
              <td class="whitespace-nowrap px-6 py-4">
                <span class={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium ${user.status === 'active' ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-300' : user.status === 'suspended' ? 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-300' : 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-300'}`}>
                  <span class={`h-1.5 w-1.5 rounded-full ${user.status === 'active' ? 'bg-green-500' : user.status === 'suspended' ? 'bg-red-500' : 'bg-yellow-500'}`}></span>
                  {user.status}
                </span>
              </td>
              <td class="whitespace-nowrap px-6 py-4 text-gray-600 dark:text-gray-300">{formatBalance(user.balance)}</td>
              <td class="whitespace-nowrap px-6 py-4 text-gray-500 dark:text-gray-400">{formatDate(user.created_at)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>

    <!-- Mobile cards -->
    <div class="space-y-3 md:hidden">
      {#each users as user (user.id)}
        <div class="rounded-xl border border-gray-200 bg-white p-4 shadow-sm dark:border-gray-700 dark:bg-gray-800">
          <div class="flex items-start justify-between gap-2">
            <div class="min-w-0 flex-1">
              <div class="truncate font-medium text-gray-900 dark:text-white">{user.email}</div>
              <div class="mt-1 text-xs text-gray-500 dark:text-gray-400">{formatDate(user.created_at)}</div>
            </div>
            <span class={`shrink-0 inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium ${user.role === 'admin' ? 'bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-300' : 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300'}`}>
              {user.role}
            </span>
          </div>
          <div class="mt-3 flex items-center justify-between">
            <span class={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium ${user.status === 'active' ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-300' : user.status === 'suspended' ? 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-300' : 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-300'}`}>
              <span class={`h-1.5 w-1.5 rounded-full ${user.status === 'active' ? 'bg-green-500' : user.status === 'suspended' ? 'bg-red-500' : 'bg-yellow-500'}`}></span>
              {user.status}
            </span>
            <span class="text-sm font-medium text-gray-700 dark:text-gray-300">{formatBalance(user.balance)}</span>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
