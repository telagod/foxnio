<script lang="ts">
  import { page } from '$app/stores';

  let { collapsed = false, mobileOpen = false }: { collapsed?: boolean; mobileOpen?: boolean } = $props();

  interface NavItem { href: string; label: string; icon: string; }

  let navItems: NavItem[] = [
    { href: '/dashboard', label: 'Dashboard', icon: 'chart-bar' },
    { href: '/apikeys', label: 'API Keys', icon: 'key' },
    { href: '/usage', label: 'Usage', icon: 'trending-up' },
    { href: '/alerts', label: 'Alerts', icon: 'bell' },
    { href: '/health', label: 'Health', icon: 'heart' },
    { href: '/playground', label: 'Playground', icon: 'chat-bubble' },
  ];

  let adminItems: NavItem[] = [
    { href: '/admin', label: 'Admin Dashboard', icon: 'cog' },
    { href: '/admin/users', label: 'Users', icon: 'users' },
    { href: '/admin/accounts', label: 'Accounts', icon: 'shield-check' },
    { href: '/admin/groups', label: 'Groups', icon: 'collection' },
    { href: '/admin/apikeys', label: 'All API Keys', icon: 'key' },
    { href: '/admin/stats', label: 'Statistics', icon: 'presentation-chart' },
  ];

  function closeMobile() { mobileOpen = false; }

  function isActive(p: string, h: string): boolean {
    if (h === '/') return p === '/';
    return p === h || p.startsWith(h + '/');
  }
</script>

{#snippet navIcon(name: string)}
  {#if name === 'chart-bar'}
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M3 13.125C3 12.504 3.504 12 4.125 12h2.25c.621 0 1.125.504 1.125 1.125v6.75C7.5 20.496 6.996 21 6.375 21h-2.25A1.125 1.125 0 013 19.875v-6.75zM9.75 8.625c0-.621.504-1.125 1.125-1.125h2.25c.621 0 1.125.504 1.125 1.125v11.25c0 .621-.504 1.125-1.125 1.125h-2.25a1.125 1.125 0 01-1.125-1.125V8.625zM16.5 4.125c0-.621.504-1.125 1.125-1.125h2.25C20.496 3 21 3.504 21 4.125v15.75c0 .621-.504 1.125-1.125 1.125h-2.25a1.125 1.125 0 01-1.125-1.125V4.125z"/></svg>
  {:else if name === 'key'}
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M15.75 5.25a3 3 0 013 3m3 0a6 6 0 01-7.029 5.912c-.563-.097-1.159.026-1.563.43L10.5 17.25H8.25v2.25H6v2.25H2.25v-2.818c0-.597.237-1.17.659-1.591l6.499-6.499c.404-.404.527-1 .43-1.563A6 6 0 1121.75 8.25z"/></svg>
  {:else if name === 'trending-up'}
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M2.25 18L9 11.25l4.306 4.307a11.95 11.95 0 015.814-5.519l2.74-1.22m0 0l-5.94-2.28m5.94 2.28l-2.28 5.941"/></svg>
  {:else if name === 'bell'}
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M14.857 17.082a23.848 23.848 0 005.454-1.31A8.967 8.967 0 0118 9.75v-.7V9A6 6 0 006 9v.75a8.967 8.967 0 01-2.312 6.022c1.733.64 3.56 1.085 5.455 1.31m5.714 0a24.255 24.255 0 01-5.714 0m5.714 0a3 3 0 11-5.714 0"/></svg>
  {:else if name === 'heart'}
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M21 8.25c0-2.485-2.099-4.5-4.688-4.5-1.935 0-3.597 1.126-4.312 2.733-.715-1.607-2.377-2.733-4.313-2.733C5.1 3.75 3 5.765 3 8.25c0 7.22 9 12 9 12s9-4.78 9-12z"/></svg>
  {:else if name === 'chat-bubble'}
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M20.25 8.511c.884.284 1.5 1.128 1.5 2.097v4.286c0 1.136-.847 2.1-1.98 2.193-.34.027-.68.052-1.02.072v3.091l-3-3c-1.354 0-2.694-.055-4.02-.163a2.115 2.115 0 01-.825-.242m9.345-8.334a2.126 2.126 0 00-.476-.095 48.64 48.64 0 00-8.048 0c-1.131.094-1.976 1.057-1.976 2.192v4.286c0 .837.46 1.58 1.155 1.951m9.345-8.334V6.637c0-1.621-1.152-3.026-2.76-3.235A48.455 48.455 0 0011.25 3c-2.115 0-4.198.137-6.24.402-1.608.209-2.76 1.614-2.76 3.235v6.226c0 1.621 1.152 3.026 2.76 3.235.577.075 1.157.14 1.74.194V21l4.155-4.155"/></svg>
  {:else if name === 'cog'}
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.324.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 011.37.49l1.296 2.247a1.125 1.125 0 01-.26 1.431l-1.003.827c-.293.24-.438.613-.431.992a6.759 6.759 0 010 .255c-.007.378.138.75.43.99l1.005.828c.424.35.534.954.26 1.43l-1.298 2.247a1.125 1.125 0 01-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.57 6.57 0 01-.22.128c-.331.183-.581.495-.644.869l-.213 1.28c-.09.543-.56.941-1.11.941h-2.594c-.55 0-1.02-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 01-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 01-1.369-.49l-1.297-2.247a1.125 1.125 0 01.26-1.431l1.004-.827c.292-.24.437-.613.43-.992a6.932 6.932 0 010-.255c.007-.378-.138-.75-.43-.99l-1.004-.828a1.125 1.125 0 01-.26-1.43l1.297-2.247a1.125 1.125 0 011.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.087.22-.128.332-.183.582-.495.644-.869l.214-1.281z"/><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"/></svg>
  {:else if name === 'users'}
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M15 19.128a9.38 9.38 0 002.625.372 9.337 9.337 0 004.121-.952 4.125 4.125 0 00-7.533-2.493M15 19.128v-.003c0-1.113-.285-2.16-.786-3.07M15 19.128v.106A12.318 12.318 0 018.624 21c-2.331 0-4.512-.645-6.374-1.766l-.001-.109a6.375 6.375 0 0111.964-3.07M12 6.375a3.375 3.375 0 11-6.75 0 3.375 3.375 0 016.75 0zm8.25 2.25a2.625 2.625 0 11-5.25 0 2.625 2.625 0 015.25 0z"/></svg>
  {:else if name === 'shield-check'}
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9 12.75L11.25 15 15 9.75m-3-7.036A11.959 11.959 0 013.598 6 11.99 11.99 0 003 9.749c0 5.592 3.824 10.29 9 11.623 5.176-1.332 9-6.03 9-11.622 0-1.31-.21-2.571-.598-3.751h-.152c-3.196 0-6.1-1.248-8.25-3.285z"/></svg>
  {:else if name === 'presentation-chart'}
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M3.75 3v11.25A2.25 2.25 0 006 16.5h2.25M3.75 3h-1.5m1.5 0h16.5m0 0h1.5m-1.5 0v11.25A2.25 2.25 0 0118 16.5h-2.25m-7.5 0h7.5m-7.5 0l-1 3m8.5-3l1 3m0 0l.5 1.5m-.5-1.5h-9.5m0 0l-.5 1.5M9 11.25v-5.5m3 5.5v-3.5m3 3.5v-1.5"/></svg>
  {:else if name === 'collection'}
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M3.75 6.75h4.5a.75.75 0 01.75.75v10.5a.75.75 0 01-.75.75H3.75m0-12v12m5.25-12h11.5A1.75 1.75 0 0121 8v8.5a1.75 1.75 0 01-1.75 1.75H8.999m-2.25 0h.001M8.999 5.25v13.5"/></svg>
  {/if}
{/snippet}

{#if mobileOpen}
  <div
    class="fixed inset-0 bg-black/50 z-40 lg:hidden backdrop-blur-sm transition-opacity"
    onclick={closeMobile}
    onkeydown={(e) => e.key === 'Escape' && closeMobile()}
    role="button"
    tabindex="-1"
    aria-label="Close navigation menu"
  ></div>
{/if}

<aside
  class="fixed lg:relative inset-y-0 left-0 z-50
         bg-white dark:bg-gray-900 text-gray-900 dark:text-white
         border-r border-gray-200 dark:border-gray-800
         transform transition-all duration-300 ease-in-out
         {mobileOpen ? 'translate-x-0' : '-translate-x-full lg:translate-x-0'}
         {collapsed ? 'lg:w-20' : 'lg:w-64'}
         w-64 shadow-lg lg:shadow-none flex flex-col"
  aria-label="Main navigation"
>
  <div class="h-16 flex items-center justify-between px-4 border-b border-gray-200 dark:border-gray-800 flex-shrink-0">
    <a href="/" class="flex items-center gap-3 group" onclick={closeMobile}>
      <div class="w-8 h-8 flex-shrink-0 transition-transform group-hover:scale-110">
        <svg viewBox="0 0 100 100" fill="none" class="w-full h-full" aria-hidden="true">
          <path d="M32 12 L18 45 L50 88 L82 45 L68 12 L58 45 L42 45 Z" class="fill-gray-900 dark:fill-white"/>
          <path d="M26 48 L74 48 L50 56 Z" class="fill-white dark:fill-gray-900"/>
          <path d="M46 58 L54 58 L50 82 Z" class="fill-white dark:fill-gray-900"/>
        </svg>
      </div>
      {#if !collapsed}
        <div class="flex flex-col">
          <span class="text-lg font-bold tracking-tight">FoxNIO</span>
          <span class="text-xs text-gray-500 dark:text-gray-400">AI API Gateway</span>
        </div>
      {/if}
    </a>
    <button
      class="lg:hidden p-2 rounded-lg text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
      onclick={closeMobile}
      aria-label="Close sidebar"
    >
      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
      </svg>
    </button>
  </div>

  <button
    class="hidden lg:flex absolute -right-3 top-20 w-6 h-6 bg-white dark:bg-gray-800 rounded-full
           items-center justify-center text-gray-500 dark:text-gray-400
           border border-gray-300 dark:border-gray-700
           hover:border-gray-400 dark:hover:border-gray-600
           hover:text-gray-700 dark:hover:text-gray-200
           transition-all shadow-sm hover:shadow"
    onclick={() => collapsed = !collapsed}
    aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
  >
    <svg class="w-4 h-4 transition-transform duration-300 {collapsed ? 'rotate-180' : ''}" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"/>
    </svg>
  </button>

  <nav class="flex-1 p-4 space-y-1 overflow-y-auto" aria-label="Primary">
    <div class="mb-4">
      {#if !collapsed}
        <div class="text-xs font-semibold text-gray-400 dark:text-gray-500 uppercase tracking-wider mb-2 px-3">Main Menu</div>
      {/if}
      {#each navItems as item}
        {@const active = isActive($page.url.pathname, item.href)}
        <a
          href={item.href}
          onclick={closeMobile}
          class="flex items-center px-3 py-2.5 rounded-lg transition-all group
                 {active ? 'bg-blue-50 dark:bg-blue-900/20 text-blue-600 dark:text-blue-400 font-medium' : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800'}
                 {collapsed ? 'justify-center' : ''}"
          title={collapsed ? item.label : undefined}
          aria-current={active ? 'page' : undefined}
        >
          <span class="flex-shrink-0 {collapsed ? '' : 'mr-3'}">
            {@render navIcon(item.icon)}
          </span>
          {#if !collapsed}<span class="truncate">{item.label}</span>{/if}
        </a>
      {/each}
    </div>

    <div class="pt-4 border-t border-gray-200 dark:border-gray-800">
      {#if !collapsed}
        <div class="text-xs font-semibold text-gray-400 dark:text-gray-500 uppercase tracking-wider mb-2 px-3">Admin</div>
      {/if}
      {#each adminItems as item}
        {@const active = isActive($page.url.pathname, item.href)}
        <a
          href={item.href}
          onclick={closeMobile}
          class="flex items-center px-3 py-2.5 rounded-lg transition-all group
                 {active ? 'bg-blue-50 dark:bg-blue-900/20 text-blue-600 dark:text-blue-400 font-medium' : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800'}
                 {collapsed ? 'justify-center' : ''}"
          title={collapsed ? item.label : undefined}
          aria-current={active ? 'page' : undefined}
        >
          <span class="flex-shrink-0 {collapsed ? '' : 'mr-3'}">
            {@render navIcon(item.icon)}
          </span>
          {#if !collapsed}<span class="truncate">{item.label}</span>{/if}
        </a>
      {/each}
    </div>
  </nav>

  {#if !collapsed}
    <div class="flex-shrink-0 p-4">
      <div class="bg-gray-50 dark:bg-gray-800/50 rounded-lg p-3 border border-gray-200 dark:border-gray-800">
        <div class="flex items-center gap-2 mb-1">
          <div class="w-4 h-4 flex-shrink-0">
            <svg viewBox="0 0 100 100" fill="none" class="w-full h-full" aria-hidden="true">
              <path d="M32 12 L18 45 L50 88 L82 45 L68 12 L58 45 L42 45 Z" class="fill-gray-900 dark:fill-white"/>
              <path d="M26 48 L74 48 L50 56 Z" class="fill-white dark:fill-gray-900"/>
              <path d="M46 58 L54 58 L50 82 Z" class="fill-white dark:fill-gray-900"/>
            </svg>
          </div>
          <span class="text-xs font-medium text-gray-700 dark:text-gray-300">FoxNIO</span>
        </div>
        <p class="text-xs text-gray-500 dark:text-gray-400">v0.1.0</p>
      </div>
    </div>
  {/if}
</aside>

<style>
  nav::-webkit-scrollbar { width: 6px; }
  nav::-webkit-scrollbar-track { background: transparent; }
  nav::-webkit-scrollbar-thumb { background: rgba(0, 0, 0, 0.1); border-radius: 3px; }
  nav::-webkit-scrollbar-thumb:hover { background: rgba(0, 0, 0, 0.2); }
  :global(.dark) nav::-webkit-scrollbar-thumb { background: rgba(255, 255, 255, 0.1); }
  :global(.dark) nav::-webkit-scrollbar-thumb:hover { background: rgba(255, 255, 255, 0.2); }
</style>
