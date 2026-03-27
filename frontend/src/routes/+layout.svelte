<script lang="ts">
  import Sidebar from '$lib/components/Sidebar.svelte';
  import { onMount } from 'svelte';
  
  let { children } = $props();
  
  // 响应式状态
  let sidebarCollapsed = $state(false);
  let sidebarMobileOpen = $state(false);
  let isMobile = $state(false);
  let isDark = $state(false);
  
  onMount(() => {
    // 检测屏幕尺寸
    const checkMobile = () => {
      isMobile = window.innerWidth < 1024;
      if (!isMobile) {
        sidebarMobileOpen = false;
      }
    };
    
    // 检测主题
    const checkTheme = () => {
      isDark = document.documentElement.classList.contains('dark') ||
               window.matchMedia('(prefers-color-scheme: dark)').matches;
    };
    
    checkMobile();
    checkTheme();
    
    window.addEventListener('resize', checkMobile);
    
    // 监听主题变化
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    mediaQuery.addEventListener('change', checkTheme);
    
    const observer = new MutationObserver(checkTheme);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['class']
    });
    
    return () => {
      window.removeEventListener('resize', checkMobile);
      mediaQuery.removeEventListener('change', checkTheme);
      observer.disconnect();
    };
  });
  
  function toggleMobileSidebar() {
    sidebarMobileOpen = !sidebarMobileOpen;
  }
  
  function toggleTheme() {
    isDark = !isDark;
    document.documentElement.classList.toggle('dark', isDark);
    localStorage.setItem('theme', isDark ? 'dark' : 'light');
  }
</script>

<div class="min-h-screen bg-gray-50 dark:bg-gray-900 flex">
  <!-- 侧边栏 -->
  <Sidebar 
    collapsed={sidebarCollapsed} 
    mobileOpen={sidebarMobileOpen}
  />
  
  <!-- 主内容区 -->
  <div class="flex-1 flex flex-col min-w-0">
    <!-- 顶部导航栏 -->
    <header class="h-16 bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 
                  sticky top-0 z-30 flex items-center px-4 lg:px-6 shadow-sm">
      <!-- 移动端菜单按钮 -->
      <button
        class="lg:hidden p-2 rounded-lg text-gray-600 dark:text-gray-300 
               hover:bg-gray-100 dark:hover:bg-gray-700 mr-4 transition-colors"
        onclick={toggleMobileSidebar}
        aria-label="Toggle sidebar"
      >
        <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                d="M4 6h16M4 12h16M4 18h16"></path>
        </svg>
      </button>
      
      <!-- Logo（移动端显示） -->
      <div class="lg:hidden flex items-center gap-2 mr-4">
        <div class="w-6 h-6">
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
        <span class="text-sm font-bold">FoxNIO</span>
      </div>
      
      <!-- 面包屑 / 标题 -->
      <div class="flex-1 hidden lg:block">
        <h1 class="text-lg font-semibold text-gray-900 dark:text-white">
          Dashboard
        </h1>
      </div>
      
      <!-- 右侧操作 -->
      <div class="flex items-center gap-2 sm:gap-3">
        <!-- 主题切换 -->
        <button
          onclick={toggleTheme}
          class="p-2 rounded-lg text-gray-600 dark:text-gray-300 
                 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
          aria-label="Toggle theme"
        >
          {#if isDark}
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                    d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z"></path>
            </svg>
          {:else}
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                    d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z"></path>
            </svg>
          {/if}
        </button>
        
        <!-- 通知 -->
        <button class="p-2 rounded-lg text-gray-600 dark:text-gray-300 
                       hover:bg-gray-100 dark:hover:bg-gray-700 relative transition-colors"
                aria-label="Notifications">
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                  d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9"></path>
          </svg>
          <!-- 通知徽章 -->
          <span class="absolute top-1.5 right-1.5 w-2 h-2 bg-red-500 rounded-full"></span>
        </button>
        
        <!-- 用户菜单 -->
        <button class="flex items-center gap-2 p-2 rounded-lg 
                       text-gray-700 dark:text-gray-200
                       hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
                aria-label="User menu">
          <div class="w-8 h-8 bg-gradient-to-br from-blue-500 to-purple-600 rounded-full flex items-center justify-center">
            <span class="text-white text-sm font-medium">U</span>
          </div>
          <svg class="w-4 h-4 hidden sm:block" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path>
          </svg>
        </button>
      </div>
    </header>
    
    <!-- 页面内容 -->
    <main class="flex-1 overflow-auto p-4 lg:p-6">
      {@render children()}
    </main>
    
    <!-- 页脚 -->
    <footer class="h-12 bg-white dark:bg-gray-800 border-t border-gray-200 dark:border-gray-700
                  flex items-center justify-center text-sm text-gray-500 dark:text-gray-400">
      <div class="flex items-center gap-2">
        <div class="w-4 h-4">
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
        <span>FoxNIO · AI API Gateway · 优雅 · 专业 · 克制</span>
      </div>
    </footer>
  </div>
</div>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
  }
  
  /* 防止内容溢出 */
  :global(html, body) {
    overflow-x: hidden;
  }
</style>
