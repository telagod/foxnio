<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import { api } from '$lib/api';
  
  let email = $state('');
  let password = $state('');
  let loading = $state(false);
  let error = $state('');
  
  let registered = $derived($page.url.searchParams.get('registered') === 'true');
  
  async function handleLogin(e: Event) {
    e.preventDefault();
    error = '';
    loading = true;
    
    try {
      const data = await api.login(email, password);
      localStorage.setItem('token', data.token);
      api.setToken(data.token);
      goto('/dashboard');
    } catch (e) {
      error = e instanceof Error ? e.message : '登录失败';
    } finally {
      loading = false;
    }
  }
</script>

<svelte:head>
  <title>登录 - FoxNIO</title>
</svelte:head>

<div class="min-h-[80vh] flex items-center justify-center animate-fade-in">
  <div class="glass-card p-8 w-full max-w-md">
    <div class="text-center mb-8">
      <span class="text-4xl mb-4 block">🦊</span>
      <h1 class="text-2xl font-bold">欢迎回来</h1>
      <p class="text-light-muted dark:text-dark-muted mt-2">登录 FoxNIO</p>
    </div>
    
    {#if registered}
      <div class="bg-green-500/10 border border-green-500/20 text-green-500 px-4 py-3 rounded-xl text-sm mb-4">
        注册成功！请登录。
      </div>
    {/if}
    
    <form onsubmit={handleLogin} class="space-y-4">
      {#if error}
        <div class="bg-red-500/10 border border-red-500/20 text-red-500 px-4 py-3 rounded-xl text-sm">
          {error}
        </div>
      {/if}
      
      <div>
        <label class="block text-sm font-medium mb-2">邮箱</label>
        <input
          type="email"
          bind:value={email}
          placeholder="your@email.com"
          required
          class="glass-input w-full"
        />
      </div>
      
      <div>
        <label class="block text-sm font-medium mb-2">密码</label>
        <input
          type="password"
          bind:value={password}
          placeholder="输入密码"
          required
          class="glass-input w-full"
        />
      </div>
      
      <div class="flex items-center justify-between text-sm">
        <label class="flex items-center gap-2 cursor-pointer">
          <input type="checkbox" class="rounded" />
          <span class="text-light-muted dark:text-dark-muted">记住我</span>
        </label>
        <a href="/forgot-password" class="text-brand-primary hover:underline">
          忘记密码？
        </a>
      </div>
      
      <button
        type="submit"
        disabled={loading}
        class="w-full glass-button bg-brand-primary/20 hover:bg-brand-primary/30 font-semibold py-3 disabled:opacity-50"
      >
        {loading ? '登录中...' : '登录'}
      </button>
    </form>
    
    <p class="text-center text-light-muted dark:text-dark-muted mt-6 text-sm">
      还没有账户？<a href="/register" class="text-brand-primary hover:underline">注册</a>
    </p>
  </div>
</div>
