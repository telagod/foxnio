<svelte:head>
  <title>渠道管理 - Admin</title>
</svelte:head>

<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type Account, type PaginatedResponse } from '$lib/api';

  const PROVIDERS = ['openai', 'anthropic', 'gemini', 'deepseek', 'mistral', 'cohere'] as const;
  const CREDENTIAL_TYPES = ['api_key', 'oauth_token'] as const;

  let accounts = $state<Account[]>([]);
  let total = $state(0);
  let page = $state(1);
  let totalPages = $state(1);
  let loading = $state(true);
  let error = $state<string | null>(null);

  // Filters
  let searchQuery = $state('');
  let filterProvider = $state('');
  let filterStatus = $state('');
  let searchTimeout: ReturnType<typeof setTimeout> | null = null;

  // Toast
  let toast = $state<{ message: string; type: 'success' | 'error' } | null>(null);
  let toastTimeout: ReturnType<typeof setTimeout> | null = null;

  // Modals
  let showCreateModal = $state(false);
  let showEditModal = $state(false);
  let showDeleteConfirm = $state(false);
  let showImportModal = $state(false);

  // Form state
  let formName = $state('');
  let formProvider = $state<string>('openai');
  let formCredentialType = $state<string>('api_key');
  let formCredential = $state('');
  let formPriority = $state(0);
  let formSubmitting = $state(false);

  // Edit target
  let editTarget = $state<Account | null>(null);

  // Delete target
  let deleteTarget = $state<Account | null>(null);
  let deleteSubmitting = $state(false);

  // Import
  let importJson = $state('');
  let importSubmitting = $state(false);
  let importResult = $state<{ succeeded: number; failed: number; errors: string[] } | null>(null);

  onMount(() => {
    const token = localStorage.getItem('token');
    if (token) api.setToken(token);
    loadAccounts();
  });

  function showToast(message: string, type: 'success' | 'error') {
    if (toastTimeout) clearTimeout(toastTimeout);
    toast = { message, type };
    toastTimeout = setTimeout(() => { toast = null; }, 3500);
  }

  async function loadAccounts() {
    loading = true;
    error = null;
    try {
      const params: Record<string, unknown> = { page, per_page: 20 };
      if (searchQuery.trim()) params.search = searchQuery.trim();
      if (filterProvider) params.provider = filterProvider;
      if (filterStatus) params.status = filterStatus;
      const res: PaginatedResponse<Account> = await api.listAccounts(params);
      accounts = res.data;
      total = res.total;
      totalPages = res.total_pages;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load accounts';
    } finally {
      loading = false;
    }
  }

  function onSearchInput(value: string) {
    searchQuery = value;
    if (searchTimeout) clearTimeout(searchTimeout);
    searchTimeout = setTimeout(() => { page = 1; loadAccounts(); }, 350);
  }

  function onFilterChange() {
    page = 1;
    loadAccounts();
  }

  function goPage(p: number) {
    if (p < 1 || p > totalPages) return;
    page = p;
    loadAccounts();
  }

  function formatDate(value: string): string {
    if (!value) return '-';
    return new Date(value).toLocaleString('zh-CN');
  }

  function platformColor(provider: string): string {
    switch (provider.toLowerCase()) {
      case 'openai': return 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300';
      case 'anthropic': return 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-300';
      case 'gemini': return 'bg-sky-100 text-sky-700 dark:bg-sky-900/30 dark:text-sky-300';
      case 'deepseek': return 'bg-violet-100 text-violet-700 dark:bg-violet-900/30 dark:text-violet-300';
      case 'mistral': return 'bg-rose-100 text-rose-700 dark:bg-rose-900/30 dark:text-rose-300';
      case 'cohere': return 'bg-teal-100 text-teal-700 dark:bg-teal-900/30 dark:text-teal-300';
      default: return 'bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300';
    }
  }

  function statusColor(status: string): string {
    switch (status) {
      case 'active': return 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-300';
      case 'disabled': return 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-300';
      case 'error': return 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-300';
      default: return 'bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300';
    }
  }

  function statusDot(status: string): string {
    switch (status) {
      case 'active': return 'bg-green-500';
      case 'disabled': return 'bg-red-500';
      case 'error': return 'bg-yellow-500';
      default: return 'bg-gray-400';
    }
  }

  // Create modal
  function openCreateModal() {
    formName = '';
    formProvider = 'openai';
    formCredentialType = 'api_key';
    formCredential = '';
    formPriority = 0;
    formSubmitting = false;
    showCreateModal = true;
  }

  async function submitCreate() {
    if (!formName.trim() || !formCredential.trim()) return;
    formSubmitting = true;
    try {
      await api.createAccount({
        name: formName.trim(),
        provider: formProvider,
        credential_type: formCredentialType,
        credential: formCredential.trim(),
        priority: formPriority,
      });
      showCreateModal = false;
      showToast('渠道创建成功', 'success');
      loadAccounts();
    } catch (e) {
      showToast(e instanceof Error ? e.message : 'Create failed', 'error');
    } finally {
      formSubmitting = false;
    }
  }

  // Edit modal
  function openEditModal(account: Account) {
    editTarget = account;
    formName = account.name;
    formProvider = account.provider;
    formCredentialType = account.credential_type;
    formCredential = '';
    formPriority = account.priority ?? 0;
    formSubmitting = false;
    showEditModal = true;
  }

  async function submitEdit() {
    if (!editTarget || !formName.trim()) return;
    formSubmitting = true;
    try {
      const updates: Record<string, unknown> = {
        name: formName.trim(),
        provider: formProvider,
        credential_type: formCredentialType,
        priority: formPriority,
      };
      if (formCredential.trim()) updates.credential = formCredential.trim();
      await api.updateAccount(editTarget.id, updates);
      showEditModal = false;
      editTarget = null;
      showToast('渠道更新成功', 'success');
      loadAccounts();
    } catch (e) {
      showToast(e instanceof Error ? e.message : 'Update failed', 'error');
    } finally {
      formSubmitting = false;
    }
  }

  // Delete
  function openDeleteConfirm(account: Account) {
    deleteTarget = account;
    deleteSubmitting = false;
    showDeleteConfirm = true;
  }

  async function submitDelete() {
    if (!deleteTarget) return;
    deleteSubmitting = true;
    try {
      await api.deleteAccount(deleteTarget.id);
      showDeleteConfirm = false;
      deleteTarget = null;
      showToast('渠道已删除', 'success');
      loadAccounts();
    } catch (e) {
      showToast(e instanceof Error ? e.message : 'Delete failed', 'error');
    } finally {
      deleteSubmitting = false;
    }
  }

  // Import
  function openImportModal() {
    importJson = '';
    importSubmitting = false;
    importResult = null;
    showImportModal = true;
  }

  async function submitImport() {
    importSubmitting = true;
    importResult = null;
    try {
      const parsed = JSON.parse(importJson);
      if (!Array.isArray(parsed) || parsed.length === 0) {
        showToast('JSON must be a non-empty array', 'error');
        importSubmitting = false;
        return;
      }
      const res = await api.batchCreateAccounts(parsed);
      importResult = { succeeded: res.succeeded, failed: res.failed, errors: res.errors };
      if (res.succeeded > 0) {
        showToast(`成功导入 ${res.succeeded} 个渠道`, 'success');
        loadAccounts();
      }
      if (res.failed > 0) {
        showToast(`${res.failed} 个渠道导入失败`, 'error');
      }
    } catch (e) {
      if (e instanceof SyntaxError) {
        showToast('JSON 格式错误', 'error');
      } else {
        showToast(e instanceof Error ? e.message : 'Import failed', 'error');
      }
    } finally {
      importSubmitting = false;
    }
  }

  function handleModalKeydown(e: KeyboardEvent, closeFn: () => void) {
    if (e.key === 'Escape') closeFn();
  }

  function closeCreateModal() { showCreateModal = false; }
  function closeEditModal() { showEditModal = false; editTarget = null; }
  function closeDeleteConfirm() { showDeleteConfirm = false; deleteTarget = null; }
  function closeImportModal() { showImportModal = false; importResult = null; }
</script>

<!-- Toast -->
{#if toast}
  <div
    class="fixed right-4 top-4 z-50 flex items-center gap-2 rounded-lg px-4 py-3 text-sm font-medium shadow-lg transition-all {toast.type === 'success' ? 'bg-green-600 text-white' : 'bg-red-600 text-white'}"
    role="status"
    aria-live="polite"
  >
    {#if toast.type === 'success'}
      <svg class="h-4 w-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6L9 17l-5-5"/></svg>
    {:else}
      <svg class="h-4 w-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>
    {/if}
    {toast.message}
  </div>
{/if}

<div class="space-y-6">
  <!-- Header -->
  <div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
    <div>
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">渠道管理</h1>
      <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">Provider Accounts ({total} total)</p>
    </div>
    <div class="flex gap-2">
      <button
        onclick={openImportModal}
        class="inline-flex items-center gap-1.5 rounded-lg border border-gray-300 bg-white px-3.5 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-200 dark:hover:bg-gray-700"
      >
        <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
        批量导入
      </button>
      <button
        onclick={openCreateModal}
        class="inline-flex items-center gap-1.5 rounded-lg bg-blue-600 px-3.5 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-700"
      >
        <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        添加渠道
      </button>
    </div>
  </div>

  <!-- Search / Filter bar -->
  <div class="flex flex-col gap-3 sm:flex-row sm:items-center">
    <div class="relative flex-1">
      <svg class="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-gray-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
      <input
        type="text"
        id="search-accounts"
        placeholder="搜索渠道名称..."
        value={searchQuery}
        oninput={(e) => onSearchInput(e.currentTarget.value)}
        class="w-full rounded-lg border border-gray-300 bg-white py-2 pl-10 pr-3 text-sm text-gray-900 placeholder-gray-400 transition-colors focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-white dark:placeholder-gray-500 dark:focus:border-blue-400"
      />
    </div>
    <div class="flex gap-2">
      <select
        id="filter-provider"
        value={filterProvider}
        onchange={(e) => { filterProvider = e.currentTarget.value; onFilterChange(); }}
        class="rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-700 transition-colors focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-200"
        aria-label="Filter by provider"
      >
        <option value="">All Platforms</option>
        {#each PROVIDERS as p}
          <option value={p}>{p.charAt(0).toUpperCase() + p.slice(1)}</option>
        {/each}
      </select>
      <select
        id="filter-status"
        value={filterStatus}
        onchange={(e) => { filterStatus = e.currentTarget.value; onFilterChange(); }}
        class="rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-700 transition-colors focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-200"
        aria-label="Filter by status"
      >
        <option value="">All Status</option>
        <option value="active">Active</option>
        <option value="disabled">Disabled</option>
        <option value="error">Error</option>
      </select>
    </div>
  </div>

  <!-- Error -->
  {#if error}
    <div class="flex items-start gap-3 rounded-lg border border-red-200 bg-red-50 p-4 dark:border-red-800 dark:bg-red-900/20" role="alert">
      <svg class="mt-0.5 h-5 w-5 shrink-0 text-red-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
      <div class="flex-1">
        <p class="text-sm font-medium text-red-800 dark:text-red-200">加载失败</p>
        <p class="mt-1 text-sm text-red-700 dark:text-red-300">{error}</p>
      </div>
      <button onclick={loadAccounts} class="shrink-0 rounded-md bg-red-100 px-3 py-1 text-xs font-medium text-red-700 transition-colors hover:bg-red-200 dark:bg-red-900/40 dark:text-red-300 dark:hover:bg-red-900/60">重试</button>
    </div>
  {/if}

  <!-- Loading -->
  {#if loading}
    <div class="flex h-64 items-center justify-center" role="status" aria-label="Loading accounts">
      <svg class="h-8 w-8 animate-spin text-blue-500" viewBox="0 0 24 24" fill="none">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3"></circle>
        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
      </svg>
    </div>
  {:else if accounts.length === 0}
    <!-- Empty state -->
    <div class="flex flex-col items-center justify-center rounded-xl border border-dashed border-gray-300 bg-white py-16 dark:border-gray-600 dark:bg-gray-800/50">
      <svg class="h-12 w-12 text-gray-300 dark:text-gray-600" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>
      <p class="mt-4 text-sm text-gray-500 dark:text-gray-400">暂无渠道账号</p>
      <button onclick={openCreateModal} class="mt-4 inline-flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-700">
        <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        添加第一个渠道
      </button>
    </div>
  {:else}

    <!-- Desktop table -->
    <div class="hidden overflow-hidden rounded-xl border border-gray-200 bg-white shadow-sm dark:border-gray-700 dark:bg-gray-800 md:block">
      <table class="min-w-full divide-y divide-gray-200 text-sm dark:divide-gray-700">
        <thead>
          <tr class="bg-gray-50 text-left text-xs font-medium uppercase tracking-wider text-gray-500 dark:bg-gray-800/60 dark:text-gray-400">
            <th scope="col" class="px-5 py-3">Name</th>
            <th scope="col" class="px-5 py-3">Platform</th>
            <th scope="col" class="px-5 py-3">Credential</th>
            <th scope="col" class="px-5 py-3">Status</th>
            <th scope="col" class="px-5 py-3">Priority</th>
            <th scope="col" class="px-5 py-3">Last Error</th>
            <th scope="col" class="px-5 py-3">Created</th>
            <th scope="col" class="px-5 py-3 text-right">Actions</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-gray-100 dark:divide-gray-700/50">
          {#each accounts as account (account.id)}
            <tr class="transition-colors hover:bg-gray-50 dark:hover:bg-gray-700/20">
              <td class="whitespace-nowrap px-5 py-3.5">
                <span class="font-medium text-gray-900 dark:text-white">{account.name}</span>
              </td>
              <td class="whitespace-nowrap px-5 py-3.5">
                <span class="inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium {platformColor(account.provider)}">{account.provider}</span>
              </td>
              <td class="whitespace-nowrap px-5 py-3.5 text-gray-500 dark:text-gray-400">{account.credential_type}</td>
              <td class="whitespace-nowrap px-5 py-3.5">
                <span class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium {statusColor(account.status)}">
                  <span class="h-1.5 w-1.5 rounded-full {statusDot(account.status)}"></span>
                  {account.status}
                </span>
              </td>
              <td class="whitespace-nowrap px-5 py-3.5 text-gray-600 dark:text-gray-300">{account.priority ?? 0}</td>
              <td class="max-w-[200px] truncate px-5 py-3.5 text-xs text-gray-500 dark:text-gray-400" title={account.last_error ?? ''}>{account.last_error ?? '-'}</td>
              <td class="whitespace-nowrap px-5 py-3.5 text-gray-500 dark:text-gray-400">{formatDate(account.created_at)}</td>
              <td class="whitespace-nowrap px-5 py-3.5 text-right">
                <div class="flex items-center justify-end gap-1">
                  <button onclick={() => openEditModal(account)} class="rounded-md p-1.5 text-gray-400 transition-colors hover:bg-gray-100 hover:text-blue-600 dark:hover:bg-gray-700 dark:hover:text-blue-400" aria-label="Edit {account.name}">
                    <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
                  </button>
                  <button onclick={() => openDeleteConfirm(account)} class="rounded-md p-1.5 text-gray-400 transition-colors hover:bg-red-50 hover:text-red-600 dark:hover:bg-red-900/20 dark:hover:text-red-400" aria-label="Delete {account.name}">
                    <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/><line x1="10" y1="11" x2="10" y2="17"/><line x1="14" y1="11" x2="14" y2="17"/></svg>
                  </button>
                </div>
              </td>
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
            <span class="shrink-0 inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium {platformColor(account.provider)}">{account.provider}</span>
          </div>
          <div class="mt-3 flex items-center justify-between">
            <span class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium {statusColor(account.status)}">
              <span class="h-1.5 w-1.5 rounded-full {statusDot(account.status)}"></span>
              {account.status}
            </span>
            <span class="text-xs text-gray-500 dark:text-gray-400">Priority: {account.priority ?? 0}</span>
          </div>
          {#if account.last_error}
            <p class="mt-2 truncate text-xs text-red-500 dark:text-red-400" title={account.last_error}>{account.last_error}</p>
          {/if}
          <div class="mt-3 flex items-center justify-between border-t border-gray-100 pt-3 dark:border-gray-700">
            <span class="text-xs text-gray-400 dark:text-gray-500">{formatDate(account.created_at)}</span>
            <div class="flex gap-1">
              <button onclick={() => openEditModal(account)} class="rounded-md p-1.5 text-gray-400 transition-colors hover:bg-gray-100 hover:text-blue-600 dark:hover:bg-gray-700 dark:hover:text-blue-400" aria-label="Edit {account.name}">
                <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
              </button>
              <button onclick={() => openDeleteConfirm(account)} class="rounded-md p-1.5 text-gray-400 transition-colors hover:bg-red-50 hover:text-red-600 dark:hover:bg-red-900/20 dark:hover:text-red-400" aria-label="Delete {account.name}">
                <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/><line x1="10" y1="11" x2="10" y2="17"/><line x1="14" y1="11" x2="14" y2="17"/></svg>
              </button>
            </div>
          </div>
        </div>
      {/each}
    </div>

    <!-- Pagination -->
    {#if totalPages > 1}
      <div class="flex items-center justify-between">
        <p class="text-sm text-gray-500 dark:text-gray-400">Page {page} of {totalPages}</p>
        <div class="flex gap-1">
          <button onclick={() => goPage(page - 1)} disabled={page <= 1} class="rounded-md border border-gray-300 bg-white px-3 py-1.5 text-sm text-gray-700 transition-colors hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-40 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700" aria-label="Previous page">
            <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"/></svg>
          </button>
          <button onclick={() => goPage(page + 1)} disabled={page >= totalPages} class="rounded-md border border-gray-300 bg-white px-3 py-1.5 text-sm text-gray-700 transition-colors hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-40 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700" aria-label="Next page">
            <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"/></svg>
          </button>
        </div>
      </div>
    {/if}
  {/if}
</div>

<!-- Create Modal -->
{#if showCreateModal}
  <div class="fixed inset-0 z-40 flex items-center justify-center bg-black/50 p-4" role="dialog" aria-modal="true" aria-labelledby="create-modal-title" tabindex="-1" onkeydown={(e) => handleModalKeydown(e, closeCreateModal)} onclick={closeCreateModal}>
    <div class="w-full max-w-lg rounded-xl bg-white p-6 shadow-xl dark:bg-gray-800" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <h2 id="create-modal-title" class="text-lg font-semibold text-gray-900 dark:text-white">添加渠道</h2>
      <form onsubmit={(e) => { e.preventDefault(); submitCreate(); }} class="mt-5 space-y-4">
        <div>
          <label for="create-name" class="block text-sm font-medium text-gray-700 dark:text-gray-300">Name</label>
          <input id="create-name" type="text" required bind:value={formName} class="mt-1 w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-white" placeholder="e.g. OpenAI Main" />
        </div>
        <div class="grid grid-cols-2 gap-4">
          <div>
            <label for="create-provider" class="block text-sm font-medium text-gray-700 dark:text-gray-300">Provider</label>
            <select id="create-provider" bind:value={formProvider} class="mt-1 w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-white">
              {#each PROVIDERS as p}
                <option value={p}>{p.charAt(0).toUpperCase() + p.slice(1)}</option>
              {/each}
            </select>
          </div>
          <div>
            <label for="create-cred-type" class="block text-sm font-medium text-gray-700 dark:text-gray-300">Credential Type</label>
            <select id="create-cred-type" bind:value={formCredentialType} class="mt-1 w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-white">
              {#each CREDENTIAL_TYPES as ct}
                <option value={ct}>{ct}</option>
              {/each}
            </select>
          </div>
        </div>
        <div>
          <label for="create-credential" class="block text-sm font-medium text-gray-700 dark:text-gray-300">Credential</label>
          <input id="create-credential" type="password" required bind:value={formCredential} class="mt-1 w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm font-mono text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-white" placeholder="sk-..." />
        </div>
        <div>
          <label for="create-priority" class="block text-sm font-medium text-gray-700 dark:text-gray-300">Priority</label>
          <input id="create-priority" type="number" bind:value={formPriority} class="mt-1 w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-white" />
        </div>
        <div class="flex justify-end gap-3 pt-2">
          <button type="button" onclick={closeCreateModal} class="rounded-lg border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600">取消</button>
          <button type="submit" disabled={formSubmitting} class="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-700 disabled:opacity-50">{formSubmitting ? '提交中...' : '创建'}</button>
        </div>
      </form>
    </div>
  </div>
{/if}

<!-- Edit Modal -->
{#if showEditModal && editTarget}
  <div class="fixed inset-0 z-40 flex items-center justify-center bg-black/50 p-4" role="dialog" aria-modal="true" aria-labelledby="edit-modal-title" tabindex="-1" onkeydown={(e) => handleModalKeydown(e, closeEditModal)} onclick={closeEditModal}>
    <div class="w-full max-w-lg rounded-xl bg-white p-6 shadow-xl dark:bg-gray-800" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <h2 id="edit-modal-title" class="text-lg font-semibold text-gray-900 dark:text-white">编辑渠道</h2>
      <form onsubmit={(e) => { e.preventDefault(); submitEdit(); }} class="mt-5 space-y-4">
        <div>
          <label for="edit-name" class="block text-sm font-medium text-gray-700 dark:text-gray-300">Name</label>
          <input id="edit-name" type="text" required bind:value={formName} class="mt-1 w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-white" />
        </div>
        <div class="grid grid-cols-2 gap-4">
          <div>
            <label for="edit-provider" class="block text-sm font-medium text-gray-700 dark:text-gray-300">Provider</label>
            <select id="edit-provider" bind:value={formProvider} class="mt-1 w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-white">
              {#each PROVIDERS as p}
                <option value={p}>{p.charAt(0).toUpperCase() + p.slice(1)}</option>
              {/each}
            </select>
          </div>
          <div>
            <label for="edit-cred-type" class="block text-sm font-medium text-gray-700 dark:text-gray-300">Credential Type</label>
            <select id="edit-cred-type" bind:value={formCredentialType} class="mt-1 w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-white">
              {#each CREDENTIAL_TYPES as ct}
                <option value={ct}>{ct}</option>
              {/each}
            </select>
          </div>
        </div>
        <div>
          <label for="edit-credential" class="block text-sm font-medium text-gray-700 dark:text-gray-300">Credential <span class="font-normal text-gray-400">(leave blank to keep current)</span></label>
          <input id="edit-credential" type="password" bind:value={formCredential} class="mt-1 w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm font-mono text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-white" placeholder="unchanged" />
        </div>
        <div>
          <label for="edit-priority" class="block text-sm font-medium text-gray-700 dark:text-gray-300">Priority</label>
          <input id="edit-priority" type="number" bind:value={formPriority} class="mt-1 w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-white" />
        </div>
        <div class="flex justify-end gap-3 pt-2">
          <button type="button" onclick={closeEditModal} class="rounded-lg border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600">取消</button>
          <button type="submit" disabled={formSubmitting} class="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-700 disabled:opacity-50">{formSubmitting ? '提交中...' : '保存'}</button>
        </div>
      </form>
    </div>
  </div>
{/if}

<!-- Delete Confirmation -->
{#if showDeleteConfirm && deleteTarget}
  <div class="fixed inset-0 z-40 flex items-center justify-center bg-black/50 p-4" role="dialog" aria-modal="true" aria-labelledby="delete-modal-title" tabindex="-1" onkeydown={(e) => handleModalKeydown(e, closeDeleteConfirm)} onclick={closeDeleteConfirm}>
    <div class="w-full max-w-sm rounded-xl bg-white p-6 shadow-xl dark:bg-gray-800" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <div class="flex items-start gap-3">
        <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-red-100 dark:bg-red-900/30">
          <svg class="h-5 w-5 text-red-600 dark:text-red-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
        </div>
        <div>
          <h2 id="delete-modal-title" class="text-base font-semibold text-gray-900 dark:text-white">确认删除</h2>
          <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">确定要删除渠道 <span class="font-medium text-gray-700 dark:text-gray-200">{deleteTarget.name}</span> 吗？此操作不可撤销。</p>
        </div>
      </div>
      <div class="mt-5 flex justify-end gap-3">
        <button type="button" onclick={closeDeleteConfirm} class="rounded-lg border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600">取消</button>
        <button type="button" onclick={submitDelete} disabled={deleteSubmitting} class="rounded-lg bg-red-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-red-700 disabled:opacity-50">{deleteSubmitting ? '删除中...' : '删除'}</button>
      </div>
    </div>
  </div>
{/if}

<!-- Import Modal -->
{#if showImportModal}
  <div class="fixed inset-0 z-40 flex items-center justify-center bg-black/50 p-4" role="dialog" aria-modal="true" aria-labelledby="import-modal-title" tabindex="-1" onkeydown={(e) => handleModalKeydown(e, closeImportModal)} onclick={closeImportModal}>
    <div class="w-full max-w-xl rounded-xl bg-white p-6 shadow-xl dark:bg-gray-800" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <h2 id="import-modal-title" class="text-lg font-semibold text-gray-900 dark:text-white">批量导入</h2>
      <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">Paste a JSON array of accounts below.</p>
      <form onsubmit={(e) => { e.preventDefault(); submitImport(); }} class="mt-4 space-y-4">
        <div>
          <label for="import-json" class="block text-sm font-medium text-gray-700 dark:text-gray-300">JSON Data</label>
          <textarea
            id="import-json"
            rows="10"
            required
            bind:value={importJson}
            class="mt-1 w-full rounded-lg border border-gray-300 bg-white px-3 py-2 font-mono text-xs text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
            placeholder={`[\n  {\n    "name": "OpenAI Main",\n    "provider": "openai",\n    "credential_type": "api_key",\n    "credential": "sk-..."\n  },\n  {\n    "name": "Claude Pro",\n    "provider": "anthropic",\n    "credential_type": "api_key",\n    "credential": "sk-ant-..."\n  }\n]`}
          ></textarea>
        </div>
        {#if importResult}
          <div class="rounded-lg border p-3 text-sm {importResult.failed > 0 ? 'border-yellow-200 bg-yellow-50 dark:border-yellow-800 dark:bg-yellow-900/20' : 'border-green-200 bg-green-50 dark:border-green-800 dark:bg-green-900/20'}">
            <p class="font-medium {importResult.failed > 0 ? 'text-yellow-800 dark:text-yellow-200' : 'text-green-800 dark:text-green-200'}">
              Import complete: {importResult.succeeded} succeeded, {importResult.failed} failed
            </p>
            {#if importResult.errors.length > 0}
              <ul class="mt-2 list-inside list-disc text-xs text-yellow-700 dark:text-yellow-300">
                {#each importResult.errors as err}
                  <li>{err}</li>
                {/each}
              </ul>
            {/if}
          </div>
        {/if}
        <div class="flex justify-end gap-3 pt-2">
          <button type="button" onclick={closeImportModal} class="rounded-lg border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600">关闭</button>
          <button type="submit" disabled={importSubmitting} class="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-700 disabled:opacity-50">{importSubmitting ? '导入中...' : '导入'}</button>
        </div>
      </form>
    </div>
  </div>
{/if}
