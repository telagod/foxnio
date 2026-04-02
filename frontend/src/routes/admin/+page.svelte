<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { api, type DashboardStats } from '$lib/api';

  let stats: DashboardStats = {
    total_users: 0,
    total_accounts: 0,
    total_requests_today: 0,
    total_revenue: 0,
    active_users: 0,
    active_accounts: 0
  };

  let loading = true;
  let error: string | null = null;

  onMount(() => {
    // 从 localStorage 恢复 token
    const token = localStorage.getItem('token');
    if (token) api.setToken(token);
    
    loadStats();
    // 每 30 秒刷新一次
    const interval = setInterval(loadStats, 30000);
    return () => clearInterval(interval);
  });

  async function loadStats() {
    try {
      stats = await api.getAdminStats();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Network error';
      console.error('Failed to load stats:', e);
    } finally {
      loading = false;
    }
  }

  function formatNumber(num: number): string {
    if (num >= 1000000) return (num / 1000000).toFixed(1) + 'M';
    if (num >= 1000) return (num / 1000).toFixed(1) + 'K';
    return num.toString();
  }

  function formatCurrency(cents: number): string {
    return '$' + (cents / 100).toFixed(2);
  }
</script>

<div class="space-y-6">
  <!-- 页面标题 -->
  <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
    <div>
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Dashboard</h1>
      <p class="text-gray-500 dark:text-gray-400 mt-1">系统概览和关键指标</p>
    </div>
    
    <button
      on:click={loadStats}
      disabled={loading}
      class="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 
             disabled:opacity-50 disabled:cursor-not-allowed transition-colors
             flex items-center gap-2"
    >
      <svg class="w-4 h-4 {loading ? 'animate-spin' : ''}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
              d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"></path>
      </svg>
      Refresh
    </button>
  </div>

  <!-- 错误提示 -->
  {#if error}
    <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 
                rounded-lg p-4 flex items-start gap-3">
      <svg class="w-5 h-5 text-red-500 mt-0.5 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
        <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clip-rule="evenodd"></path>
      </svg>
      <div>
        <h3 class="text-sm font-medium text-red-800 dark:text-red-200">Error loading data</h3>
        <p class="text-sm text-red-700 dark:text-red-300 mt-1">{error}</p>
      </div>
    </div>
  {/if}

  <!-- 加载状态 -->
  {#if loading}
    <div class="flex items-center justify-center h-64">
      <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-500"></div>
    </div>
  {:else}
    <!-- 统计卡片 -->
    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 lg:gap-6">
      <!-- 总用户 -->
      <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-6
                  hover:shadow-md transition-shadow">
        <div class="flex items-center justify-between">
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-gray-500 dark:text-gray-400 truncate">Total Users</p>
            <p class="text-2xl lg:text-3xl font-bold text-gray-900 dark:text-white mt-1">
              {formatNumber(stats.total_users)}
            </p>
          </div>
          <div class="flex-shrink-0 w-12 h-12 bg-blue-100 dark:bg-blue-900/30 rounded-full 
                      flex items-center justify-center">
            <svg class="w-6 h-6 text-blue-600 dark:text-blue-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                    d="M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197m13.5-9a2.5 2.5 0 11-5 0 2.5 2.5 0 015 0z"></path>
            </svg>
          </div>
        </div>
      </div>

      <!-- 活跃账号 -->
      <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-6
                  hover:shadow-md transition-shadow">
        <div class="flex items-center justify-between">
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-gray-500 dark:text-gray-400 truncate">Active Accounts</p>
            <p class="text-2xl lg:text-3xl font-bold text-gray-900 dark:text-white mt-1">
              {formatNumber(stats.active_accounts)}
            </p>
          </div>
          <div class="flex-shrink-0 w-12 h-12 bg-green-100 dark:bg-green-900/30 rounded-full 
                      flex items-center justify-center">
            <svg class="w-6 h-6 text-green-600 dark:text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                    d="M5 12h14M5 12a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v4a2 2 0 01-2 2M5 12a2 2 0 00-2 2v4a2 2 0 002 2h14a2 2 0 002-2v-4a2 2 0 00-2-2m-2-4h.01M17 16h.01"></path>
            </svg>
          </div>
        </div>
      </div>

      <!-- 今日请求 -->
      <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-6
                  hover:shadow-md transition-shadow">
        <div class="flex items-center justify-between">
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-gray-500 dark:text-gray-400 truncate">Requests Today</p>
            <p class="text-2xl lg:text-3xl font-bold text-gray-900 dark:text-white mt-1">
              {formatNumber(stats.total_requests_today)}
            </p>
          </div>
          <div class="flex-shrink-0 w-12 h-12 bg-purple-100 dark:bg-purple-900/30 rounded-full 
                      flex items-center justify-center">
            <svg class="w-6 h-6 text-purple-600 dark:text-purple-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                    d="M13 10V3L4 14h7v7l9-11h-7z"></path>
            </svg>
          </div>
        </div>
      </div>

      <!-- 收入 -->
      <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-6
                  hover:shadow-md transition-shadow">
        <div class="flex items-center justify-between">
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-gray-500 dark:text-gray-400 truncate">Revenue</p>
            <p class="text-2xl lg:text-3xl font-bold text-gray-900 dark:text-white mt-1">
              {formatCurrency(stats.total_revenue)}
            </p>
          </div>
          <div class="flex-shrink-0 w-12 h-12 bg-yellow-100 dark:bg-yellow-900/30 rounded-full 
                      flex items-center justify-center">
            <svg class="w-6 h-6 text-yellow-600 dark:text-yellow-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                    d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path>
            </svg>
          </div>
        </div>
      </div>

      <!-- 活跃用户 -->
      <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-6
                  hover:shadow-md transition-shadow">
        <div class="flex items-center justify-between">
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-gray-500 dark:text-gray-400 truncate">Active Users</p>
            <p class="text-2xl lg:text-3xl font-bold text-gray-900 dark:text-white mt-1">
              {formatNumber(stats.active_users)}
            </p>
          </div>
          <div class="flex-shrink-0 w-12 h-12 bg-indigo-100 dark:bg-indigo-900/30 rounded-full 
                      flex items-center justify-center">
            <svg class="w-6 h-6 text-indigo-600 dark:text-indigo-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                    d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"></path>
            </svg>
          </div>
        </div>
      </div>

      <!-- 总账号 -->
      <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-6
                  hover:shadow-md transition-shadow">
        <div class="flex items-center justify-between">
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-gray-500 dark:text-gray-400 truncate">Total Accounts</p>
            <p class="text-2xl lg:text-3xl font-bold text-gray-900 dark:text-white mt-1">
              {formatNumber(stats.total_accounts)}
            </p>
          </div>
          <div class="flex-shrink-0 w-12 h-12 bg-red-100 dark:bg-red-900/30 rounded-full 
                      flex items-center justify-center">
            <svg class="w-6 h-6 text-red-600 dark:text-red-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                    d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"></path>
            </svg>
          </div>
        </div>
      </div>
    </div>

    <!-- 快捷操作 -->
    <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-6">
      <h2 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">Quick Actions</h2>
      <div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
        <a href="/admin/users" 
           class="flex flex-col items-center justify-center p-4 bg-gray-50 dark:bg-gray-700/50 
                  rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors group">
          <span class="text-2xl mb-2 group-hover:scale-110 transition-transform">👥</span>
          <span class="text-sm font-medium text-gray-700 dark:text-gray-300">Manage Users</span>
        </a>
        <a href="/admin/accounts" 
           class="flex flex-col items-center justify-center p-4 bg-gray-50 dark:bg-gray-700/50 
                  rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors group">
          <span class="text-2xl mb-2 group-hover:scale-110 transition-transform">🔑</span>
          <span class="text-sm font-medium text-gray-700 dark:text-gray-300">Manage Accounts</span>
        </a>
        <a href="/apikeys" 
           class="flex flex-col items-center justify-center p-4 bg-gray-50 dark:bg-gray-700/50 
                  rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors group">
          <span class="text-2xl mb-2 group-hover:scale-110 transition-transform">🗝️</span>
          <span class="text-sm font-medium text-gray-700 dark:text-gray-300">API Keys</span>
        </a>
        <a href="/playground" 
           class="flex flex-col items-center justify-center p-4 bg-gray-50 dark:bg-gray-700/50 
                  rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors group">
          <span class="text-2xl mb-2 group-hover:scale-110 transition-transform">💬</span>
          <span class="text-sm font-medium text-gray-700 dark:text-gray-300">Playground</span>
        </a>
      </div>
    </div>
  {/if}
</div>
