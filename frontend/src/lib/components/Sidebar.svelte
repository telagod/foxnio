<script lang="ts">
  import { page } from '$app/stores';
  import { createEventDispatcher } from 'svelte';
  import { onMount } from 'svelte';
  
  export let collapsed = false;
  export let mobileOpen = false;
  
  const dispatch = createEventDispatcher();
  
  // 主题状态
  let isDark = $state(false);
  
  onMount(() => {
    // 检测系统主题
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    isDark = mediaQuery.matches;
    
    // 监听主题变化
    mediaQuery.addEventListener('change', (e) => {
      isDark = e.matches;
    });
    
    // 监听文档主题变化
    const observer = new MutationObserver(() => {
      isDark = document.documentElement.classList.contains('dark');
    });
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['class']
    });
    
    return () => observer.disconnect();
  });
  
  let navItems = [
    { href: '/', label: 'Dashboard', icon: '📊' },
    { href: '/apikeys', label: 'API Keys', icon: '🗝️' },
    { href: '/usage', label: 'Usage', icon: '📈' },
    { href: '/health', label: 'Health', icon: '🏥' },
    { href: '/playground', label: 'Playground', icon: '💬' },
  ];
  
  let adminItems = [
    { href: '/admin', label: 'Admin Dashboard', icon: '⚙️' },
    { href: '/admin/users', label: 'Users', icon: '👥' },
    { href: '/admin/accounts', label: 'Accounts', icon: '🔑' },
    { href: '/admin/apikeys', label: 'All API Keys', icon: '🗝️' },
    { href: '/admin/stats', label: 'Statistics', icon: '📊' },
  ];
  
  function closeMobile() {
    mobileOpen = false;
  }
</script>

<!-- 移动端遮罩 -->
{#if mobileOpen}
  <div 
    class="fixed inset-0 bg-black bg-opacity-50 z-40 lg:hidden backdrop-blur-sm"
    on:click={closeMobile}
    onkeydown={(e) => e.key === 'Escape' && closeMobile()}
    role="button"
    tabindex="0"
    aria-label="Close menu"
  ></div>
{/if}

<!-- 侧边栏 -->
<aside 
  class="fixed lg:relative inset-y-0 left-0 z-50 
         bg-white dark:bg-gray-900 text-gray-900 dark:text-white
         border-r border-gray-200 dark:border-gray-800
         transform transition-all duration-300 ease-in-out
         {mobileOpen ? 'translate-x-0' : '-translate-x-full lg:translate-x-0'}
         {collapsed ? 'lg:w-20' : 'lg:w-64'}
         w-64 shadow-lg lg:shadow-none"
>
  <!-- Logo -->
  <div class="h-16 flex items-center justify-between px-4 border-b border-gray-200 dark:border-gray-800">
    <a href="/" class="flex items-center gap-3 group" on:click={closeMobile}>
      <!-- Logo SVG - 自动切换深浅色 -->
      <div class="w-8 h-8 flex-shrink-0 transition-transform group-hover:scale-110">
        {#if isDark}
          <!-- 暗色模式 - 白色狐狸 -->
          <svg viewBox="0 0 100 100" fill="none" class="w-full h-full">
            <path d="M32 12 L18 45 L50 88 L82 45 L68 12 L58 45 L42 45 Z" fill="#ffffff"/>
            <path d="M26 48 L74 48 L50 56 Z" fill="#1a1a1a"/>
            <path d="M46 58 L54 58 L50 82 Z" fill="#1a1a1a"/>
          </svg>
        {:else}
          <!-- 亮色模式 - 黑色狐狸 -->
          <svg viewBox="0 0 100 100" fill="none" class="w-full h-full">
            <path d="M32 12 L18 45 L50 88 L82 45 L68 12 L58 45 L42 45 Z" fill="#1a1a1a"/>
            <path d="M26 48 L74 48 L50 56 Z" fill="#ffffff"/>
            <path d="M46 58 L54 58 L50 82 Z" fill="#ffffff"/>
          </svg>
        {/if}
      </div>
      
      {#if !collapsed}
        <div class="flex flex-col">
          <span class="text-lg font-bold tracking-tight">FoxNIO</span>
          <span class="text-xs text-gray-500 dark:text-gray-400">AI API Gateway</span>
        </div>
      {/if}
    </a>
    
    <!-- 移动端关闭按钮 -->
    <button 
      class="lg:hidden p-2 rounded-lg text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
      on:click={closeMobile}
      aria-label="Close sidebar"
    >
      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path>
      </svg>
    </button>
  </div>
  
  <!-- 折叠按钮（桌面端） -->
  <button
    class="hidden lg:flex absolute -right-3 top-20 w-6 h-6 bg-white dark:bg-gray-800 rounded-full 
           items-center justify-center text-gray-500 dark:text-gray-400
           border border-gray-300 dark:border-gray-700 
           hover:border-gray-400 dark:hover:border-gray-600
           hover:text-gray-700 dark:hover:text-gray-200
           transition-all shadow-sm hover:shadow"
    on:click={() => collapsed = !collapsed}
    aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
  >
    <svg class="w-4 h-4 transition-transform duration-300 {collapsed ? 'rotate-180' : ''}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"></path>
    </svg>
  </button>

  <!-- 导航 -->
  <nav class="p-4 space-y-1 overflow-y-auto h-[calc(100vh-8rem)]">
    <!-- 主菜单 -->
    <div class="mb-4">
      {#if !collapsed}
        <div class="text-xs font-semibold text-gray-400 dark:text-gray-500 uppercase tracking-wider mb-2 px-3">
          Main Menu
        </div>
      {/if}
      {#each navItems as item}
        <a
          href={item.href}
          on:click={closeMobile}
          class="flex items-center px-3 py-2.5 rounded-lg transition-all group
                 {$page.url.pathname === item.href 
                   ? 'bg-blue-50 dark:bg-blue-900/20 text-blue-600 dark:text-blue-400 font-medium' 
                   : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800'}
                 {collapsed ? 'justify-center' : ''}"
          title={collapsed ? item.label : ''}
        >
          <span class="text-lg {collapsed ? '' : 'mr-3'}">{item.icon}</span>
          {#if !collapsed}
            <span>{item.label}</span>
          {/if}
        </a>
      {/each}
    </div>

    <!-- 管理员菜单 -->
    <div class="pt-4 border-t border-gray-200 dark:border-gray-800">
      {#if !collapsed}
        <div class="text-xs font-semibold text-gray-400 dark:text-gray-500 uppercase tracking-wider mb-2 px-3">
          Admin
        </div>
      {/if}
      {#each adminItems as item}
        <a
          href={item.href}
          on:click={closeMobile}
          class="flex items-center px-3 py-2.5 rounded-lg transition-all group
                 {$page.url.pathname === item.href 
                   ? 'bg-blue-50 dark:bg-blue-900/20 text-blue-600 dark:text-blue-400 font-medium' 
                   : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800'}
                 {collapsed ? 'justify-center' : ''}"
          title={collapsed ? item.label : ''}
        >
          <span class="text-lg {collapsed ? '' : 'mr-3'}">{item.icon}</span>
          {#if !collapsed}
            <span>{item.label}</span>
          {/if}
        </a>
      {/each}
    </div>
  </nav>

  <!-- 版本信息 -->
  {#if !collapsed}
    <div class="absolute bottom-4 left-4 right-4">
      <div class="bg-gray-50 dark:bg-gray-800/50 rounded-lg p-3 border border-gray-200 dark:border-gray-800">
        <div class="flex items-center gap-2 mb-1">
          <!-- 小 Logo -->
          <div class="w-4 h-4 flex-shrink-0">
            {#if isDark}
              <svg viewBox="0 0 100 100" fill="none" class="w-full h-full">
                <path d="M32 12 L18 45 L50 88 L82 45 L68 12 L58 45 L42 45 Z" fill="#ffffff"/>
                <path d="M26 48 L74 48 L50 56 Z" fill="#1a1a1a"/>
                <path d="M46 58 L54 58 L50 82 Z" fill="#1a1a1a"/>
              </svg>
            {:else}
              <svg viewBox="0 0 100 100" fill="none" class="w-full h-full">
                <path d="M32 12 L18 45 L50 88 L82 45 L68 12 L58 45 L42 45 Z" fill="#1a1a1a"/>
                <path d="M26 48 L74 48 L50 56 Z" fill="#ffffff"/>
                <path d="M46 58 L54 58 L50 82 Z" fill="#ffffff"/>
              </svg>
            {/if}
          </div>
          <span class="text-xs font-medium text-gray-700 dark:text-gray-300">FoxNIO</span>
        </div>
        <p class="text-xs text-gray-500 dark:text-gray-400">v0.1.0 · 优雅 · 专业 · 克制</p>
      </div>
    </div>
  {/if}
</aside>

<style>
  /* 自定义滚动条 */
  nav::-webkit-scrollbar {
    width: 6px;
  }
  
  nav::-webkit-scrollbar-track {
    background: transparent;
  }
  
  nav::-webkit-scrollbar-thumb {
    background: rgba(0, 0, 0, 0.1);
    border-radius: 3px;
  }
  
  nav::-webkit-scrollbar-thumb:hover {
    background: rgba(0, 0, 0, 0.2);
  }
  
  :global(.dark) nav::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.1);
  }
  
  :global(.dark) nav::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.2);
  }
</style>
