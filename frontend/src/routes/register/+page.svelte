<script lang="ts">
  import { goto } from '$app/navigation';
  
  let email = $state('');
  let password = $state('');
  let confirmPassword = $state('');
  let loading = $state(false);
  let error = $state('');
  
  async function handleRegister(e: Event) {
    e.preventDefault();
    error = '';
    
    if (password !== confirmPassword) {
      error = '密码不匹配';
      return;
    }
    
    loading = true;
    
    try {
      const response = await fetch('/api/v1/auth/register', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email, password }),
      });
      
      if (response.ok) {
        goto('/login?registered=true');
      } else {
        const data = await response.json();
        error = data.error || '注册失败';
      }
    } catch (e) {
      error = '网络错误，请重试';
    } finally {
      loading = false;
    }
  }
</script>

<svelte:head>
  <title>注册 - FoxNIO</title>
</svelte:head>

<div class="min-h-[80vh] flex items-center justify-center animate-fade-in">
  <div class="glass-card p-8 w-full max-w-md">
    <div class="text-center mb-8">
      <span class="text-4xl mb-4 block">🦊</span>
      <h1 class="text-2xl font-bold">创建账户</h1>
      <p class="text-light-muted dark:text-dark-muted mt-2">开始使用 FoxNIO</p>
    </div>
    
    <form onsubmit={handleRegister} class="space-y-4">
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
          placeholder="至少 8 个字符"
          required
          minlength={8}
          class="glass-input w-full"
        />
      </div>
      
      <div>
        <label class="block text-sm font-medium mb-2">确认密码</label>
        <input
          type="password"
          bind:value={confirmPassword}
          placeholder="再次输入密码"
          required
          class="glass-input w-full"
        />
      </div>
      
      <button
        type="submit"
        disabled={loading}
        class="w-full glass-button bg-brand-primary/20 hover:bg-brand-primary/30 font-semibold py-3 disabled:opacity-50"
      >
        {loading ? '注册中...' : '注册'}
      </button>
    </form>
    
    <p class="text-center text-light-muted dark:text-dark-muted mt-6 text-sm">
      已有账户？<a href="/login" class="text-brand-primary hover:underline">登录</a>
    </p>
  </div>
</div>
