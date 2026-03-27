<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';

  interface ApiKey {
    id: string;
    key: string;
    name: string;
    status: string;
    created_at: string;
    last_used_at: string | null;
  }

  let apiKeys: ApiKey[] = [];
  let filteredKeys: ApiKey[] = [];
  let loading = true;
  let showCreateModal = false;
  let newKeyName = '';
  let searchTerm = '';
  let copyingId: string | null = null;

  onMount(async () => {
    await loadApiKeys();
  });

  async function loadApiKeys() {
    try {
      const response = await fetch('/api/v1/user/apikeys');
      if (response.ok) {
        const data = await response.json();
        apiKeys = data.data || [];
        filterKeys();
      }
    } catch (e) {
      console.error('Failed to load API keys:', e);
    } finally {
      loading = false;
    }
  }

  function filterKeys() {
    if (!searchTerm.trim()) {
      filteredKeys = apiKeys;
    } else {
      const term = searchTerm.toLowerCase();
      filteredKeys = apiKeys.filter(key => 
        key.name.toLowerCase().includes(term) ||
        key.key.toLowerCase().includes(term)
      );
    }
  }

  $: if (searchTerm !== undefined) filterKeys();

  async function createKey() {
    if (!newKeyName.trim()) return;

    try {
      const response = await fetch('/api/v1/user/apikeys', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: newKeyName })
      });

      if (response.ok) {
        await loadApiKeys();
        showCreateModal = false;
        newKeyName = '';
      }
    } catch (e) {
      console.error('Failed to create API key:', e);
    }
  }

  async function deleteKey(id: string) {
    if (!confirm('Are you sure you want to delete this API key? This action cannot be undone.')) return;

    try {
      const response = await fetch(`/api/v1/user/apikeys/${id}`, {
        method: 'DELETE'
      });

      if (response.ok) {
        await loadApiKeys();
      }
    } catch (e) {
      console.error('Failed to delete API key:', e);
    }
  }

  async function copyToClipboard(text: string, id: string) {
    try {
      await navigator.clipboard.writeText(text);
      copyingId = id;
      setTimeout(() => copyingId = null, 2000);
    } catch (e) {
      console.error('Failed to copy:', e);
    }
  }
</script>

<div class="space-y-6">
  <!-- 页面标题 -->
  <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
    <div>
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">API Keys</h1>
      <p class="text-gray-500 dark:text-gray-400 mt-1">管理你的 API 密钥</p>
    </div>
    
    <button
      on:click={() => showCreateModal = true}
      class="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 
             transition-colors flex items-center gap-2 justify-center"
    >
      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"></path>
      </svg>
      Create New Key
    </button>
  </div>

  <!-- 搜索框 -->
  {#if apiKeys.length > 0}
    <div class="relative">
      <svg class="absolute left-3 top-1/2 transform -translate-y-1/2 w-5 h-5 text-gray-400" 
           fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
              d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"></path>
      </svg>
      <input
        type="text"
        bind:value={searchTerm}
        placeholder="Search by name or key..."
        class="w-full pl-10 pr-4 py-2 border border-gray-300 dark:border-gray-600 
               rounded-lg bg-white dark:bg-gray-800
               text-gray-900 dark:text-white
               focus:ring-2 focus:ring-blue-500 focus:border-transparent"
      />
    </div>
  {/if}

  <!-- 加载状态 -->
  {#if loading}
    <div class="flex items-center justify-center h-64">
      <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-500"></div>
    </div>
  {:else if apiKeys.length === 0}
    <!-- 空状态 -->
    <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-12 text-center">
      <div class="text-6xl mb-4">🔑</div>
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">No API Keys Yet</h3>
      <p class="text-gray-500 dark:text-gray-400 mb-6">Create your first API key to start using the API</p>
      <button
        on:click={() => showCreateModal = true}
        class="px-6 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-colors"
      >
        Create API Key
      </button>
    </div>
  {:else}
    <!-- 卡片列表（移动端） -->
    <div class="lg:hidden space-y-4">
      {#each filteredKeys as key}
        <div class="bg-white dark:bg-gray-800 rounded-lg shadow-sm border border-gray-200 dark:border-gray-700 p-4">
          <div class="flex items-start justify-between mb-3">
            <div class="flex-1 min-w-0">
              <h3 class="font-medium text-gray-900 dark:text-white truncate">{key.name || 'Unnamed'}</h3>
              <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
                {new Date(key.created_at).toLocaleDateString()}
              </p>
            </div>
            <span class="px-2 py-1 text-xs font-medium rounded-full 
                        {key.status === 'active' ? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400' : 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400'}">
              {key.status}
            </span>
          </div>
          
          <div class="flex items-center gap-2 mb-3">
            <code class="flex-1 text-sm bg-gray-100 dark:bg-gray-700 px-2 py-1 rounded font-mono truncate">
              {key.key}
            </code>
            <button
              on:click={() => copyToClipboard(key.key, key.id)}
              class="p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
              title="Copy to clipboard"
            >
              {#if copyingId === key.id}
                <svg class="w-5 h-5 text-green-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"></path>
                </svg>
              {:else}
                <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                        d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"></path>
                </svg>
              {/if}
            </button>
          </div>
          
          <button
            on:click={() => deleteKey(key.id)}
            class="w-full px-4 py-2 text-red-600 dark:text-red-400 
                   hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors
                   text-sm font-medium"
          >
            Delete
          </button>
        </div>
      {/each}
    </div>

    <!-- 表格（桌面端） -->
    <div class="hidden lg:block bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 overflow-hidden">
      <div class="overflow-x-auto">
        <table class="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
          <thead class="bg-gray-50 dark:bg-gray-700/50">
            <tr>
              <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                Name
              </th>
              <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                Key
              </th>
              <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                Status
              </th>
              <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                Created
              </th>
              <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                Actions
              </th>
            </tr>
          </thead>
          <tbody class="divide-y divide-gray-200 dark:divide-gray-700">
            {#each filteredKeys as key}
              <tr class="hover:bg-gray-50 dark:hover:bg-gray-700/50 transition-colors">
                <td class="px-6 py-4 whitespace-nowrap">
                  <span class="text-sm font-medium text-gray-900 dark:text-white">
                    {key.name || 'Unnamed'}
                  </span>
                </td>
                <td class="px-6 py-4 whitespace-nowrap">
                  <div class="flex items-center gap-2">
                    <code class="text-sm bg-gray-100 dark:bg-gray-700 px-2 py-1 rounded font-mono">
                      {key.key}
                    </code>
                    <button
                      on:click={() => copyToClipboard(key.key, key.id)}
                      class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
                      title="Copy to clipboard"
                    >
                      {#if copyingId === key.id}
                        <svg class="w-4 h-4 text-green-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"></path>
                        </svg>
                      {:else}
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                                d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"></path>
                        </svg>
                      {/if}
                    </button>
                  </div>
                </td>
                <td class="px-6 py-4 whitespace-nowrap">
                  <span class="px-2 py-1 text-xs font-medium rounded-full 
                              {key.status === 'active' ? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400' : 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400'}">
                    {key.status}
                  </span>
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400">
                  {new Date(key.created_at).toLocaleDateString()}
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm">
                  <button
                    on:click={() => deleteKey(key.id)}
                    class="text-red-600 dark:text-red-400 hover:text-red-900 dark:hover:text-red-300 
                           transition-colors font-medium"
                  >
                    Delete
                  </button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>
  {/if}
</div>

<!-- 创建模态框 -->
{#if showCreateModal}
  <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4" 
       on:keydown={(e) => e.key === 'Escape' && (showCreateModal = false)}
       role="dialog"
       aria-modal="true"
       aria-labelledby="modal-title">
    <div class="bg-white dark:bg-gray-800 rounded-xl max-w-md w-full p-6 shadow-xl">
      <h2 id="modal-title" class="text-xl font-bold text-gray-900 dark:text-white mb-4">
        Create New API Key
      </h2>
      
      <div class="mb-4">
        <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
          Name (optional)
        </label>
        <input
          type="text"
          bind:value={newKeyName}
          on:keydown={(e) => e.key === 'Enter' && createKey()}
          class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 
                 rounded-lg bg-white dark:bg-gray-700
                 text-gray-900 dark:text-white
                 focus:ring-2 focus:ring-blue-500 focus:border-transparent"
          placeholder="e.g., Production Key"
        />
      </div>

      <div class="flex justify-end gap-3">
        <button
          on:click={() => { showCreateModal = false; newKeyName = ''; }}
          class="px-4 py-2 border border-gray-300 dark:border-gray-600 
                 rounded-lg text-gray-700 dark:text-gray-300
                 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
        >
          Cancel
        </button>
        <button
          on:click={createKey}
          class="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-colors"
        >
          Create
        </button>
      </div>
    </div>
  </div>
{/if}
