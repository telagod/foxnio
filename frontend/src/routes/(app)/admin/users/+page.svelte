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

  // Toast
  let toast = $state<{ message: string; type: 'success' | 'error' } | null>(null);
  let toastTimeout: ReturnType<typeof setTimeout> | null = null;

  // Modals
  let showCreateModal = $state(false);
  let showEditModal = $state(false);
  let showDeleteConfirm = $state(false);

  // Create form
  let createEmail = $state('');
  let createPassword = $state('');
  let createRole = $state('user');
  let createSubmitting = $state(false);

  // Edit form
  let editTarget = $state<User | null>(null);
  let editRole = $state('user');
  let editStatus = $state('active');
  let editBalanceDelta = $state(0);
  let editBalanceReason = $state('');
  let editSubmitting = $state(false);

  // Delete
  let deleteTarget = $state<User | null>(null);
  let deleteSubmitting = $state(false);

  onMount(() => {
    const token = localStorage.getItem('token');
    if (token) api.setToken(token);
    loadUsers();
  });

  function showToast(message: string, type: 'success' | 'error') {
    if (toastTimeout) clearTimeout(toastTimeout);
    toast = { message, type };
    toastTimeout = setTimeout(() => { toast = null; }, 3500);
  }
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

  function handleWindowKeydown(e: KeyboardEvent) {
    if (e.key !== 'Escape') return;
    if (showDeleteConfirm) return closeDeleteConfirm();
    if (showEditModal) return closeEditModal();
    if (showCreateModal) return closeCreateModal();
  }

  // Create
  function openCreateModal() {
    createEmail = '';
    createPassword = '';
    createRole = 'user';
    createSubmitting = false;
    showCreateModal = true;
  }
  function closeCreateModal() { showCreateModal = false; }

  async function submitCreate() {
    if (!createEmail.trim() || !createPassword.trim()) return;
    createSubmitting = true;
    try {
      await api.createUser({ email: createEmail.trim(), password: createPassword, role: createRole });
      showCreateModal = false;
      showToast('用户创建成功', 'success');
      loadUsers();
    } catch (e) {
      showToast(e instanceof Error ? e.message : 'Create failed', 'error');
    } finally {
      createSubmitting = false;
    }
  }
  // Edit
  function openEditModal(user: User) {
    editTarget = user;
    editRole = user.role;
    editStatus = user.status;
    editBalanceDelta = 0;
    editBalanceReason = '';
    editSubmitting = false;
    showEditModal = true;
  }
  function closeEditModal() { showEditModal = false; editTarget = null; }

  async function submitEdit() {
    if (!editTarget) return;
    editSubmitting = true;
    try {
      const needsUpdate = editRole !== editTarget.role || editStatus !== editTarget.status;
      const needsBalance = editBalanceDelta !== 0 && editBalanceReason.trim();

      if (needsUpdate) {
        await api.updateUser(editTarget.id, { role: editRole, status: editStatus });
      }
      if (needsBalance) {
        await api.updateUserBalance(editTarget.id, editBalanceDelta, editBalanceReason.trim());
      }
      showEditModal = false;
      editTarget = null;
      showToast('用户更新成功', 'success');
      loadUsers();
    } catch (e) {
      showToast(e instanceof Error ? e.message : 'Update failed', 'error');
    } finally {
      editSubmitting = false;
    }
  }

  // Delete
  function openDeleteConfirm(user: User) {
    deleteTarget = user;
    deleteSubmitting = false;
    showDeleteConfirm = true;
  }
  function closeDeleteConfirm() { showDeleteConfirm = false; deleteTarget = null; }

  async function submitDelete() {
    if (!deleteTarget) return;
    deleteSubmitting = true;
    try {
      await api.deleteUser(deleteTarget.id);
      showDeleteConfirm = false;
      deleteTarget = null;
      showToast('用户已删除', 'success');
      loadUsers();
    } catch (e) {
      showToast(e instanceof Error ? e.message : 'Delete failed', 'error');
    } finally {
      deleteSubmitting = false;
    }
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

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
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">User Management</h1>
      <p class="mt-1 text-gray-500 dark:text-gray-400">Manage all registered users ({total} total)</p>
    </div>
    <div class="flex gap-2">
      <button
        onclick={loadUsers}
        disabled={loading}
        class="inline-flex items-center justify-center gap-2 rounded-lg border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-50 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-200 dark:hover:bg-gray-700"
        aria-label="Refresh users"
      >
        <svg class="h-4 w-4" class:animate-spin={loading} fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" stroke-linecap="round" stroke-linejoin="round"><path d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"></path></svg>
        Refresh
      </button>
      <button
        onclick={openCreateModal}
        class="inline-flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-700"
      >
        <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        创建用户
      </button>
    </div>
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
      <div class="flex-1">
        <div class="text-sm font-medium text-red-800 dark:text-red-200">Failed to load</div>
        <div class="mt-1 text-sm text-red-700 dark:text-red-300">{error}</div>
      </div>
      <button onclick={loadUsers} class="shrink-0 rounded-md bg-red-100 px-3 py-1 text-xs font-medium text-red-700 transition-colors hover:bg-red-200 dark:bg-red-900/40 dark:text-red-300 dark:hover:bg-red-900/60">重试</button>
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
    <div class="flex flex-col items-center justify-center rounded-xl border border-dashed border-gray-300 bg-white py-16 dark:border-gray-600 dark:bg-gray-800/50">
      <svg class="h-12 w-12 text-gray-300 dark:text-gray-600" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4-4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 00-3-3.87"/><path d="M16 3.13a4 4 0 010 7.75"/></svg>
      <p class="mt-4 text-sm text-gray-500 dark:text-gray-400">{search ? 'No users match your search' : 'No users found'}</p>
    </div>
  {:else}
    <!-- Desktop table -->
    <div class="hidden overflow-hidden rounded-xl border border-gray-200 bg-white shadow-sm dark:border-gray-700 dark:bg-gray-800 md:block">
      <table class="min-w-full divide-y divide-gray-200 text-sm dark:divide-gray-700">
        <thead>
          <tr class="bg-gray-50 text-left text-xs font-medium uppercase tracking-wider text-gray-500 dark:bg-gray-800/60 dark:text-gray-400">
            <th scope="col" class="px-6 py-3">Email</th>
            <th scope="col" class="px-6 py-3">Role</th>
            <th scope="col" class="px-6 py-3">Status</th>
            <th scope="col" class="px-6 py-3">Balance</th>
            <th scope="col" class="px-6 py-3">Created</th>
            <th scope="col" class="px-6 py-3 text-right">Actions</th>
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
              <td class="whitespace-nowrap px-6 py-4 text-right">
                <div class="flex items-center justify-end gap-1">
                  <button onclick={() => openEditModal(user)} class="rounded-md p-1.5 text-gray-400 transition-colors hover:bg-gray-100 hover:text-blue-600 dark:hover:bg-gray-700 dark:hover:text-blue-400" aria-label="Edit {user.email}">
                    <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
                  </button>
                  <button onclick={() => openDeleteConfirm(user)} class="rounded-md p-1.5 text-gray-400 transition-colors hover:bg-red-50 hover:text-red-600 dark:hover:bg-red-900/20 dark:hover:text-red-400" aria-label="Delete {user.email}">
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
          <div class="mt-3 flex items-center justify-end gap-1 border-t border-gray-100 pt-3 dark:border-gray-700">
            <button onclick={() => openEditModal(user)} class="rounded-md p-1.5 text-gray-400 transition-colors hover:bg-gray-100 hover:text-blue-600 dark:hover:bg-gray-700 dark:hover:text-blue-400" aria-label="Edit {user.email}">
              <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
            </button>
            <button onclick={() => openDeleteConfirm(user)} class="rounded-md p-1.5 text-gray-400 transition-colors hover:bg-red-50 hover:text-red-600 dark:hover:bg-red-900/20 dark:hover:text-red-400" aria-label="Delete {user.email}">
              <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/><line x1="10" y1="11" x2="10" y2="17"/><line x1="14" y1="11" x2="14" y2="17"/></svg>
            </button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
<!-- Create User Modal -->
{#if showCreateModal}
  <div class="fixed inset-0 z-40 flex items-center justify-center p-4" role="dialog" aria-modal="true" aria-labelledby="create-user-title">
    <button type="button" class="absolute inset-0 bg-black/50" aria-label="关闭创建用户弹窗" onclick={closeCreateModal}></button>
    <div class="relative w-full max-w-lg rounded-xl bg-white p-6 shadow-xl dark:bg-gray-800" role="document">
      <h2 id="create-user-title" class="text-lg font-semibold text-gray-900 dark:text-white">创建用户</h2>
      <form onsubmit={(e) => { e.preventDefault(); submitCreate(); }} class="mt-5 space-y-4">
        <div>
          <label for="create-user-email" class="block text-sm font-medium text-gray-700 dark:text-gray-300">邮箱</label>
          <input id="create-user-email" type="email" required bind:value={createEmail} class="mt-1 w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-white" placeholder="user@example.com" />
        </div>
        <div>
          <label for="create-user-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300">密码</label>
          <input id="create-user-password" type="password" required minlength={8} bind:value={createPassword} autocomplete="new-password" class="mt-1 w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-white" placeholder="至少 8 个字符" />
        </div>
        <div>
          <label for="create-user-role" class="block text-sm font-medium text-gray-700 dark:text-gray-300">角色</label>
          <select id="create-user-role" bind:value={createRole} class="mt-1 w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-white">
            <option value="user">User</option>
            <option value="admin">Admin</option>
          </select>
        </div>
        <div class="flex justify-end gap-3 pt-2">
          <button type="button" onclick={closeCreateModal} class="rounded-lg border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600">取消</button>
          <button type="submit" disabled={createSubmitting} class="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-700 disabled:opacity-50">{createSubmitting ? '提交中...' : '创建'}</button>
        </div>
      </form>
    </div>
  </div>
{/if}

<!-- Edit User Modal -->
{#if showEditModal && editTarget}
  <div class="fixed inset-0 z-40 flex items-center justify-center p-4" role="dialog" aria-modal="true" aria-labelledby="edit-user-title">
    <button type="button" class="absolute inset-0 bg-black/50" aria-label="关闭编辑用户弹窗" onclick={closeEditModal}></button>
    <div class="relative w-full max-w-lg rounded-xl bg-white p-6 shadow-xl dark:bg-gray-800" role="document">
      <h2 id="edit-user-title" class="text-lg font-semibold text-gray-900 dark:text-white">编辑用户</h2>
      <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">{editTarget.email}</p>
      <form onsubmit={(e) => { e.preventDefault(); submitEdit(); }} class="mt-5 space-y-4">
        <div class="grid grid-cols-2 gap-4">
          <div>
            <label for="edit-user-role" class="block text-sm font-medium text-gray-700 dark:text-gray-300">角色</label>
            <select id="edit-user-role" bind:value={editRole} class="mt-1 w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-white">
              <option value="user">User</option>
              <option value="admin">Admin</option>
            </select>
          </div>
          <div>
            <label for="edit-user-status" class="block text-sm font-medium text-gray-700 dark:text-gray-300">状态</label>
            <select id="edit-user-status" bind:value={editStatus} class="mt-1 w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-white">
              <option value="active">Active</option>
              <option value="suspended">Suspended</option>
            </select>
          </div>
        </div>
        <div class="border-t border-gray-200 pt-4 dark:border-gray-700">
          <h3 class="text-sm font-medium text-gray-700 dark:text-gray-300">余额调整</h3>
          <p class="mt-0.5 text-xs text-gray-500 dark:text-gray-400">当前余额: {formatBalance(editTarget.balance)}</p>
          <div class="mt-3 grid grid-cols-2 gap-4">
            <div>
              <label for="edit-balance-delta" class="block text-sm font-medium text-gray-700 dark:text-gray-300">调整金额 (分)</label>
              <input id="edit-balance-delta" type="number" step="1" bind:value={editBalanceDelta} class="mt-1 w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-white" placeholder="正数充值，负数扣减" />
            </div>
            <div>
              <label for="edit-balance-reason" class="block text-sm font-medium text-gray-700 dark:text-gray-300">原因</label>
              <input id="edit-balance-reason" type="text" bind:value={editBalanceReason} class="mt-1 w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-white" placeholder="调整原因" />
            </div>
          </div>
          {#if editBalanceDelta !== 0 && !editBalanceReason.trim()}
            <p class="mt-2 text-xs text-amber-600 dark:text-amber-400">调整余额时需填写原因</p>
          {/if}
        </div>
        <div class="flex justify-end gap-3 pt-2">
          <button type="button" onclick={closeEditModal} class="rounded-lg border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600">取消</button>
          <button type="submit" disabled={editSubmitting || (editBalanceDelta !== 0 && !editBalanceReason.trim())} class="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-700 disabled:opacity-50">{editSubmitting ? '提交中...' : '保存'}</button>
        </div>
      </form>
    </div>
  </div>
{/if}

<!-- Delete Confirmation -->
{#if showDeleteConfirm && deleteTarget}
  <div class="fixed inset-0 z-40 flex items-center justify-center p-4" role="dialog" aria-modal="true" aria-labelledby="delete-user-title">
    <button type="button" class="absolute inset-0 bg-black/50" aria-label="关闭删除用户弹窗" onclick={closeDeleteConfirm}></button>
    <div class="relative w-full max-w-sm rounded-xl bg-white p-6 shadow-xl dark:bg-gray-800" role="document">
      <div class="flex items-start gap-3">
        <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-red-100 dark:bg-red-900/30">
          <svg class="h-5 w-5 text-red-600 dark:text-red-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
        </div>
        <div>
          <h2 id="delete-user-title" class="text-base font-semibold text-gray-900 dark:text-white">确认删除</h2>
          <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">确定要删除用户 <span class="font-medium text-gray-700 dark:text-gray-200">{deleteTarget.email}</span> 吗？此操作不可撤销。</p>
        </div>
      </div>
      <div class="mt-5 flex justify-end gap-3">
        <button type="button" onclick={closeDeleteConfirm} class="rounded-lg border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600">取消</button>
        <button type="button" onclick={submitDelete} disabled={deleteSubmitting} class="rounded-lg bg-red-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-red-700 disabled:opacity-50">{deleteSubmitting ? '删除中...' : '删除'}</button>
      </div>
    </div>
  </div>
{/if}
