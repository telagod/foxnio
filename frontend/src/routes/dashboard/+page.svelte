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

  let user = $state<User>({
    id: '',
    email: '',
    balance: 0,
    role: '',
    status: '',
    created_at: ''
  });
  let apiKeys = $state<ApiKey[]>([]);
  let usageStats = $state<DashboardUsageStats>({
    total_requests: 0,
    total_input_tokens: 0,
    total_output_tokens: 0,
    total_cost: 0,
    total_cost_yuan: 0
  });
  let loading = $state(true);
  let creating = $state(false);
  let deletingId = $state<string | null>(null);
  let error = $state('');

  onMount(async () => {
    const token = localStorage.getItem('token');
    if (!token) {
      await goto('/login');
      return;
    }

    api.setToken(token);

    try {
      await loadDashboard();
    } catch (e) {
      console.error('Failed to load dashboard:', e);
      localStorage.removeItem('token');
      await goto('/login');
    } finally {
      loading = false;
    }
  });

  async function loadDashboard() {
    const [me, apiKeyResponse, usage] = await Promise.all([
      api.getMe(),
      api.listApiKeys(),
      api.getUserUsage()
    ]);

    user = me;
    apiKeys = apiKeyResponse.data || [];
    usageStats = {
      total_requests: usage.total_requests || 0,
      total_input_tokens: (usage as any).total_input_tokens || 0,
      total_output_tokens: (usage as any).total_output_tokens || 0,
      total_cost: usage.total_cost || 0,
      total_cost_yuan: (usage as any).total_cost_yuan || (usage.total_cost || 0) / 100
    };
    error = '';
  }

  async function createApiKey() {
    const rawName = window.prompt('输入 API Key 名称（可选）');
    if (rawName === null) return;
    const name = rawName.trim();

    creating = true;
    try {
      await api.createApiKey(name || undefined);
      await loadDashboard();
    } catch (e) {
      error = e instanceof Error ? e.message : '创建 API Key 失败';
    } finally {
      creating = false;
    }
  }

  async function deleteApiKey(id: string) {
    if (!window.confirm('确认删除这个 API Key？')) return;

    deletingId = id;
    try {
      await api.deleteApiKey(id);
      await loadDashboard();
    } catch (e) {
      error = e instanceof Error ? e.message : '删除 API Key 失败';
    } finally {
      deletingId = null;
    }
  }

  function logout() {
    localStorage.removeItem('token');
    api.setToken('');
    goto('/login');
  }

  function formatDate(value: string): string {
    if (!value) return '-';
    return new Date(value).toLocaleString('zh-CN');
  }

  function formatTokens(value: number): string {
    if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
    if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
    return `${value}`;
  }
</script>

<svelte:head>
  <title>仪表盘 - FoxNIO</title>
</svelte:head>

{#if loading}
  <div class="flex items-center justify-center min-h-[60vh]">
    <div class="animate-pulse-soft text-2xl">🦊</div>
  </div>
{:else}
  <div class="animate-fade-in space-y-8">
    <section class="glass-card p-6">
      <div class="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
        <div>
          <h1 class="text-2xl font-bold">欢迎回来</h1>
          <p class="mt-1 text-light-muted dark:text-dark-muted">{user.email}</p>
          <p class="mt-2 text-sm text-light-muted dark:text-dark-muted">
            角色：{user.role || 'user'} · 状态：{user.status || 'unknown'}
          </p>
        </div>
        <button onclick={logout} class="glass-button text-sm">退出登录</button>
      </div>
      {#if error}
        <div class="mt-4 rounded-xl border border-red-500/20 bg-red-500/10 px-4 py-3 text-sm text-red-600 dark:text-red-300">
          {error}
        </div>
      {/if}
    </section>

    <section class="grid grid-cols-1 gap-4 md:grid-cols-3">
      <div class="glass-card p-6">
        <div class="mb-2 text-sm text-light-muted dark:text-dark-muted">账户余额</div>
        <div class="text-3xl font-bold text-brand-primary">¥{(user.balance / 100).toFixed(2)}</div>
      </div>

      <div class="glass-card p-6">
        <div class="mb-2 text-sm text-light-muted dark:text-dark-muted">API Keys</div>
        <div class="text-3xl font-bold">{apiKeys.length}</div>
      </div>

      <div class="glass-card p-6">
        <div class="mb-2 text-sm text-light-muted dark:text-dark-muted">近 30 天请求</div>
        <div class="text-3xl font-bold">{formatTokens(usageStats.total_requests)}</div>
      </div>
    </section>

    <section class="grid grid-cols-1 gap-4 md:grid-cols-2">
      <div class="glass-card p-6">
        <div class="mb-2 text-sm text-light-muted dark:text-dark-muted">输入 Token</div>
        <div class="text-3xl font-bold">{formatTokens(usageStats.total_input_tokens)}</div>
      </div>

      <div class="glass-card p-6">
        <div class="mb-2 text-sm text-light-muted dark:text-dark-muted">输出 Token</div>
        <div class="text-3xl font-bold">{formatTokens(usageStats.total_output_tokens)}</div>
      </div>
    </section>

    <section class="glass-card p-6">
      <div class="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
        <div>
          <h2 class="text-xl font-semibold">API Keys</h2>
          <p class="mt-1 text-sm text-light-muted dark:text-dark-muted">
            当前近 30 天费用：¥{usageStats.total_cost_yuan.toFixed(2)}
          </p>
        </div>
        <button
          onclick={createApiKey}
          class="glass-button bg-brand-primary/20"
          disabled={creating}
        >
          {creating ? '创建中...' : '+ 创建密钥'}
        </button>
      </div>

      {#if apiKeys.length === 0}
        <div class="py-8 text-center text-light-muted dark:text-dark-muted">
          暂无 API Key，点击上方按钮创建
        </div>
      {:else}
        <div class="mt-4 space-y-2">
          {#each apiKeys as key}
            <div class="glass-card flex flex-col gap-3 bg-white/30 p-4 dark:bg-black/10 md:flex-row md:items-center md:justify-between">
              <div>
                <div class="font-mono text-sm">{key.key}</div>
                <div class="mt-1 text-xs text-light-muted dark:text-dark-muted">
                  {(key.name || '未命名密钥')} · 创建于 {formatDate(key.created_at)}
                </div>
                <div class="mt-1 text-xs text-light-muted dark:text-dark-muted">
                  状态：{key.status} · 最后使用：{formatDate(key.last_used_at || '')}
                </div>
              </div>
              <button
                onclick={() => deleteApiKey(key.id)}
                class="text-sm text-red-500 hover:text-red-400"
                disabled={deletingId === key.id}
              >
                {deletingId === key.id ? '删除中...' : '删除'}
              </button>
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <section class="glass-card p-6">
      <h2 class="mb-4 text-xl font-semibold">快速开始</h2>
      <div class="overflow-x-auto rounded-xl bg-black/5 p-4 font-mono text-sm dark:bg-white/5">
        <pre class="text-light-muted dark:text-dark-muted">{`curl https://api.foxnio.io/v1/chat/completions \\
  -H "Authorization: Bearer YOUR_API_KEY" \\
  -H "Content-Type: application/json" \\
  -d '{"model": "gpt-4-turbo", "messages": [{"role": "user", "content": "Hello!"}]}'`}</pre>
      </div>
    </section>
  </div>
{/if}
