<script lang="ts">
  import { goto } from '$app/navigation';
  import { api } from '$lib/api';
  import ThemeToggle from '$lib/components/ThemeToggle.svelte';

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
      await api.register(email, password);
      goto('/login?registered=true');
    } catch (e) {
      error = e instanceof Error ? e.message : '注册失败';
    } finally {
      loading = false;
    }
  }
</script>

<svelte:head>
  <title>注册 - FoxNIO</title>
</svelte:head>

<ThemeToggle floating={true} />

<div class="min-h-[80vh] flex items-center justify-center animate-fade-in">
  <div class="glass-card p-8 w-full max-w-md">
    <div class="text-center mb-8">
      <span class="inline-block text-brand-primary mb-4">
        <svg viewBox="0 0 100 100" fill="none" class="w-10 h-10 mx-auto" aria-hidden="true">
          <path d="M32 12 L18 45 L50 88 L82 45 L68 12 L58 45 L42 45 Z" fill="currentColor"/>
          <path d="M26 48 L74 48 L50 56 Z" fill="currentColor" opacity="0.3"/>
          <path d="M46 58 L54 58 L50 82 Z" fill="currentColor" opacity="0.3"/>
        </svg>
      </span>
      <h1 class="text-2xl font-bold">创建账户</h1>
      <p class="text-light-muted dark:text-dark-muted mt-2">开始使用 FoxNIO</p>
    </div>

    <form onsubmit={handleRegister} class="space-y-4">
      {#if error}
        <div class="bg-red-500/10 border border-red-500/20 text-red-500 px-4 py-3 rounded-xl text-sm" role="alert">
          {error}
        </div>
      {/if}

      <div>
        <label for="reg-email" class="block text-sm font-medium mb-2">邮箱</label>
        <input
          id="reg-email"
          type="email"
          bind:value={email}
          placeholder="your@email.com"
          required
          autocomplete="email"
          class="glass-input w-full"
        />
      </div>

      <div>
        <label for="reg-password" class="block text-sm font-medium mb-2">密码</label>
        <input
          id="reg-password"
          type="password"
          bind:value={password}
          placeholder="至少 8 个字符"
          required
          minlength={8}
          autocomplete="new-password"
          class="glass-input w-full"
        />
      </div>

      <div>
        <label for="reg-confirm" class="block text-sm font-medium mb-2">确认密码</label>
        <input
          id="reg-confirm"
          type="password"
          bind:value={confirmPassword}
          placeholder="再次输入密码"
          required
          autocomplete="new-password"
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
