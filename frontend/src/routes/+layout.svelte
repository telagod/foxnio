<script lang="ts">
  import '../styles/global.css';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import { page } from '$app/stores';
  import { onMount } from 'svelte';

  let { children } = $props();

  let sidebarCollapsed = $state(false);
  let sidebarMobileOpen = $state(false);
  let isMobile = $state(false);
  let isDark = $state(false);

  const routeTitles: Record<string, string> = {
    '/': 'Dashboard',
    '/dashboard': 'Dashboard',
    '/apikeys': 'API Keys',
    '/usage': 'Usage',
    '/alerts': 'Alerts',
    '/health': 'Health',
    '/playground': 'Playground',
    '/login': 'Login',
    '/register': 'Register',
    '/admin': 'Admin Dashboard',
    '/admin/users': 'Users',
    '/admin/accounts': 'Accounts',
    '/admin/apikeys': 'All API Keys',
    '/admin/stats': 'Statistics',
  };

  let pageTitle = $derived(routeTitles[$page.url.pathname] ?? 'FoxNIO');

  onMount(() => {
    const checkMobile = () => {
      isMobile = window.innerWidth < 1024;
      if (!isMobile) sidebarMobileOpen = false;
    };

    const checkTheme = () => {
      isDark = document.documentElement.classList.contains('dark') ||
               window.matchMedia('(prefers-color-scheme: dark)').matches;
    };

    checkMobile();
    checkTheme();
    window.addEventListener('resize', checkMobile);
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
  <Sidebar
    collapsed={sidebarCollapsed}
    mobileOpen={sidebarMobileOpen}
  />

  <div class="flex-1 flex flex-col min-w-0">
    <header class="h-16 bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700
                    sticky top-0 z-30 flex items-center px-4 lg:px-6 shadow-sm">
      <button
        class="lg:hidden p-2 rounded-lg text-gray-600 dark:text-gray-300
               hover:bg-gray-100 dark:hover:bg-gray-700 mr-4 transition-colors"
        onclick={toggleMobileSidebar}
        aria-label="Toggle sidebar"
      >
        <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3.75 6.75h16.5M3.75 12h16.5m-16.5 5.25h16.5"/>
        </svg>
      </button>

      <div class="lg:hidden flex items-center gap-2 mr-4">
        <div class="w-6 h-6">
          <svg viewBox="0 0 100 100" fill="none" class="w-full h-full" aria-hidden="true">
            <path d="M32 12 L18 45 L50 88 L82 45 L68 12 L58 45 L42 45 Z" class="fill-gray-900 dark:fill-white"/>
            <path d="M26 48 L74 48 L50 56 Z" class="fill-white dark:fill-gray-900"/>
            <path d="M46 58 L54 58 L50 82 Z" class="fill-white dark:fill-gray-900"/>
          </svg>
        </div>
        <span class="text-sm font-bold text-gray-900 dark:text-white">FoxNIO</span>
      </div>

      <div class="flex-1 hidden lg:block">
        <h1 class="text-lg font-semibold text-gray-900 dark:text-white">{pageTitle}</h1>
      </div>

      <div class="flex items-center gap-2 sm:gap-3 ml-auto">
        <button
          onclick={toggleTheme}
          class="p-2 rounded-lg text-gray-600 dark:text-gray-300
                 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
          aria-label={isDark ? 'Switch to light mode' : 'Switch to dark mode'}
        >
          {#if isDark}
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 3v2.25m6.364.386l-1.591 1.591M21 12h-2.25m-.386 6.364l-1.591-1.591M12 18.75V21m-4.773-4.227l-1.591 1.591M5.25 12H3m4.227-4.773L5.636 5.636M15.75 12a3.75 3.75 0 11-7.5 0 3.75 3.75 0 017.5 0z"/>
            </svg>
          {:else}
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21.752 15.002A9.718 9.718 0 0118 15.75c-5.385 0-9.75-4.365-9.75-9.75 0-1.33.266-2.597.748-3.752A9.753 9.753 0 003 11.25C3 16.635 7.365 21 12.75 21a9.753 9.753 0 009.002-5.998z"/>
            </svg>
          {/if}
        </button>

        <button
          class="p-2 rounded-lg text-gray-600 dark:text-gray-300
                 hover:bg-gray-100 dark:hover:bg-gray-700 relative transition-colors"
          aria-label="Notifications"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.857 17.082a23.848 23.848 0 005.454-1.31A8.967 8.967 0 0118 9.75v-.7V9A6 6 0 006 9v.75a8.967 8.967 0 01-2.312 6.022c1.733.64 3.56 1.085 5.455 1.31m5.714 0a24.255 24.255 0 01-5.714 0m5.714 0a3 3 0 11-5.714 0"/>
          </svg>
          <span class="absolute top-1.5 right-1.5 w-2 h-2 bg-red-500 rounded-full" aria-hidden="true"></span>
        </button>

        <button
          class="flex items-center gap-2 p-2 rounded-lg
                 text-gray-700 dark:text-gray-200
                 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
          aria-label="User menu"
        >
          <div class="w-8 h-8 bg-gradient-to-br from-blue-500 to-purple-600 rounded-full flex items-center justify-center">
            <span class="text-white text-sm font-medium">U</span>
          </div>
          <svg class="w-4 h-4 hidden sm:block" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19.5 8.25l-7.5 7.5-7.5-7.5"/>
          </svg>
        </button>
      </div>
    </header>

    <main class="flex-1 overflow-auto p-4 lg:p-6">
      {@render children()}
    </main>

    <footer class="h-12 bg-white dark:bg-gray-800 border-t border-gray-200 dark:border-gray-700
                    flex items-center justify-center text-sm text-gray-500 dark:text-gray-400">
      <div class="flex items-center gap-2">
        <div class="w-4 h-4 flex-shrink-0">
          <svg viewBox="0 0 100 100" fill="none" class="w-full h-full" aria-hidden="true">
            <path d="M32 12 L18 45 L50 88 L82 45 L68 12 L58 45 L42 45 Z" class="fill-gray-500 dark:fill-gray-400"/>
            <path d="M26 48 L74 48 L50 56 Z" class="fill-white dark:fill-gray-800"/>
            <path d="M46 58 L54 58 L50 82 Z" class="fill-white dark:fill-gray-800"/>
          </svg>
        </div>
        <span>FoxNIO &middot; AI API Gateway</span>
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
  :global(html, body) {
    overflow-x: hidden;
  }
</style>
