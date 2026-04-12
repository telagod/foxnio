<svelte:head>
  <title>渠道管理 - Admin</title>
</svelte:head>

<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type Account, type GroupInfo, type PaginatedResponse, type ProviderInfo } from '$lib/api';

  const CREDENTIAL_TYPES = ['api_key', 'oauth_token'] as const;
  const BATCH_STATUS_OPTIONS = ['active', 'disabled', 'error'] as const;
  const BATCH_SCOPE_CONFIRM_THRESHOLD = 500;
  const DEFAULT_PROVIDER_KEYS = ['openai', 'anthropic', 'gemini', 'deepseek', 'mistral', 'cohere'];

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
  let filterGroupId = $state('');
  let searchTimeout: ReturnType<typeof setTimeout> | null = null;

  // Batch actions
  let selectedAccountIds = $state<string[]>([]);
  let batchStatus = $state<'active' | 'disabled' | 'error'>('active');
  let batchClearError = $state(false);
  let batchGroupId = $state<string>('');
  let batchSubmitting = $state(false);
  let batchUseFilterScope = $state(false);
  let groups = $state<GroupInfo[]>([]);
  let providers = $state<ProviderInfo[]>([]);
  let groupsLoading = $state(false);
  let groupsError = $state<string | null>(null);

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
  let importResult = $state<{ succeeded: number; failed: number; skipped: number; errors: string[] } | null>(null);
  let importPreview = $state<{
    total: number;
    valid: number;
    invalid: number;
    duplicate: number;
    will_import: number;
    duration_ms: number;
    providers: Array<{
      provider: string;
      total: number;
      valid: number;
      invalid: number;
      duplicate: number;
      will_import: number;
    }>;
    errors: Array<{ index?: number; name?: string; error?: string }>;
  } | null>(null);
  let importPreviewInput = $state('');

  let selectedCount = $derived(selectedAccountIds.length);
  let batchHasFilterScope = $derived(
    Boolean(searchQuery.trim()) || Boolean(filterProvider) || Boolean(filterStatus) || Boolean(filterGroupId)
  );
  let batchScopeHighRisk = $derived(batchHasFilterScope && total >= BATCH_SCOPE_CONFIRM_THRESHOLD);
  let currentPageAllSelected = $derived(
    accounts.length > 0 && accounts.every((account) => selectedAccountIds.includes(account.id))
  );

  function syncUrlFromFilters() {
    if (typeof window === 'undefined') return;
    const params = new URLSearchParams();
    if (page > 1) params.set('page', String(page));
    if (searchQuery.trim()) params.set('search', searchQuery.trim());
    if (filterProvider) params.set('provider', filterProvider);
    if (filterStatus) params.set('status', filterStatus);
    if (filterGroupId) params.set('group_id', filterGroupId);
    const query = params.toString();
    window.history.replaceState({}, '', query ? `${window.location.pathname}?${query}` : window.location.pathname);
  }

  function initFiltersFromUrl() {
    if (typeof window === 'undefined') return;
    const params = new URLSearchParams(window.location.search);
    const pageFromUrl = Number(params.get('page'));
    if (Number.isInteger(pageFromUrl) && pageFromUrl > 0) {
      page = pageFromUrl;
    }
    const searchFromUrl = params.get('search');
    if (searchFromUrl !== null) {
      searchQuery = searchFromUrl;
    }
    const providerFromUrl = params.get('provider');
    if (providerFromUrl && getProviderKeys().includes(providerFromUrl)) {
      filterProvider = providerFromUrl;
    }
    const statusFromUrl = params.get('status');
    if (statusFromUrl && BATCH_STATUS_OPTIONS.includes(statusFromUrl as (typeof BATCH_STATUS_OPTIONS)[number])) {
      filterStatus = statusFromUrl;
    }
    const groupFromUrl = params.get('group_id');
    if (groupFromUrl && /^\d+$/.test(groupFromUrl)) {
      filterGroupId = groupFromUrl;
    }
  }

  onMount(() => {
    const token = localStorage.getItem('token');
    if (token) api.setToken(token);
    initFiltersFromUrl();
    loadAccounts();
    loadGroups();
    loadProviders();
  });

  function getProviderKeys() {
    return providers.length > 0 ? providers.map((provider) => provider.key) : DEFAULT_PROVIDER_KEYS;
  }

  async function loadProviders() {
    try {
      const res = await api.listAccountProviders();
      providers = res.providers.filter((provider) => provider.key !== 'google');
      if (!providers.some((provider) => provider.key === formProvider) && providers.length > 0) {
        formProvider = providers[0].key;
      }
    } catch {
      providers = DEFAULT_PROVIDER_KEYS.map((key) => ({
        key,
        display_name: key,
        base_url: '',
        auth_header: '',
        requires_version_header: false
      }));
    }
  }

  function showToast(message: string, type: 'success' | 'error') {
    if (toastTimeout) clearTimeout(toastTimeout);
    toast = { message, type };
    toastTimeout = setTimeout(() => { toast = null; }, 3500);
  }

  async function loadAccounts() {
    loading = true;
    error = null;
    syncUrlFromFilters();
    try {
      const params: Record<string, unknown> = { page, per_page: 20 };
      if (searchQuery.trim()) params.search = searchQuery.trim();
      if (filterProvider) params.provider = filterProvider;
      if (filterStatus) params.status = filterStatus;
      if (filterGroupId) params.group_id = Number(filterGroupId);
      const res: PaginatedResponse<Account> = await api.listAccounts(params);
      accounts = res.data;
      total = res.total;
      totalPages = res.total_pages;
      if (page > res.total_pages && res.total_pages > 0) {
        page = res.total_pages;
        return await loadAccounts();
      }
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load accounts';
    } finally {
      loading = false;
    }
  }

  async function loadGroups() {
    groupsLoading = true;
    groupsError = null;
    try {
      const res = await api.listAllGroups();
      groups = res.data ?? [];
    } catch (e) {
      groupsError = e instanceof Error ? e.message : 'Failed to load groups';
    } finally {
      groupsLoading = false;
    }
  }

  function onSearchInput(value: string) {
    searchQuery = value;
    if (searchTimeout) clearTimeout(searchTimeout);
    searchTimeout = setTimeout(() => {
      page = 1;
      selectedAccountIds = [];
      syncUrlFromFilters();
      loadAccounts();
    }, 350);
  }

  function onFilterChange() {
    page = 1;
    selectedAccountIds = [];
    batchUseFilterScope = false;
    syncUrlFromFilters();
    loadAccounts();
  }

  function getBatchScopeFilters() {
    const payload: {
      filter_status?: string;
      filter_provider?: string;
      filter_search?: string;
      filter_group_id?: number;
    } = {};
    if (filterStatus) payload.filter_status = filterStatus;
    if (filterProvider) payload.filter_provider = filterProvider;
    if (searchQuery.trim()) payload.filter_search = searchQuery.trim();
    if (filterGroupId !== '') payload.filter_group_id = Number(filterGroupId);
    return Object.keys(payload).length > 0 ? payload : null;
  }

  function confirmBatchFilterScope(totalScopeCount: number) {
    if (totalScopeCount < BATCH_SCOPE_CONFIRM_THRESHOLD) {
      return true;
    }
    const ok = window.confirm(
      `当前筛选范围有 ${totalScopeCount} 条账号，确认要执行批量变更吗？\n` +
      `此操作将影响所有当前筛选结果，且不受已勾选列表影响。`
    );
    return ok;
  }

  function selectAllCurrentPage(checked: boolean) {
    const ids = accounts.map((account) => account.id);
    if (checked) {
      const existing = new Set(selectedAccountIds);
      ids.forEach((id) => existing.add(id));
      selectedAccountIds = Array.from(existing);
    } else {
      const idSet = new Set(ids);
      selectedAccountIds = selectedAccountIds.filter((id) => !idSet.has(id));
    }
  }

  function toggleAccountSelection(id: string, checked: boolean) {
    if (checked) {
      if (!selectedAccountIds.includes(id)) {
        selectedAccountIds = [...selectedAccountIds, id];
      }
    } else {
      selectedAccountIds = selectedAccountIds.filter((item) => item !== id);
    }
  }

  function clearSelection() {
    selectedAccountIds = [];
  }

  async function batchSetStatus() {
    const scopeFilters = batchUseFilterScope ? getBatchScopeFilters() : null;
    if (!batchUseFilterScope && selectedAccountIds.length === 0) {
      showToast('请先选择要处理的账号', 'error');
      return;
    }
    if (batchUseFilterScope && scopeFilters === null) {
      showToast('当前筛选为空，无法按筛选执行', 'error');
      return;
    }
    if (batchUseFilterScope && !confirmBatchFilterScope(total)) {
      return;
    }

    const targetIds = batchUseFilterScope ? null : selectedAccountIds;

    batchSubmitting = true;
    try {
      const res = await api.batchSetAccountStatus(targetIds, batchStatus, batchClearError, scopeFilters ?? undefined);
      const parts = `已处理 ${res.total} 个，成功 ${res.succeeded} 个，失败 ${res.failed} 个`;
      if (res.failed > 0) {
        showToast(`批量状态变更完成：${parts}`, 'error');
      } else {
        showToast(`批量状态变更完成：${parts}`, 'success');
      }
      if (res.errors.length > 0) {
        showToast(`失败原因示例：${res.errors.slice(0, 3).join('；')}`, 'error');
      }
      if (res.succeeded > 0) loadAccounts();
      if (res.failed === 0) {
        clearSelection();
      }
    } catch (e) {
      showToast(e instanceof Error ? e.message : 'Batch status update failed', 'error');
    } finally {
      batchSubmitting = false;
    }
  }

  async function batchSetGroup() {
    const scopeFilters = batchUseFilterScope ? getBatchScopeFilters() : null;
    if (!batchUseFilterScope && selectedAccountIds.length === 0) {
      showToast('请先选择要处理的账号', 'error');
      return;
    }
    if (batchUseFilterScope && scopeFilters === null) {
      showToast('当前筛选为空，无法按筛选执行', 'error');
      return;
    }
    if (batchUseFilterScope && !confirmBatchFilterScope(total)) {
      return;
    }
    if (batchGroupId.length === 0) {
      showToast('请先选择要设置的分组', 'error');
      return;
    }
    let groupId: number | null = null;
    if (batchGroupId === '__clear__') {
      groupId = null;
    } else if (batchGroupId.length > 0) {
      const parsed = Number(batchGroupId);
      if (!Number.isFinite(parsed) || !Number.isInteger(parsed) || parsed < 0) {
        showToast('分组 ID 需为非负整数或留空', 'error');
        return;
      }
      groupId = parsed;
    }

    batchSubmitting = true;
    try {
      const targetIds = batchUseFilterScope ? null : selectedAccountIds;
      const res = await api.batchSetAccountGroup(targetIds, groupId, scopeFilters ?? undefined);
      const msg =
        groupId === null
          ? `分组清空完成：成功 ${res.succeeded}/${res.total}，失败 ${res.failed}`
          : `分组设置完成：成功 ${res.succeeded}/${res.total}，失败 ${res.failed}`;
      if (res.failed > 0) {
        showToast(msg, 'error');
      } else {
        showToast(msg, 'success');
      }
      if (res.errors.length > 0) {
        showToast(`失败原因示例：${res.errors.slice(0, 3).join('；')}`, 'error');
      }
      if (res.succeeded > 0) {
        loadAccounts();
      }
      if (res.failed === 0) {
        clearSelection();
      }
    } catch (e) {
      showToast(e instanceof Error ? e.message : 'Batch group update failed', 'error');
    } finally {
      batchSubmitting = false;
    }
  }

  async function batchClearRateLimit() {
    const scopeFilters = batchUseFilterScope ? getBatchScopeFilters() : null;
    if (!batchUseFilterScope && selectedAccountIds.length === 0) {
      showToast('请先选择要处理的账号', 'error');
      return;
    }
    if (batchUseFilterScope && scopeFilters === null) {
      showToast('当前筛选为空，无法按筛选执行', 'error');
      return;
    }
    if (batchUseFilterScope && !confirmBatchFilterScope(total)) {
      return;
    }

    const targetIds = batchUseFilterScope ? null : selectedAccountIds;

    batchSubmitting = true;
    try {
      const res = await api.batchClearRateLimits(targetIds, scopeFilters ?? undefined);
      const msg = `清理限流完成：清理 key ${res.deleted_keys}，处理 ${res.processed}，缺失 ${res.missing}，非法 ${res.invalid}`;
      if (res.processed === 0 && res.deleted_keys === 0 && res.missing === 0) {
        showToast('清理完成：无可清理记录', 'success');
      } else if (res.invalid > 0 || res.missing > 0 || res.processed < selectedAccountIds.length) {
        showToast(msg, 'error');
      } else {
        showToast(msg, 'success');
      }
      loadAccounts();
      if (res.invalid === 0 && res.missing === 0) {
        clearSelection();
      }
    } catch (e) {
      showToast(e instanceof Error ? e.message : 'Batch clear rate limits failed', 'error');
    } finally {
      batchSubmitting = false;
    }
  }

  function goPage(p: number) {
    if (p < 1 || p > totalPages) return;
    page = p;
    syncUrlFromFilters();
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

  function renderGroupName(groupId: number | null | undefined): string {
    if (groupId === null || groupId === undefined) return '未分组';
    const matched = groups.find((group) => group.id === groupId);
    return matched ? matched.name : `Group #${groupId}`;
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
    importPreview = null;
    importPreviewInput = '';
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
      const useBulkImport = parsed.length > 100;
      const normalizedInput = JSON.stringify(parsed);
      let succeeded = 0;
      let failed = 0;
      let skipped = 0;

      if (useBulkImport) {
        if (importPreviewInput !== normalizedInput) {
          const previewRes = await api.previewFastImportAccounts(parsed, {
            batch_size: 1000,
            validation_concurrency: 64,
            skip_duplicates: true,
            fast_mode: false
          });
          importPreview = previewRes.preview;
          importPreviewInput = normalizedInput;
          showToast(
            `预检完成：预计导入 ${previewRes.preview.will_import} 个，重复 ${previewRes.preview.duplicate} 个。再次点击“导入”确认执行。`,
            previewRes.preview.invalid > 0 ? 'error' : 'success'
          );
          importSubmitting = false;
          return;
        }

        const res = await api.fastImportAccounts(parsed, {
          batch_size: 1000,
          validation_concurrency: 64,
          skip_duplicates: true,
          fast_mode: false
        });
        succeeded = res.imported;
        failed = res.failed;
        skipped = res.skipped;
        importResult = {
          succeeded,
          failed,
          skipped,
          errors: res.errors
            .map((item) => item.error ?? '')
            .filter((item) => item.length > 0)
        };
        importPreview = null;
      } else {
        const res = await api.batchCreateAccounts(parsed);
        succeeded = res.succeeded;
        failed = res.failed;
        importResult = { succeeded, failed, skipped, errors: res.errors };
      }

      if (succeeded > 0) {
        showToast(`成功导入 ${succeeded} 个渠道`, 'success');
        loadAccounts();
      }
      if (skipped > 0) {
        showToast(`${skipped} 个重复渠道已跳过`, 'success');
      }
      if (failed > 0) {
        showToast(`${failed} 个渠道导入失败`, 'error');
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

  function handleWindowKeydown(e: KeyboardEvent) {
    if (e.key !== 'Escape') return;
    if (showImportModal) return closeImportModal();
    if (showDeleteConfirm) return closeDeleteConfirm();
    if (showEditModal) return closeEditModal();
    if (showCreateModal) return closeCreateModal();
  }

  function closeCreateModal() { showCreateModal = false; }
  function closeEditModal() { showEditModal = false; editTarget = null; }
  function closeDeleteConfirm() { showDeleteConfirm = false; deleteTarget = null; }
  function closeImportModal() {
    showImportModal = false;
    importResult = null;
    importPreview = null;
    importPreviewInput = '';
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
        {#each getProviderKeys() as p}
          <option value={p}>{providers.find((provider) => provider.key === p)?.display_name ?? (p.charAt(0).toUpperCase() + p.slice(1))}</option>
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
      <select
        id="filter-group"
        value={filterGroupId}
        onchange={(e) => { filterGroupId = e.currentTarget.value; onFilterChange(); }}
        class="rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-700 transition-colors focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-200"
        aria-label="Filter by group"
      >
        <option value="">All Groups</option>
        {#if groupsLoading}
          <option value="" disabled>分组列表加载中...</option>
        {:else if groupsError}
          <option value="" disabled>{groupsError}</option>
        {:else}
          {#each groups as group}
            <option value={String(group.id)}>{group.name}</option>
          {/each}
        {/if}
      </select>
    </div>
  </div>

  {#if selectedCount > 0 || batchHasFilterScope}
    <div class="rounded-lg border border-blue-200 bg-blue-50 p-3 dark:border-blue-800 dark:bg-blue-900/20">
      <div class="flex flex-col gap-3">
        {#if batchHasFilterScope}
          <p class="text-sm font-medium text-blue-900 dark:text-blue-100">
            当前筛选 {total} 条（{selectedCount} 条已勾选）
          </p>
          {#if batchScopeHighRisk}
            <p class="rounded-md border border-amber-200 bg-amber-50 p-2 text-xs text-amber-900 dark:border-amber-800 dark:bg-amber-900/30 dark:text-amber-200">
              当前筛选命中 {total} 条，操作将按筛选范围全部执行。建议先缩小条件或先导出核验。
            </p>
          {/if}
        {:else}
          <p class="text-sm font-medium text-blue-900 dark:text-blue-100">
            已选择 {selectedCount} 个账号
          </p>
        {/if}
        <div class="grid gap-3 md:grid-cols-[1fr_1fr_220px_auto] md:items-end">
          <label class="col-span-full flex items-center gap-2 text-sm text-blue-700 dark:text-blue-200">
            <input
              type="checkbox"
              bind:checked={batchUseFilterScope}
              class="rounded border-blue-300 text-blue-600 focus:ring-blue-500 dark:border-blue-900 dark:bg-gray-700"
              disabled={!batchHasFilterScope || batchSubmitting}
            />
            <span>按当前筛选范围执行（忽略已勾选）</span>
          </label>
          <div>
            <label for="batch-status" class="mb-1 block text-xs text-blue-700 dark:text-blue-200">批量改状态</label>
            <div class="flex gap-2">
              <select
                id="batch-status"
                bind:value={batchStatus}
                class="rounded-md border border-blue-200 bg-white px-2 py-2 text-sm dark:border-blue-900 dark:bg-gray-700"
              >
                {#each BATCH_STATUS_OPTIONS as status}
                  <option value={status}>{status}</option>
                {/each}
              </select>
              <label class="flex items-center gap-1.5 text-xs text-blue-700 dark:text-blue-200">
                <input
                  type="checkbox"
                  bind:checked={batchClearError}
                  class="rounded border-blue-300 text-blue-600 focus:ring-blue-500 dark:border-blue-900 dark:bg-gray-700"
                />
                同时清除错误
              </label>
            </div>
          </div>
          <div>
            <label for="batch-group-id" class="mb-1 block text-xs text-blue-700 dark:text-blue-200">批量设置分组</label>
            <div class="flex gap-2">
              <select
                id="batch-group-id"
                bind:value={batchGroupId}
                class="w-full rounded-md border border-blue-200 bg-white px-2 py-2 text-sm dark:border-blue-900 dark:bg-gray-700"
                aria-label="Select target group for batch operation"
              >
                <option value="" disabled={groups.length > 0}>选择分组...</option>
                <option value="__clear__">清空分组</option>
                {#if groupsLoading}
                  <option value="__loading__" disabled>加载分组中...</option>
                {:else if groupsError}
                  <option value="__error__" disabled>{groupsError}</option>
                {:else}
                  {#each groups as group}
                    <option value={String(group.id)}>{group.name} ({group.platform}) [ID:{group.id}]</option>
                  {/each}
                {/if}
              </select>
              <button
                onclick={batchSetGroup}
                disabled={batchSubmitting}
                class="rounded-md bg-indigo-600 px-3 py-2 text-sm font-medium text-white transition-colors hover:bg-indigo-700 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {batchSubmitting ? '执行中...' : '设置分组'}
              </button>
            </div>
          </div>
          <div class="flex gap-2">
            <button
              onclick={() => batchSetStatus()}
              disabled={batchSubmitting}
              class="rounded-md bg-green-600 px-3 py-2 text-sm font-medium text-white transition-colors hover:bg-green-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {batchSubmitting ? '执行中...' : '应用状态'}
            </button>
            <button
              onclick={batchClearRateLimit}
              disabled={batchSubmitting}
              class="rounded-md bg-gray-700 px-3 py-2 text-sm font-medium text-white transition-colors hover:bg-gray-800 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {batchSubmitting ? '执行中...' : '清除限流'}
            </button>
          </div>
          <div class="justify-self-end">
            <button
              onclick={clearSelection}
              class="rounded-md border border-blue-200 bg-white px-3 py-2 text-sm font-medium text-blue-700 transition-colors hover:bg-white/80 dark:border-blue-800 dark:bg-gray-800 dark:text-blue-100 dark:hover:bg-gray-700"
            >
              清空选择
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}

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
            <th scope="col" class="px-5 py-3">
              <input
                type="checkbox"
                checked={currentPageAllSelected}
                onchange={(e) => selectAllCurrentPage((e.currentTarget as HTMLInputElement).checked)}
                class="rounded border-gray-300 text-blue-600 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:focus:ring-blue-400"
                aria-label="Select all accounts on current page"
              />
            </th>
            <th scope="col" class="px-5 py-3">Name</th>
            <th scope="col" class="px-5 py-3">Platform</th>
            <th scope="col" class="px-5 py-3">Credential</th>
            <th scope="col" class="px-5 py-3">Group</th>
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
                <input
                  type="checkbox"
                  checked={selectedAccountIds.includes(account.id)}
                  onchange={(e) => toggleAccountSelection(account.id, (e.currentTarget as HTMLInputElement).checked)}
                  class="rounded border-gray-300 text-blue-600 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:focus:ring-blue-400"
                  aria-label={`Select account ${account.name}`}
                />
              </td>
              <td class="whitespace-nowrap px-5 py-3.5">
                <span class="font-medium text-gray-900 dark:text-white">{account.name}</span>
              </td>
              <td class="whitespace-nowrap px-5 py-3.5">
                <span class="inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium {platformColor(account.provider)}">{account.provider}</span>
              </td>
              <td class="whitespace-nowrap px-5 py-3.5 text-gray-500 dark:text-gray-400">{account.credential_type}</td>
              <td class="whitespace-nowrap px-5 py-3.5 text-gray-600 dark:text-gray-300">{renderGroupName(account.group_id)}</td>
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
          <label class="mb-2 flex items-center gap-2 text-sm text-gray-700 dark:text-gray-200">
            <input
              type="checkbox"
              checked={selectedAccountIds.includes(account.id)}
              onchange={(e) => toggleAccountSelection(account.id, (e.currentTarget as HTMLInputElement).checked)}
              class="rounded border-gray-300 text-blue-600 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700"
            />
            <span>选择</span>
          </label>
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
          <div class="mt-1 text-xs text-gray-500 dark:text-gray-400">Group: {renderGroupName(account.group_id)}</div>
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
  <div class="fixed inset-0 z-40 flex items-center justify-center p-4" role="dialog" aria-modal="true" aria-labelledby="create-modal-title">
    <button type="button" class="absolute inset-0 bg-black/50" aria-label="关闭添加渠道弹窗" onclick={closeCreateModal}></button>
    <div class="relative w-full max-w-lg rounded-xl bg-white p-6 shadow-xl dark:bg-gray-800" role="document">
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
              {#each getProviderKeys() as p}
                <option value={p}>{providers.find((provider) => provider.key === p)?.display_name ?? (p.charAt(0).toUpperCase() + p.slice(1))}</option>
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
  <div class="fixed inset-0 z-40 flex items-center justify-center p-4" role="dialog" aria-modal="true" aria-labelledby="edit-modal-title">
    <button type="button" class="absolute inset-0 bg-black/50" aria-label="关闭编辑渠道弹窗" onclick={closeEditModal}></button>
    <div class="relative w-full max-w-lg rounded-xl bg-white p-6 shadow-xl dark:bg-gray-800" role="document">
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
              {#each getProviderKeys() as p}
                <option value={p}>{providers.find((provider) => provider.key === p)?.display_name ?? (p.charAt(0).toUpperCase() + p.slice(1))}</option>
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
  <div class="fixed inset-0 z-40 flex items-center justify-center p-4" role="dialog" aria-modal="true" aria-labelledby="delete-modal-title">
    <button type="button" class="absolute inset-0 bg-black/50" aria-label="关闭删除渠道弹窗" onclick={closeDeleteConfirm}></button>
    <div class="relative w-full max-w-sm rounded-xl bg-white p-6 shadow-xl dark:bg-gray-800" role="document">
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
  <div class="fixed inset-0 z-40 flex items-center justify-center p-4" role="dialog" aria-modal="true" aria-labelledby="import-modal-title">
    <button type="button" class="absolute inset-0 bg-black/50" aria-label="关闭批量导入弹窗" onclick={closeImportModal}></button>
    <div class="relative w-full max-w-xl rounded-xl bg-white p-6 shadow-xl dark:bg-gray-800" role="document">
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
            oninput={() => {
              importPreview = null;
              importPreviewInput = '';
            }}
            class="mt-1 w-full rounded-lg border border-gray-300 bg-white px-3 py-2 font-mono text-xs text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
            placeholder={`[\n  {\n    "name": "OpenAI Main",\n    "provider": "openai",\n    "credential_type": "api_key",\n    "credential": "sk-..."\n  },\n  {\n    "name": "Claude Pro",\n    "provider": "anthropic",\n    "credential_type": "api_key",\n    "credential": "sk-ant-..."\n  }\n]`}
          ></textarea>
        </div>
        {#if importPreview}
          <div class="rounded-lg border border-blue-200 bg-blue-50 p-3 text-sm dark:border-blue-800 dark:bg-blue-900/20">
            <p class="font-medium text-blue-800 dark:text-blue-200">
              预检完成：预计导入 {importPreview.will_import} / {importPreview.total}，重复 {importPreview.duplicate}，校验失败 {importPreview.invalid}
            </p>
            <p class="mt-1 text-xs text-blue-700 dark:text-blue-300">
              耗时 {importPreview.duration_ms}ms。再次点击“导入”将按当前 JSON 真正执行。
            </p>
            {#if importPreview.providers.length > 0}
              <div class="mt-2 overflow-x-auto">
                <table class="min-w-full text-xs text-blue-900 dark:text-blue-100">
                  <thead>
                    <tr class="text-left">
                      <th class="pr-4">Provider</th>
                      <th class="pr-4">Total</th>
                      <th class="pr-4">Will Import</th>
                      <th class="pr-4">Duplicate</th>
                      <th class="pr-4">Invalid</th>
                    </tr>
                  </thead>
                  <tbody>
                    {#each importPreview.providers as provider}
                      <tr>
                        <td class="pr-4 py-1">{provider.provider}</td>
                        <td class="pr-4 py-1">{provider.total}</td>
                        <td class="pr-4 py-1">{provider.will_import}</td>
                        <td class="pr-4 py-1">{provider.duplicate}</td>
                        <td class="pr-4 py-1">{provider.invalid}</td>
                      </tr>
                    {/each}
                  </tbody>
                </table>
              </div>
            {/if}
            {#if importPreview.errors.length > 0}
              <ul class="mt-2 list-inside list-disc text-xs text-blue-700 dark:text-blue-300">
                {#each importPreview.errors as err}
                  <li>{err.name || `#${err.index ?? '-'}`}: {err.error ?? 'Invalid item'}</li>
                {/each}
              </ul>
            {/if}
          </div>
        {/if}
        {#if importResult}
          <div class="rounded-lg border p-3 text-sm {importResult.failed > 0 ? 'border-yellow-200 bg-yellow-50 dark:border-yellow-800 dark:bg-yellow-900/20' : 'border-green-200 bg-green-50 dark:border-green-800 dark:bg-green-900/20'}">
            <p class="font-medium {importResult.failed > 0 ? 'text-yellow-800 dark:text-yellow-200' : 'text-green-800 dark:text-green-200'}">
              Import complete: {importResult.succeeded} succeeded, {importResult.skipped} skipped, {importResult.failed} failed
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
          <button type="submit" disabled={importSubmitting} class="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-700 disabled:opacity-50">{importSubmitting ? '导入中...' : importPreview ? '确认导入' : '导入'}</button>
        </div>
      </form>
    </div>
  </div>
{/if}
