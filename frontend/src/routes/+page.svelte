<script lang="ts">
  import { onMount } from 'svelte';
  
  let stats = $state({
    totalUsers: 0,
    totalKeys: 0,
    totalAccounts: 0,
    totalRequests: 0,
  });
  
  const features = [
    { icon: '🔑', title: 'API Key 管理', desc: '生成、分发、管理 API Key' },
    { icon: '👤', title: '多账号管理', desc: '支持 OAuth、API Key 多种认证' },
    { icon: '📊', title: '精确计费', desc: 'Token 级别的用量追踪' },
    { icon: '⚡', title: '智能调度', desc: '粘性会话、负载均衡' },
    { icon: '🛡️', title: '并发控制', desc: '用户级、账号级并发限制' },
    { icon: '📈', title: '监控看板', desc: '实时监控、用量分析' },
  ];
</script>

<svelte:head>
  <title>FoxNIO - AI API Gateway</title>
</svelte:head>

<div class="animate-fade-in">
  <!-- Hero -->
  <section class="text-center mb-16">
    <div class="inline-flex items-center gap-2 glass-card px-4 py-2 mb-6">
      <span class="animate-pulse-soft">🦊</span>
      <span class="text-sm font-medium">FoxNIO v0.1.0</span>
    </div>
    
    <h1 class="text-5xl font-bold mb-4 tracking-tight">
      AI API Gateway
    </h1>
    <p class="text-xl text-light-muted dark:text-dark-muted mb-8 max-w-2xl mx-auto">
      订阅配额分发管理平台
      <br />
      <span class="text-brand-primary">优雅</span> · 
      <span class="text-brand-secondary">专业</span> · 
      <span class="text-brand-accent">克制</span>
    </p>
    
    <div class="flex gap-4 justify-center">
      <a href="/login" class="glass-button bg-brand-primary/20 hover:bg-brand-primary/30">
        登录
      </a>
      <a href="/register" class="glass-button">
        注册
      </a>
    </div>
  </section>
  
  <!-- 功能卡片 -->
  <section class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6 mb-16">
    {#each features as feature}
      <div class="glass-card p-6 hover:scale-[1.02] transition-transform">
        <div class="text-3xl mb-4">{feature.icon}</div>
        <h3 class="text-lg font-semibold mb-2">{feature.title}</h3>
        <p class="text-light-muted dark:text-dark-muted text-sm">{feature.desc}</p>
      </div>
    {/each}
  </section>
  
  <!-- 技术栈 -->
  <section class="glass-card p-8 text-center">
    <h2 class="text-xl font-semibold mb-6">技术栈</h2>
    <div class="flex flex-wrap justify-center gap-4">
      <div class="glass-button flex items-center gap-2">
        <span>🦀</span> Rust + Axum
      </div>
      <div class="glass-button flex items-center gap-2">
        <span>⚡</span> SvelteKit
      </div>
      <div class="glass-button flex items-center gap-2">
        <span>🐘</span> PostgreSQL
      </div>
      <div class="glass-button flex items-center gap-2">
        <span>🔴</span> Redis
      </div>
    </div>
  </section>
</div>
