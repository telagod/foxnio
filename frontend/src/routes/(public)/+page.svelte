<script lang="ts">
  import ThemeToggle from '$lib/components/ThemeToggle.svelte';

  const features = [
    {
      title: 'API Key 管理',
      desc: '生成、分发、管理 API Key',
      iconPath: 'M15.75 5.25a3 3 0 0 1 3 3m3 0a6 6 0 0 1-7.029 5.912c-.563-.097-1.159.026-1.563.43L10.5 17.25H8.25v2.25H6v2.25H2.25v-2.818c0-.597.237-1.17.659-1.591l6.499-6.499c.404-.404.527-1 .43-1.563A6 6 0 1 1 21.75 8.25Z',
    },
    {
      title: '大规模号池',
      desc: '支持 OAuth、API Key 号池统一纳管',
      iconPath: 'M17.982 18.725A7.488 7.488 0 0 0 12 15.75a7.488 7.488 0 0 0-5.982 2.975m11.963 0a9 9 0 1 0-11.963 0m11.963 0A8.966 8.966 0 0 1 12 21a8.966 8.966 0 0 1-5.982-2.275M15 9.75a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z',
    },
    {
      title: '精确计费',
      desc: 'Token 级别的用量追踪',
      iconPath: 'M15.75 15.75V18m-7.5-6.75h.008v.008H8.25v-.008Zm0 2.25h.008v.008H8.25v-.008Zm0 2.25h.008v.008H8.25v-.008Zm0 2.25h.008v.008H8.25v-.008Zm2.25-4.5h.008v.008H10.5v-.008Zm0 2.25h.008v.008H10.5v-.008Zm0 2.25h.008v.008H10.5v-.008Zm2.25-6.75h.008v.008H12.75v-.008Zm0 2.25h.008v.008H12.75v-.008Zm0 2.25h.008v.008H12.75v-.008Zm0 2.25h.008v.008H12.75v-.008Zm2.25-6.75h.008v.008H15v-.008Zm0 2.25h.008v.008H15v-.008ZM15.75 9V5.25a2.25 2.25 0 0 0-2.25-2.25h-3a2.25 2.25 0 0 0-2.25 2.25V9m7.5 0H8.25m7.5 0v.75m-7.5-.75v.75m0 0h7.5',
    },
    {
      title: '高性能调度',
      desc: '粘性会话、负载均衡、故障转移',
      iconPath: 'm3.75 13.5 10.5-11.25L12 10.5h8.25L9.75 21.75 12 13.5H3.75Z',
    },
    {
      title: '并发控制',
      desc: '用户级、账号级并发限制',
      iconPath: 'M9 12.75 11.25 15 15 9.75m-3-7.036A11.959 11.959 0 0 1 3.598 6 11.99 11.99 0 0 0 3 9.749c0 5.592 3.824 10.29 9 11.623 5.176-1.332 9-6.03 9-11.622 0-1.31-.21-2.571-.598-3.751h-.152c-3.196 0-6.1-1.248-8.25-3.285Z',
    },
    {
      title: '可观测控制面',
      desc: '实时监控、用量分析、运营回路',
      iconPath: 'M3 13.125C3 12.504 3.504 12 4.125 12h2.25c.621 0 1.125.504 1.125 1.125v6.75C7.5 20.496 6.996 21 6.375 21h-2.25A1.125 1.125 0 0 1 3 19.875v-6.75ZM9.75 8.625c0-.621.504-1.125 1.125-1.125h2.25c.621 0 1.125.504 1.125 1.125v11.25c0 .621-.504 1.125-1.125 1.125h-2.25a1.125 1.125 0 0 1-1.125-1.125V8.625ZM16.5 4.125c0-.621.504-1.125 1.125-1.125h2.25C20.496 3 21 3.504 21 4.125v15.75c0 .621-.504 1.125-1.125 1.125h-2.25a1.125 1.125 0 0 1-1.125-1.125V4.125Z',
    },
  ];

  const techStack = [
    {
      name: 'Rust + Axum',
      iconPath: 'M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0 1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.241-.438.613-.43.992a7.723 7.723 0 0 1 0 .255c-.008.378.137.75.43.991l1.004.827c.424.35.534.955.26 1.43l-1.298 2.247a1.125 1.125 0 0 1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.47 6.47 0 0 1-.22.128c-.331.183-.581.495-.644.869l-.213 1.281c-.09.543-.56.94-1.11.94h-2.594c-.55 0-1.019-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125 1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.43-.991a6.932 6.932 0 0 1 0-.255c.007-.38-.138-.751-.43-.992l-1.004-.827a1.125 1.125 0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.086.22-.128.332-.183.582-.495.644-.869l.214-1.28Z M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z',
    },
    {
      name: 'SvelteKit',
      iconPath: 'm3.75 13.5 10.5-11.25L12 10.5h8.25L9.75 21.75 12 13.5H3.75Z',
    },
    {
      name: 'PostgreSQL',
      iconPath: 'M20.25 6.375c0 2.278-3.694 4.125-8.25 4.125S3.75 8.653 3.75 6.375m16.5 0c0-2.278-3.694-4.125-8.25-4.125S3.75 4.097 3.75 6.375m16.5 0v11.25c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125V6.375m16.5 0v3.75m-16.5-3.75v3.75m16.5 0v3.75C20.25 16.153 16.556 18 12 18s-8.25-1.847-8.25-4.125v-3.75m16.5 0c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125',
    },
    {
      name: 'Redis',
      iconPath: 'M5.25 14.25h13.5m-13.5 0a3 3 0 0 1-3-3m3 3a3 3 0 1 0 0 6h13.5a3 3 0 1 0 0-6m-16.5-3a3 3 0 0 1 3-3h13.5a3 3 0 0 1 3 3m-19.5 0a4.5 4.5 0 0 1 .9-2.7L5.737 5.1a3.375 3.375 0 0 1 2.7-1.35h7.126c1.062 0 2.062.5 2.7 1.35l2.587 3.45a4.5 4.5 0 0 1 .9 2.7m0 0a3 3 0 0 1-3 3m0 3h.008v.008h-.008v-.008Zm0-6h.008v.008h-.008v-.008Zm-3 6h.008v.008h-.008v-.008Zm0-6h.008v.008h-.008v-.008Z',
    },
  ];
</script>

<svelte:head>
  <title>FoxNIO - AI API Gateway</title>
</svelte:head>

<ThemeToggle floating={true} />

<div class="animate-fade-in">
  <!-- Hero -->
  <section class="text-center mb-16">
    <div class="inline-flex items-center gap-2 glass-card px-4 py-2 mb-6">
      <span class="animate-pulse-soft text-brand-primary">
        <svg viewBox="0 0 100 100" fill="none" class="w-5 h-5" aria-hidden="true">
          <path d="M32 12 L18 45 L50 88 L82 45 L68 12 L58 45 L42 45 Z" fill="currentColor"/>
          <path d="M26 48 L74 48 L50 56 Z" fill="currentColor" opacity="0.3"/>
          <path d="M46 58 L54 58 L50 82 Z" fill="currentColor" opacity="0.3"/>
        </svg>
      </span>
      <span class="text-sm font-medium">FoxNIO v0.1.0</span>
    </div>

    <h1 class="text-4xl sm:text-5xl font-bold mb-4 tracking-tight">
      FoxNIO Gateway
    </h1>
    <p class="text-lg sm:text-xl text-light-muted dark:text-dark-muted mb-8 max-w-2xl mx-auto">
      高性能 AI API Gateway / Control Plane
      <br />
      <span class="text-brand-primary">批量导入快</span> ·
      <span class="text-brand-secondary">号池调度稳</span> ·
      <span class="text-brand-accent">代理链路清晰</span>
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

  <!-- Features -->
  <section class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6 mb-16">
    {#each features as feature}
      <div class="glass-card p-6 hover:scale-[1.02] transition-transform">
        <div class="mb-4 text-brand-primary">
          <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-7 h-7" aria-hidden="true">
            <path stroke-linecap="round" stroke-linejoin="round" d={feature.iconPath}/>
          </svg>
        </div>
        <h3 class="text-lg font-semibold mb-2">{feature.title}</h3>
        <p class="text-light-muted dark:text-dark-muted text-sm">{feature.desc}</p>
      </div>
    {/each}
  </section>

  <!-- Tech Stack -->
  <section class="glass-card p-8 text-center">
    <h2 class="text-xl font-semibold mb-6">技术栈</h2>
    <div class="flex flex-wrap justify-center gap-4">
      {#each techStack as tech}
        <div class="glass-button flex items-center gap-2">
          <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-5 h-5 text-brand-secondary" aria-hidden="true">
            <path stroke-linecap="round" stroke-linejoin="round" d={tech.iconPath}/>
          </svg>
          <span>{tech.name}</span>
        </div>
      {/each}
    </div>
  </section>
</div>
