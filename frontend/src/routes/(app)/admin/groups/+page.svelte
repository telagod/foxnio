<svelte:head>
  <title>分组管理 - Admin</title>
</svelte:head>

<script lang="ts">
  import { onMount } from 'svelte';
  import {
    api,
    type GroupCapacitySummary,
    type GroupInfo,
    type GroupUpdatePayload,
    type GroupUsageSummary
  } from '$lib/api';

  const PLATFORMS = ['anthropic', 'openai', 'gemini', 'antigravity'] as const;
  const SCHEDULING_POLICIES = [
    { value: 'load_balance', label: '负载均衡' },
    { value: 'sticky', label: '强粘性' },
    { value: 'scoring', label: '评分算法' },
  ] as const;

  let groups = $state<GroupInfo[]>([]);
  let usageSummaries = $state<GroupUsageSummary[]>([]);
  let capacitySummaries = $state<GroupCapacitySummary[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let platformFilter = $state('');
  let searchKeyword = $state('');

  let showCreateModal = $state(false);
  let showDeleteModal = $state(false);
  let showEditModal = $state(false);
  let deletingId = $state<number | null>(null);
  let createSubmitting = $state(false);
  let editSubmitting = $state(false);
  let editingId = $state<number | null>(null);

  let formName = $state('');
  let formPlatform = $state('openai');
  let formDescription = $state('');
  let formDailyLimit = $state<string>('');
  let formMonthlyLimit = $state<string>('');
  let formRateMultiplier = $state<string>('');
  let formSchedulingPolicy = $state('load_balance');

  let editName = $state('');
  let editDescription = $state('');
  let editStatus = $state('active');
  let editRateMultiplier = $state('');

  let toast = $state<{ message: string; type: 'success' | 'error' } | null>(null);
  let toastTimeout: ReturnType<typeof setTimeout> | null = null;

  const filteredGroups = $derived(
    groups.filter((group) => {
      if (platformFilter && group.platform !== platformFilter) return false;
      if (!searchKeyword) return true;
      return (
        group.name.toLowerCase().includes(searchKeyword.toLowerCase()) ||
        group.platform.toLowerCase().includes(searchKeyword.toLowerCase())
      );
    })
  );

  const totalGroups = $derived(filteredGroups.length);
  const activeGroups = $derived(
    filteredGroups.filter((group) => group.status === 'active' || group.status === 'running').length
  );
  const totalAccounts = $derived(
    usageSummaries.reduce((sum, item) => sum + item.account_count, 0)
  );

  const usageMap = $derived(
    new Map(usageSummaries.map((item) => [item.group_id, item]))
  );
  const capacityMap = $derived(
    new Map(capacitySummaries.map((item) => [item.group_id, item]))
  );

  onMount(() => {
    const token = localStorage.getItem('token');
    if (token) api.setToken(token);
    loadAll();
  });

  function showToast(message: string, type: 'success' | 'error') {
    if (toastTimeout) clearTimeout(toastTimeout);
    toast = { message, type };
    toastTimeout = setTimeout(() => (toast = null), 3200);
  }

  function hideToast() {
    if (toastTimeout) clearTimeout(toastTimeout);
    toast = null;
  }

  async function loadAll() {
    loading = true;
    error = null;
    try {
      const [groupRes, usageRes, capRes] = await Promise.all([
        api.listAllGroups(),
        api.getGroupUsageSummary(),
        api.getGroupCapacitySummary(),
      ]);
      groups = groupRes.data ?? [];
      usageSummaries = usageRes.data ?? [];
      capacitySummaries = capRes.data ?? [];
    } catch (e) {
      error = e instanceof Error ? e.message : '加载分组数据失败';
    } finally {
      loading = false;
    }
  }

  function onPlatformFilter(platform: string) {
    platformFilter = platform;
  }

  function resetFilters() {
    platformFilter = '';
    searchKeyword = '';
  }

  function openCreateModal() {
    formName = '';
    formPlatform = 'openai';
    formDescription = '';
    formDailyLimit = '';
    formMonthlyLimit = '';
    formRateMultiplier = '';
    formSchedulingPolicy = 'load_balance';
    showCreateModal = true;
  }

  function closeCreateModal() {
    showCreateModal = false;
    createSubmitting = false;
  }

  function openEditModal(group: GroupInfo) {
    editingId = group.id;
    editName = group.name;
    editDescription = group.description ?? '';
    editStatus = group.status;
    editRateMultiplier = '';
    showEditModal = true;
  }

  function closeEditModal() {
    showEditModal = false;
    editingId = null;
    editSubmitting = false;
  }

  function statusColor(status: string): string {
    if (status === 'active' || status === 'running') return 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300';
    if (status === 'disabled' || status === 'inactive') return 'bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300';
    return 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-300';
  }

  function formatMoney(value: number): string {
    return `¥${value.toFixed(2)}`;
  }

  function percentToText(value: number): string {
    return `${value.toFixed(1)}%`;
  }

  function getUsageSummary(groupId: number): GroupUsageSummary | undefined {
    return usageMap.get(groupId);
  }

  function getCapacitySummary(groupId: number): GroupCapacitySummary | undefined {
    return capacityMap.get(groupId);
  }

  function progressWidth(value: number): string {
    return `${Math.min(100, Math.max(0, value))}%`;
  }

  async function submitCreate() {
    if (!formName.trim() || !formPlatform) {
      showToast('名称和平台必填', 'error');
      return;
    }
    createSubmitting = true;
    try {
      await api.createGroup({
        name: formName.trim(),
        platform: formPlatform,
        description: formDescription.trim() || undefined,
        daily_limit_usd: formDailyLimit ? Number(formDailyLimit) : undefined,
        monthly_limit_usd: formMonthlyLimit ? Number(formMonthlyLimit) : undefined,
        rate_multiplier: formRateMultiplier ? Number(formRateMultiplier) : undefined,
        scheduling_policy: formSchedulingPolicy || undefined,
      });
      closeCreateModal();
      showToast('分组创建成功', 'success');
      await loadAll();
    } catch (e) {
      showToast(e instanceof Error ? e.message : '创建分组失败', 'error');
    } finally {
      createSubmitting = false;
    }
  }

  function confirmDelete(id: number) {
    deletingId = id;
    showDeleteModal = true;
  }

  function closeDeleteModal() {
    deletingId = null;
    showDeleteModal = false;
  }

  async function doDelete() {
    if (deletingId === null) return;
    try {
      await api.deleteGroup(deletingId);
      showToast('分组已删除', 'success');
      await loadAll();
      closeDeleteModal();
    } catch (e) {
      showToast(e instanceof Error ? e.message : '删除失败', 'error');
    }
  }

  async function submitEdit() {
    if (!editingId || !editName.trim()) {
      showToast('分组名不能为空', 'error');
      return;
    }

    editSubmitting = true;
    try {
      const payload: GroupUpdatePayload = {
        name: editName.trim(),
        description: editDescription.trim() || null,
        status: editStatus,
        rate_multiplier: editRateMultiplier ? Number(editRateMultiplier) : undefined,
      };

      await api.updateGroup(editingId, payload);
      closeEditModal();
      showToast('分组更新成功', 'success');
      await loadAll();
    } catch (e) {
      showToast(e instanceof Error ? e.message : '更新失败', 'error');
    } finally {
      editSubmitting = false;
    }
  }
</script>

{#if toast}
  <div
    class="fixed right-4 top-4 z-50 flex items-center gap-2 rounded-lg px-4 py-3 text-sm font-medium text-white shadow-lg {toast.type === 'success' ? 'bg-emerald-600' : 'bg-red-600'}"
    role="status"
    aria-live="polite"
  >
    <svg class="h-4 w-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      {#if toast.type === 'success'}
        <path d="M20 6L9 17l-5-5"/>
      {:else}
        <circle cx="12" cy="12" r="10"/><path d="m15 9-5 5-2-2"/>
      {/if}
    </svg>
    <span>{toast.message}</span>
    <button onclick={hideToast} class="ml-2 rounded-full px-2 py-0.5 text-xs opacity-80 hover:opacity-100" aria-label="close toast">×</button>
  </div>
{/if}

<div class="space-y-6">
  <div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
    <div>
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">分组管理</h1>
      <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">运营分组容量、额度与账号关系</p>
    </div>
    <div class="flex items-center gap-2">
      <button
        onclick={openCreateModal}
        class="rounded-lg bg-blue-600 px-3 py-2 text-sm font-medium text-white hover:bg-blue-700"
      >
        + 新建分组
      </button>
      <button
        onclick={loadAll}
        class="rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-200 dark:hover:bg-gray-700"
      >
        刷新
      </button>
    </div>
  </div>

  <div class="flex flex-col gap-3 rounded-xl border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-800 sm:flex-row sm:items-end">
    <div class="flex-1">
      <label class="mb-1 block text-xs text-gray-500 dark:text-gray-400" for="group-search">搜索分组</label>
      <input
        id="group-search"
        type="text"
        value={searchKeyword}
        oninput={(e) => (searchKeyword = e.currentTarget.value)}
        class="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
        placeholder="输入分组名称/平台关键词"
      />
    </div>
    <div class="min-w-40">
      <label class="mb-1 block text-xs text-gray-500 dark:text-gray-400" for="platform-filter">按平台过滤</label>
      <select
        id="platform-filter"
        value={platformFilter}
        onchange={(e) => onPlatformFilter(e.currentTarget.value)}
        class="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-700 dark:border-gray-600 dark:bg-gray-900 dark:text-gray-200"
      >
        <option value="">全部</option>
        {#each PLATFORMS as p}
          <option value={p}>{p}</option>
        {/each}
      </select>
    </div>
    <button
      onclick={resetFilters}
      class="h-10 rounded-lg border border-gray-300 px-3 py-2 text-sm text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-200 dark:hover:bg-gray-700"
    >
      清空
    </button>
  </div>

  <div class="grid gap-4 md:grid-cols-4">
    <div class="rounded-xl border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800">
      <p class="text-xs text-gray-500 dark:text-gray-400">分组总数</p>
      <p class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{totalGroups}</p>
    </div>
    <div class="rounded-xl border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800">
      <p class="text-xs text-gray-500 dark:text-gray-400">活跃分组</p>
      <p class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">{activeGroups}</p>
    </div>
    <div class="rounded-xl border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800">
      <p class="text-xs text-gray-500 dark:text-gray-400">已配置日额度</p>
      <p class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">
        {usageSummaries.filter((item) => item.daily_limit_usd > 0).length}
      </p>
    </div>
    <div class="rounded-xl border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800">
      <p class="text-xs text-gray-500 dark:text-gray-400">已配置月额度</p>
      <p class="mt-2 text-2xl font-bold text-gray-900 dark:text-white">
        {usageSummaries.filter((item) => item.monthly_limit_usd > 0).length}
      </p>
    </div>
  </div>

  <p class="text-sm text-gray-500 dark:text-gray-400">
    当前账户池总账号数（分组口径）：<span class="font-semibold text-gray-900 dark:text-white">{totalAccounts}</span>
  </p>

  {#if error}
    <div class="rounded-lg border border-red-200 bg-red-50 p-4 text-sm text-red-700 dark:border-red-800 dark:bg-red-900/20 dark:text-red-200">
      {error}
    </div>
  {/if}

  {#if loading}
    <div class="flex h-56 items-center justify-center">
      <svg class="h-8 w-8 animate-spin text-blue-500" viewBox="0 0 24 24" fill="none">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3"></circle>
        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
      </svg>
    </div>
  {:else if filteredGroups.length === 0}
    <div class="rounded-xl border border-dashed border-gray-300 bg-white p-8 text-center text-sm text-gray-500 dark:border-gray-600 dark:bg-gray-800/50">
      当前无分组数据，建议先创建分组。
    </div>
  {:else}
    <div class="overflow-hidden rounded-xl border border-gray-200 bg-white shadow-sm dark:border-gray-700 dark:bg-gray-800">
      <table class="min-w-full divide-y divide-gray-200 text-sm dark:divide-gray-700">
        <thead class="bg-gray-50 text-left dark:bg-gray-800/70">
          <tr class="text-xs font-medium uppercase tracking-wider text-gray-500 dark:text-gray-400">
            <th class="px-4 py-3">分组</th>
            <th class="px-4 py-3">平台</th>
            <th class="px-4 py-3">状态</th>
            <th class="px-4 py-3">账号数</th>
            <th class="px-4 py-3">日额度</th>
            <th class="px-4 py-3">月额度</th>
            <th class="px-4 py-3">容量</th>
            <th class="px-4 py-3">操作</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-gray-100 dark:divide-gray-700/60">
          {#each filteredGroups as group}
            {#key group.id}
              {@const usage = getUsageSummary(group.id)}
              {@const capacity = getCapacitySummary(group.id)}
              <tr class="hover:bg-gray-50 dark:hover:bg-gray-700/20">
                <td class="px-4 py-3">
                  <div class="font-medium text-gray-900 dark:text-white">{group.name}</div>
                  <div class="text-xs text-gray-500 dark:text-gray-400">{group.description ?? '-'}</div>
                  <div class="text-xs text-gray-500 dark:text-gray-400">ID: {group.id}</div>
                </td>
                <td class="px-4 py-3 text-gray-600 dark:text-gray-300">{group.platform}</td>
                <td class="px-4 py-3">
                  <span class="inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium {statusColor(group.status)}">
                    {group.status}
                  </span>
                </td>
                <td class="px-4 py-3 text-gray-700 dark:text-gray-300">{usage ? usage.account_count : group.account_count ?? 0}</td>
                <td class="px-4 py-3 text-gray-700 dark:text-gray-300">
                  {#if usage}
                    <div class="space-y-1">
                      <div class="text-xs">{formatMoney(usage.daily_used_usd)} / {formatMoney(usage.daily_limit_usd)}</div>
                      <div class="h-1.5 overflow-hidden rounded-full bg-gray-200 dark:bg-gray-700">
                        <div class="h-full rounded-full bg-blue-500 transition-all" style={`width: ${progressWidth(usage.daily_usage_percent)};`}></div>
                      </div>
                      <div class="text-xs text-gray-500 dark:text-gray-400">活跃: {usage.active_account_count}</div>
                    </div>
                  {:else}
                    -
                  {/if}
                </td>
                <td class="px-4 py-3 text-gray-700 dark:text-gray-300">
                  {#if usage}
                    <div class="space-y-1">
                      <div class="text-xs">{formatMoney(usage.monthly_used_usd)} / {formatMoney(usage.monthly_limit_usd)}</div>
                      <div class="h-1.5 overflow-hidden rounded-full bg-gray-200 dark:bg-gray-700">
                        <div class="h-full rounded-full bg-emerald-500 transition-all" style={`width: ${progressWidth(usage.monthly_usage_percent)};`}></div>
                      </div>
                      <div class="text-xs text-gray-500 dark:text-gray-400">{percentToText(usage.monthly_usage_percent)}</div>
                    </div>
                  {:else}
                    -
                  {/if}
                </td>
                <td class="px-4 py-3 text-gray-700 dark:text-gray-300">
                  {#if capacity}
                    <div class="space-y-1">
                    <div class="text-xs">{capacity.used_capacity}/{capacity.total_capacity}</div>
                      <div class="h-1.5 overflow-hidden rounded-full bg-gray-200 dark:bg-gray-700">
                        <div class="h-full rounded-full bg-indigo-500 transition-all" style={`width: ${progressWidth(capacity.capacity_percent)};`}></div>
                      </div>
                      <div class="text-xs text-gray-500 dark:text-gray-400">{percentToText(capacity.capacity_percent)}</div>
                    </div>
                  {:else}
                    -
                  {/if}
                </td>
                <td class="px-4 py-3">
                  <div class="flex items-center justify-start gap-2">
                    <a
                      href={`/admin/accounts?group_id=${group.id}`}
                      class="rounded-md bg-cyan-600 px-2.5 py-1.5 text-xs font-medium text-white hover:bg-cyan-700"
                    >
                      查看账号
                    </a>
                    <button
                      onclick={() => openEditModal(group)}
                      class="rounded-md bg-amber-500 px-2.5 py-1.5 text-xs font-medium text-white hover:bg-amber-600"
                    >
                      编辑
                    </button>
                    <button
                      onclick={() => confirmDelete(group.id)}
                      class="rounded-md bg-red-600 px-2.5 py-1.5 text-xs font-medium text-white hover:bg-red-700"
                    >
                      删除
                    </button>
                  </div>
                </td>
              </tr>
            {/key}
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

{#if showCreateModal}
  <div class="fixed inset-0 z-40 flex items-center justify-center bg-black/50 p-4" role="dialog" aria-modal="true">
    <div class="w-full max-w-lg rounded-xl bg-white p-6 shadow-xl dark:bg-gray-800">
      <h2 class="text-lg font-semibold text-gray-900 dark:text-white">新建分组</h2>
      <div class="mt-5 space-y-4">
        <div>
          <label class="mb-1 block text-sm text-gray-700 dark:text-gray-300" for="group-name">分组名</label>
          <input
            id="group-name"
            value={formName}
            oninput={(e) => (formName = e.currentTarget.value)}
            class="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
            placeholder="例如：OpenAI-主池"
          />
        </div>
        <div>
          <label class="mb-1 block text-sm text-gray-700 dark:text-gray-300" for="group-platform">平台</label>
          <select
            id="group-platform"
            value={formPlatform}
            onchange={(e) => (formPlatform = e.currentTarget.value)}
            class="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-700 dark:border-gray-600 dark:bg-gray-900 dark:text-gray-200"
          >
            {#each PLATFORMS as p}
              <option value={p}>{p}</option>
            {/each}
          </select>
        </div>
        <div>
          <label class="mb-1 block text-sm text-gray-700 dark:text-gray-300" for="group-description">描述（可选）</label>
          <textarea
            id="group-description"
            rows="2"
            value={formDescription}
            oninput={(e) => (formDescription = e.currentTarget.value)}
            class="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
            placeholder="分组用途说明"
          ></textarea>
        </div>
        <div class="grid grid-cols-3 gap-3">
          <div>
            <label class="mb-1 block text-sm text-gray-700 dark:text-gray-300" for="group-daily">日额度($)</label>
            <input
              id="group-daily"
              type="number"
              value={formDailyLimit}
              oninput={(e) => (formDailyLimit = e.currentTarget.value)}
              class="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
              placeholder="0"
            />
          </div>
          <div>
            <label class="mb-1 block text-sm text-gray-700 dark:text-gray-300" for="group-monthly">月额度($)</label>
            <input
              id="group-monthly"
              type="number"
              value={formMonthlyLimit}
              oninput={(e) => (formMonthlyLimit = e.currentTarget.value)}
              class="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
              placeholder="0"
            />
          </div>
          <div>
            <label class="mb-1 block text-sm text-gray-700 dark:text-gray-300" for="group-rate">倍率</label>
            <input
              id="group-rate"
              type="number"
              step="0.01"
              value={formRateMultiplier}
              oninput={(e) => (formRateMultiplier = e.currentTarget.value)}
              class="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
              placeholder="1.0"
            />
          </div>
        </div>
        <div>
          <label class="mb-1 block text-sm text-gray-700 dark:text-gray-300" for="group-scheduling">调度策略</label>
          <select
            id="group-scheduling"
            value={formSchedulingPolicy}
            onchange={(e) => (formSchedulingPolicy = e.currentTarget.value)}
            class="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-700 dark:border-gray-600 dark:bg-gray-900 dark:text-gray-200"
          >
            {#each SCHEDULING_POLICIES as p}
              <option value={p.value}>{p.label}</option>
            {/each}
          </select>
        </div>
        <div class="flex justify-end gap-3 pt-2">
          <button
            type="button"
            onclick={closeCreateModal}
            class="rounded-lg border border-gray-300 px-4 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-300"
          >
            取消
          </button>
          <button
            type="button"
            onclick={submitCreate}
            disabled={createSubmitting}
            class="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white disabled:opacity-50"
          >
            {createSubmitting ? '创建中...' : '创建'}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

{#if showEditModal && editingId !== null}
  <div class="fixed inset-0 z-40 flex items-center justify-center bg-black/50 p-4" role="dialog" aria-modal="true">
    <div class="w-full max-w-lg rounded-xl bg-white p-6 shadow-xl dark:bg-gray-800">
      <h2 class="text-lg font-semibold text-gray-900 dark:text-white">编辑分组</h2>
      <div class="mt-5 space-y-4">
        <div>
          <label class="mb-1 block text-sm text-gray-700 dark:text-gray-300" for="group-edit-name">分组名</label>
          <input
            id="group-edit-name"
            value={editName}
            oninput={(e) => (editName = e.currentTarget.value)}
            class="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
            placeholder="例如：OpenAI-主池"
          />
        </div>
        <div>
          <label class="mb-1 block text-sm text-gray-700 dark:text-gray-300" for="group-edit-status">状态</label>
          <select
            id="group-edit-status"
            value={editStatus}
            onchange={(e) => (editStatus = e.currentTarget.value)}
            class="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-700 dark:border-gray-600 dark:bg-gray-900 dark:text-gray-200"
          >
            <option value="active">active</option>
            <option value="disabled">disabled</option>
            <option value="maintenance">maintenance</option>
          </select>
        </div>
        <div>
          <label class="mb-1 block text-sm text-gray-700 dark:text-gray-300" for="group-edit-description">描述</label>
          <textarea
            id="group-edit-description"
            rows="2"
            value={editDescription}
            oninput={(e) => (editDescription = e.currentTarget.value)}
            class="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
            placeholder="分组用途说明"
          ></textarea>
        </div>
        <div>
          <label class="mb-1 block text-sm text-gray-700 dark:text-gray-300" for="group-edit-rate">倍率</label>
          <input
            id="group-edit-rate"
            type="number"
            step="0.01"
            value={editRateMultiplier}
            oninput={(e) => (editRateMultiplier = e.currentTarget.value)}
            class="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
            placeholder="保持空则不修改"
          />
        </div>
        <div class="flex justify-end gap-3 pt-2">
          <button
            type="button"
            onclick={closeEditModal}
            class="rounded-lg border border-gray-300 px-4 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-300"
          >
            取消
          </button>
          <button
            type="button"
            onclick={submitEdit}
            disabled={editSubmitting}
            class="rounded-lg bg-amber-500 px-4 py-2 text-sm font-medium text-white disabled:opacity-50"
          >
            {editSubmitting ? '更新中...' : '保存'}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

{#if showDeleteModal && deletingId !== null}
  <div class="fixed inset-0 z-40 flex items-center justify-center bg-black/50 p-4" role="dialog" aria-modal="true">
    <div class="w-full max-w-sm rounded-xl bg-white p-6 shadow-xl dark:bg-gray-800">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white">确认删除</h3>
      <p class="mt-2 text-sm text-gray-500 dark:text-gray-400">
        确认删除分组 <span class="font-medium text-gray-700 dark:text-gray-200">ID: {deletingId}</span> 吗？删除后不可恢复。
      </p>
      <div class="mt-5 flex justify-end gap-3">
        <button
          type="button"
          onclick={closeDeleteModal}
          class="rounded-lg border border-gray-300 px-4 py-2 text-sm text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-300"
        >
          取消
        </button>
        <button
          type="button"
          onclick={doDelete}
          class="rounded-lg bg-red-600 px-4 py-2 text-sm text-white hover:bg-red-700"
        >
          删除
        </button>
      </div>
    </div>
  </div>
{/if}
