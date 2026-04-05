<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import { api, type ApiKey, type User } from '$lib/api';

  interface DashboardUsageStats {
    total_requests: number;
    total_input_tokens: number;
    total_output_tokens: number;
    total_cost: number;
    total_cost_yuan: number;
  }

  let user = $state<User>({ id: '', email: '', balance: 0, role: '', status: '', created_at: '' });
  let apiKeys = $state<ApiKey[]>([]);
  let usageStats = $state<DashboardUsageStats>({
    total_requests: 0, total_input_tokens: 0, total_output_tokens: 0, total_cost: 0, total_cost_yuan: 0
  });
  let loading = $state(true);
  let creating = $state(false);
  let deletingId = $state<string | null>(null);
  let error = $state('');

  onMount(async () => {
    const token = localStorage.getItem('token');
    if (!token) { await goto('/login'); return; }
    api.setToken(token);
    try { await loadDashboard(); }
    catch (e) { console.error('Failed to load dashboard:', e); localStorage.removeItem('token'); await goto('/login'); }
    finally { loading = false; }
  });

  async function loadDashboard() {
    const [me, apiKeyResponse, usage] = await Promise.all([api.getMe(), api.listApiKeys(), api.getUserUsage()]);
    user = me;
    apiKeys = apiKeyResponse.data || [];
    usageStats = {
      total_requests: usage.total_requests || 0,
      total_input_tokens: usage.total_input_tokens || 0,
      total_output_tokens: usage.total_output_tokens || 0,
      total_cost: usage.total_cost || 0,
      total_cost_yuan: usage.total_cost_yuan || (usage.total_cost || 0) / 100
    };
    error = '';
  }

  async function createApiKey() {
    const rawName = window.prompt('输入 API Key 名称（可选）');
    if (rawName === null) return;
    creating = true;
    try { await api.createApiKey(rawName.trim() || undefined); await loadDashboard(); }
    catch (e) { error = e instanceof Error ? e.message : '创建 API Key 失败'; }
    finally { creating = false; }
  }

  async function deleteApiKey(id: string) {
    if (!window.confirm('确认删除这个 API Key？')) return;
    deletingId = id;
    try { await api.deleteApiKey(id); await loadDashboard(); }
    catch (e) { error = e instanceof Error ? e.message : '删除 API Key 失败'; }
    finally { deletingId = null; }
  }

  function logout() { localStorage.removeItem('token'); api.setToken(''); goto('/login'); }
  function formatDate(value: string): string { if (!value) return '-'; return new Date(value).toLocaleString('zh-CN'); }
  function formatTokens(value: number): string {
    if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
    if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
    return `${value}`;
  }
</script>

<svelte:head><title>仪表盘 - FoxNIO</title></svelte:head>

{#if loading}
  <div class="flex items-center justify-center min-h-[60vh]" role="status" aria-label="Loading dashboard">
    <svg class="h-10 w-10 animate-spin text-brand-primary" viewBox="0 0 24 24" fill="none">
      <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3"></circle>
      <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
    </svg>
  </div>
{:else}
  <div class="animate-fade-in space-y-8">
    <!-- Header -->
    <section class="glass-card p-6">
      <div class="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
        <div>
          <h1 class="text-2xl font-bold text-gray-900 dark:text-white">欢迎回来</h1>
          <p class="mt-1 text-light-muted dark:text-dark-muted">{user.email}</p>
          <p class="mt-2 text-sm text-light-muted dark:text-dark-muted">
            角色：{user.role || 'user'} · 状态：{user.status || 'unknown'}
          </p>
        </div>
        <button onclick={logout} class="glass-button text-sm text-gray-700 dark:text-gray-300" aria-label="退出登录">
          <span class="inline-flex items-center gap-2">
            <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 21H5a2 2 0 01-2-2V5a2 2 0 012-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" y1="12" x2="9" y2="12"/></svg>
            退出登录
          </span>
        </button>
      </div>
      {#if error}
        <div class="mt-4 flex items-start gap-3 rounded-xl border border-red-500/20 bg-red-500/10 px-4 py-3 text-sm text-red-600 dark:text-red-300" role="alert">
          <svg class="mt-0.5 h-4 w-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
          {error}
        </div>
      {/if}
    </section>

    <!-- Stats row 1 -->
    <section class="grid grid-cols-1 gap-4 sm:grid-cols-3">
      <div class="glass-card p-6">
        <div class="flex items-center gap-3">
          <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-brand-primary/10">
            <svg class="h-5 w-5 text-brand-primary" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2v20M17 5H9.5a3.5 3.5 0 000 7h5a3.5 3.5 0 010 7H6"/></svg>
          </div>
          <div>
            <div class="text-sm text-light-muted dark:text-dark-muted">账户余额</div>
            <div class="text-2xl font-bold text-brand-primary">¥{(user.balance / 100).toFixed(2)}</div>
          </div>
        </div>
      </div>
      <div class="glass-card p-6">
        <div class="flex items-center gap-3">
          <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-brand-secondary/10">
            <svg class="h-5 w-5 text-brand-secondary" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 11-7.778 7.778 5.5 5.5 0 017.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4"/></svg>
          </div>
          <div>
            <div class="text-sm text-light-muted dark:text-dark-muted">API Keys</div>
            <div class="text-2xl font-bold text-gray-900 dark:text-white">{apiKeys.length}</div>
          </div>
        </div>
      </div>
      <div class="glass-card p-6">
        <div class="flex items-center gap-3">
          <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-brand-accent/10">
            <svg class="h-5 w-5 text-brand-accent" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
          </div>
          <div>
            <div class="text-sm text-light-muted dark:text-dark-muted">近 30 天请求</div>
            <div class="text-2xl font-bold text-gray-900 dark:text-white">{formatTokens(usageStats.total_requests)}</div>
          </div>
        </div>
      </div>
    </section>

    <!-- Stats row 2 -->
    <section class="grid grid-cols-1 gap-4 sm:grid-cols-2">
      <div class="glass-card p-6">
        <div class="flex items-center gap-3">
          <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-blue-500/10">
            <svg class="h-5 w-5 text-blue-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 12h16M4 6h16M4 18h10"/></svg>
          </div>
          <div>
            <div class="text-sm text-light-muted dark:text-dark-muted">输入 Token</div>
            <div class="text-2xl font-bold text-gray-900 dark:text-white">{formatTokens(usageStats.total_input_tokens)}</div>
          </div>
        </div>
      </div>
      <div class="glass-card p-6">
        <div class="flex items-center gap-3">
          <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-emerald-500/10">
            <svg class="h-5 w-5 text-emerald-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 12h16M4 6h16M4 18h10"/></svg>
          </div>
          <div>
            <div class="text-sm text-light-muted dark:text-dark-muted">输出 Token</div>
            <div class="text-2xl font-bold text-gray-900 dark:text-white">{formatTokens(usageStats.total_output_tokens)}</div>
          </div>
        </div>
      </div>
    </section>

    <!-- API Keys section -->
    <section class="glass-card p-6">
      <div class="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
        <div>
          <h2 class="text-xl font-semibold text-gray-900 dark:text-white">API Keys</h2>
          <p class="mt-1 text-sm text-light-muted dark:text-dark-muted">
            当前近 30 天费用：¥{usageStats.total_cost_yuan.toFixed(2)}
          </p>
        </div>
        <button
          onclick={createApiKey}
          class="glass-button inline-flex items-center gap-2 bg-brand-primary/20 text-gray-900 dark:text-white"
          disabled={creating}
          aria-label="创建新 API Key"
        >
          {#if creating}
            <svg class="h-4 w-4 animate-spin" viewBox="0 0 24 24" fill="none"><circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3"></circle><path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path></svg>
            创建中...
          {:else}
            <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
            创建密钥
          {/if}
        </button>
      </div>

      {#if apiKeys.length === 0}
        <div class="mt-6 flex flex-col items-center justify-center py-8 text-center">
          <svg class="h-12 w-12 text-light-muted dark:text-dark-muted" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 11-7.778 7.778 5.5 5.5 0 017.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4"/></svg>
          <p class="mt-3 text-light-muted dark:text-dark-muted">暂无 API Key，点击上方按钮创建</p>
        </div>
      {:else}
        <div class="mt-4 space-y-2">
          {#each apiKeys as key (key.id)}
            <div class="glass-card flex flex-col gap-3 bg-white/30 p-4 dark:bg-black/10 md:flex-row md:items-center md:justify-between">
              <div class="min-w-0 flex-1">
                <div class="truncate font-mono text-sm text-gray-900 dark:text-white">{key.key}</div>
                <div class="mt-1 text-xs text-light-muted dark:text-dark-muted">
                  {key.name || '未命名密钥'} · 创建于 {formatDate(key.created_at)}
                </div>
                <div class="mt-1 flex items-center gap-2 text-xs text-light-muted dark:text-dark-muted">
                  <span class="inline-flex items-center gap-1">
                    <span class="h-1.5 w-1.5 rounded-full {key.status === 'active' ? 'bg-emerald-500' : 'bg-gray-400'}"></span>
                    {key.status}
                  </span>
                  <span>· 最后使用：{formatDate(key.last_used_at || '')}</span>
                </div>
              </div>
              <button
                onclick={() => deleteApiKey(key.id)}
                class="inline-flex shrink-0 items-center gap-1.5 text-sm text-red-500 transition-colors hover:text-red-400 disabled:opacity-50"
                disabled={deletingId === key.id}
                aria-label="删除 API Key {key.name || key.key}"
              >
                {#if deletingId === key.id}
                  <svg class="h-3.5 w-3.5 animate-spin" viewBox="0 0 24 24" fill="none"><circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3"></circle><path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path></svg>
                  删除中...
                {:else}
                  <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg>
                  删除
                {/if}
              </button>
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <!-- Quick start -->
    <section class="glass-card p-6">
      <h2 class="mb-4 text-xl font-semibold text-gray-900 dark:text-white">快速开始</h2>
      <div class="overflow-x-auto rounded-xl bg-black/5 p-4 font-mono text-sm dark:bg-white/5">
        <pre class="text-light-muted dark:text-dark-muted"><code>{`curl https://api.foxnio.io/v1/chat/completions \\
  -H "Authorization: Bearer YOUR_API_KEY" \\
  -H "Content-Type: application/json" \\
  -d '{"model": "gpt-4-turbo", "messages": [{"role": "user", "content": "Hello!"}]}'`}</code></pre>
      </div>
    </section>
  </div>
{/if}
