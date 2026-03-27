<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  
  let user = $state({ email: '', balance: 0 });
  let apiKeys = $state<any[]>([]);
  let usages = $state<any[]>([]);
  let loading = $state(true);
  
  onMount(async () => {
    const token = localStorage.getItem('token');
    if (!token) {
      goto('/login');
      return;
    }
    
    try {
      // TODO: 获取用户数据
      await Promise.all([
        fetchUser(token),
        fetchApiKeys(token),
        fetchUsages(token),
      ]);
    } catch (e) {
      goto('/login');
    } finally {
      loading = false;
    }
  });
  
  async function fetchUser(token: string) {
    // TODO: 实现用户数据获取
  }
  
  async function fetchApiKeys(token: string) {
    // TODO: 实现 API Key 列表获取
    apiKeys = [
      { id: '1', key: 'sk-****...****abc123', name: '默认密钥', created: '2024-03-27' },
    ];
  }
  
  async function fetchUsages(token: string) {
    // TODO: 实现用量数据获取
    usages = [];
  }
  
  async function createApiKey() {
    // TODO: 实现创建 API Key
    alert('功能开发中...');
  }
  
  function logout() {
    localStorage.removeItem('token');
    goto('/login');
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
    <!-- 欢迎区域 -->
    <section class="glass-card p-6">
      <div class="flex items-center justify-between">
        <div>
          <h1 class="text-2xl font-bold">欢迎回来 👋</h1>
          <p class="text-light-muted dark:text-dark-muted mt-1">{user.email}</p>
        </div>
        <button onclick={logout} class="glass-button text-sm">退出登录</button>
      </div>
    </section>
    
    <!-- 统计卡片 -->
    <section class="grid grid-cols-1 md:grid-cols-3 gap-4">
      <div class="glass-card p-6">
        <div class="text-light-muted dark:text-dark-muted text-sm mb-2">账户余额</div>
        <div class="text-3xl font-bold text-brand-primary">
          ¥{(user.balance / 100).toFixed(2)}
        </div>
      </div>
      
      <div class="glass-card p-6">
        <div class="text-light-muted dark:text-dark-muted text-sm mb-2">API Keys</div>
        <div class="text-3xl font-bold">{apiKeys.length}</div>
      </div>
      
      <div class="glass-card p-6">
        <div class="text-light-muted dark:text-dark-muted text-sm mb-2">本月请求</div>
        <div class="text-3xl font-bold">{usages.length}</div>
      </div>
    </section>
    
    <!-- API Keys 管理 -->
    <section class="glass-card p-6">
      <div class="flex items-center justify-between mb-4">
        <h2 class="text-xl font-semibold">API Keys</h2>
        <button onclick={createApiKey} class="glass-button bg-brand-primary/20">
          + 创建密钥
        </button>
      </div>
      
      {#if apiKeys.length === 0}
        <div class="text-center py-8 text-light-muted dark:text-dark-muted">
          暂无 API Key，点击上方按钮创建
        </div>
      {:else}
        <div class="space-y-2">
          {#each apiKeys as key}
            <div class="flex items-center justify-between p-4 glass-card bg-white/30 dark:bg-black/10">
              <div>
                <div class="font-mono text-sm">{key.key}</div>
                <div class="text-light-muted dark:text-dark-muted text-xs mt-1">
                  {key.name} · 创建于 {key.created}
                </div>
              </div>
              <button class="text-red-500 hover:text-red-400 text-sm">删除</button>
            </div>
          {/each}
        </div>
      {/if}
    </section>
    
    <!-- 快速开始 -->
    <section class="glass-card p-6">
      <h2 class="text-xl font-semibold mb-4">快速开始</h2>
      <div class="bg-black/5 dark:bg-white/5 p-4 rounded-xl font-mono text-sm overflow-x-auto">
        <pre class="text-light-muted dark:text-dark-muted">
curl https://api.foxnio.io/v1/chat/completions \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model": "gpt-4-turbo", "messages": [{"role": "user", "content": "Hello!"}]}'
        </pre>
      </div>
    </section>
  </div>
{/if}
