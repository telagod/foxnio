<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type ApiKey } from '$lib/api';

  let apiKeys: ApiKey[] = $state([]);
  let loading = $state(true);
  let showCreateModal = $state(false);
  let newKeyName = $state('');
  let searchTerm = $state('');
  let copyingId: string | null = $state(null);

  let filtered = $derived.by(() => {
    if (!searchTerm.trim()) return apiKeys;
    const term = searchTerm.toLowerCase();
    return apiKeys.filter(key =>
      (key.name || '').toLowerCase().includes(term) ||
      key.key.toLowerCase().includes(term)
    );
  });

  onMount(async () => {
    const token = localStorage.getItem('token');
    if (token) api.setToken(token);
    await loadApiKeys();
  });

  async function loadApiKeys() {
    try {
      const data = await api.listApiKeys();
      apiKeys = data.data || [];
    } catch (e) {
      console.error('Failed to load API keys:', e);
    } finally {
      loading = false;
    }
  }

  async function createKey() {
    if (!newKeyName.trim()) return;
    try {
      await api.createApiKey(newKeyName);
      await loadApiKeys();
      showCreateModal = false;
      newKeyName = '';
    } catch (e) {
      console.error('Failed to create API key:', e);
    }
  }

  async function deleteKey(id: string) {
    if (!confirm('Are you sure you want to delete this API key? This action cannot be undone.')) return;
    try {
      await api.deleteApiKey(id);
      await loadApiKeys();
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

  function handleModalKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      showCreateModal = false;
      newKeyName = '';
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      showCreateModal = false;
      newKeyName = '';
    }
  }
</script>

<svelte:head>
  <title>API Keys - FoxNIO</title>
</svelte:head>

<div class="space-y-6">
  <!-- Header -->
  <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
    <div class="flex items-center gap-3">
      <div class="p-2 bg-amber-100 dark:bg-amber-900/30 rounded-lg">
        <svg class="w-6 h-6 text-amber-600 dark:text-amber-400" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M15.75 5.25a3 3 0 0 1 3 3m3 0a6 6 0 0 1-7.029 5.912c-.563-.097-1.159.026-1.563.43L10.5 17.25H8.25v2.25H6v2.25H2.25v-2.818c0-.597.237-1.17.659-1.591l6.499-6.499c.404-.404.527-1 .43-1.563A6 6 0 1 1 21.75 8.25z" />
        </svg>
      </div>
      <div>
        <h1 class="text-2xl font-bold text-gray-900 dark:text-white">API Keys</h1>
        <p class="text-sm text-gray-500 dark:text-gray-400 mt-0.5">Manage your API credentials</p>
      </div>
    </div>

    <button
      onclick={() => showCreateModal = true}
      class="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium
             bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors
             focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 dark:focus:ring-offset-gray-900"
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M12 4v16m8-8H4" />
      </svg>
      Create New Key
    </button>
  </div>

  <!-- Search -->
  {#if apiKeys.length > 0}
    <div class="relative">
      <svg class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400 dark:text-gray-500 pointer-events-none"
           fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
        <path stroke-linecap="round" stroke-linejoin="round" d="m21 21-5.197-5.197m0 0A7.5 7.5 0 1 0 5.196 5.196a7.5 7.5 0 0 0 10.607 10.607z" />
      </svg>
      <label for="apikey-search" class="sr-only">Search API keys</label>
      <input
        id="apikey-search"
        type="text"
        bind:value={searchTerm}
        placeholder="Search by name or key..."
        class="w-full pl-10 pr-4 py-2.5 text-sm border border-gray-300 dark:border-gray-600
               rounded-lg bg-white dark:bg-gray-800
               text-gray-900 dark:text-white
               placeholder-gray-400 dark:placeholder-gray-500
               focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
      />
    </div>
  {/if}

  <!-- Loading -->
  {#if loading}
    <div class="flex items-center justify-center h-64">
      <div class="animate-spin rounded-full h-10 w-10 border-2 border-gray-200 dark:border-gray-700 border-t-blue-500"></div>
    </div>
  {:else if apiKeys.length === 0}
    <!-- Empty State -->
    <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-12 text-center">
      <div class="flex justify-center mb-4">
        <div class="p-4 bg-amber-50 dark:bg-amber-900/20 rounded-full">
          <svg class="w-10 h-10 text-amber-500 dark:text-amber-400" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
            <path stroke-linecap="round" stroke-linejoin="round" d="M15.75 5.25a3 3 0 0 1 3 3m3 0a6 6 0 0 1-7.029 5.912c-.563-.097-1.159.026-1.563.43L10.5 17.25H8.25v2.25H6v2.25H2.25v-2.818c0-.597.237-1.17.659-1.591l6.499-6.499c.404-.404.527-1 .43-1.563A6 6 0 1 1 21.75 8.25z" />
          </svg>
        </div>
      </div>
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">No API Keys Yet</h3>
      <p class="text-sm text-gray-500 dark:text-gray-400 mb-6 max-w-sm mx-auto">Create your first API key to start using the API</p>
      <button
        onclick={() => showCreateModal = true}
        class="px-5 py-2 text-sm font-medium bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors
               focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 dark:focus:ring-offset-gray-800"
      >
        Create API Key
      </button>
    </div>
  {:else}
    <!-- Mobile Cards -->
    <div class="lg:hidden space-y-3">
      {#each filtered as key}
        <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-4">
          <div class="flex items-start justify-between mb-3">
            <div class="flex-1 min-w-0">
              <h3 class="text-sm font-semibold text-gray-900 dark:text-white truncate">{key.name || 'Unnamed'}</h3>
              <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
                {new Date(key.created_at).toLocaleDateString()}
              </p>
            </div>
            <span class="ml-2 shrink-0 px-2 py-0.5 text-xs font-medium rounded-full
                        {key.status === 'active'
                          ? 'bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400'
                          : 'bg-red-50 text-red-700 dark:bg-red-900/30 dark:text-red-400'}">
              {key.status}
            </span>
          </div>

          <div class="flex items-center gap-2 mb-3">
            <code class="flex-1 text-xs bg-gray-50 dark:bg-gray-700/50 border border-gray-200 dark:border-gray-600 px-2.5 py-1.5 rounded-md font-mono truncate text-gray-700 dark:text-gray-300">
              {key.key}
            </code>
            <button
              onclick={() => copyToClipboard(key.key, key.id)}
              class="shrink-0 p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors rounded-md hover:bg-gray-100 dark:hover:bg-gray-700"
              aria-label="Copy API key to clipboard"
            >
              {#if copyingId === key.id}
                <svg class="w-4 h-4 text-emerald-500" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5" />
                </svg>
              {:else}
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M15.666 3.888A2.25 2.25 0 0 0 13.5 2.25h-3c-1.03 0-1.9.693-2.166 1.638m7.332 0c.055.194.084.4.084.612v0a.75.75 0 0 1-.75.75H9.75a.75.75 0 0 1-.75-.75v0c0-.212.03-.418.084-.612m7.332 0c.646.049 1.288.11 1.927.184 1.1.128 1.907 1.077 1.907 2.185V19.5a2.25 2.25 0 0 1-2.25 2.25H6.75A2.25 2.25 0 0 1 4.5 19.5V6.257c0-1.108.806-2.057 1.907-2.185a48.208 48.208 0 0 1 1.927-.184" />
                </svg>
              {/if}
            </button>
          </div>

          <button
            onclick={() => deleteKey(key.id)}
            class="w-full px-3 py-2 text-sm font-medium text-red-600 dark:text-red-400
                   hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors
                   focus:outline-none focus:ring-2 focus:ring-red-500"
            aria-label="Delete API key {key.name || 'Unnamed'}"
          >
            Delete
          </button>
        </div>
      {/each}
    </div>

    <!-- Desktop Table -->
    <div class="hidden lg:block bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 overflow-hidden">
      <div class="overflow-x-auto">
        <table class="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
          <thead>
            <tr class="bg-gray-50 dark:bg-gray-700/50">
              <th scope="col" class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">Name</th>
              <th scope="col" class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">Key</th>
              <th scope="col" class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">Status</th>
              <th scope="col" class="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">Created</th>
              <th scope="col" class="px-6 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">Actions</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-gray-100 dark:divide-gray-700">
            {#each filtered as key}
              <tr class="hover:bg-gray-50 dark:hover:bg-gray-700/30 transition-colors">
                <td class="px-6 py-4 whitespace-nowrap">
                  <span class="text-sm font-medium text-gray-900 dark:text-white">{key.name || 'Unnamed'}</span>
                </td>
                <td class="px-6 py-4 whitespace-nowrap">
                  <div class="flex items-center gap-2">
                    <code class="text-xs bg-gray-50 dark:bg-gray-700/50 border border-gray-200 dark:border-gray-600 px-2 py-1 rounded font-mono text-gray-700 dark:text-gray-300">
                      {key.key}
                    </code>
                    <button
                      onclick={() => copyToClipboard(key.key, key.id)}
                      class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-700"
                      aria-label="Copy API key to clipboard"
                    >
                      {#if copyingId === key.id}
                        <svg class="w-3.5 h-3.5 text-emerald-500" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                          <path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5" />
                        </svg>
                      {:else}
                        <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
                          <path stroke-linecap="round" stroke-linejoin="round" d="M15.666 3.888A2.25 2.25 0 0 0 13.5 2.25h-3c-1.03 0-1.9.693-2.166 1.638m7.332 0c.055.194.084.4.084.612v0a.75.75 0 0 1-.75.75H9.75a.75.75 0 0 1-.75-.75v0c0-.212.03-.418.084-.612m7.332 0c.646.049 1.288.11 1.927.184 1.1.128 1.907 1.077 1.907 2.185V19.5a2.25 2.25 0 0 1-2.25 2.25H6.75A2.25 2.25 0 0 1 4.5 19.5V6.257c0-1.108.806-2.057 1.907-2.185a48.208 48.208 0 0 1 1.927-.184" />
                        </svg>
                      {/if}
                    </button>
                  </div>
                </td>
                <td class="px-6 py-4 whitespace-nowrap">
                  <span class="px-2 py-0.5 text-xs font-medium rounded-full
                              {key.status === 'active'
                                ? 'bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400'
                                : 'bg-red-50 text-red-700 dark:bg-red-900/30 dark:text-red-400'}">
                    {key.status}
                  </span>
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500 dark:text-gray-400">
                  {new Date(key.created_at).toLocaleDateString()}
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-right">
                  <button
                    onclick={() => deleteKey(key.id)}
                    class="text-sm font-medium text-red-600 dark:text-red-400 hover:text-red-800 dark:hover:text-red-300 transition-colors
                           focus:outline-none focus:underline"
                    aria-label="Delete API key {key.name || 'Unnamed'}"
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

<!-- Create Modal -->
{#if showCreateModal}
  <div
    class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 p-4"
    onclick={handleBackdropClick}
    onkeydown={handleModalKeydown}
    role="dialog"
    aria-modal="true"
    aria-labelledby="apikey-modal-title"
    tabindex="-1"
  >
    <div class="bg-white dark:bg-gray-800 rounded-xl max-w-md w-full p-6 shadow-xl border border-gray-200 dark:border-gray-700">
      <h2 id="apikey-modal-title" class="text-lg font-semibold text-gray-900 dark:text-white mb-4">
        Create New API Key
      </h2>

      <div class="mb-5">
        <label for="apikey-name-input" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">
          Name (optional)
        </label>
        <input
          id="apikey-name-input"
          type="text"
          bind:value={newKeyName}
          onkeydown={(e) => e.key === 'Enter' && createKey()}
          class="w-full px-3 py-2 text-sm border border-gray-300 dark:border-gray-600
                 rounded-lg bg-white dark:bg-gray-700
                 text-gray-900 dark:text-white
                 placeholder-gray-400 dark:placeholder-gray-500
                 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
          placeholder="e.g., Production Key"
        />
      </div>

      <div class="flex justify-end gap-3">
        <button
          onclick={() => { showCreateModal = false; newKeyName = ''; }}
          class="px-4 py-2 text-sm font-medium border border-gray-300 dark:border-gray-600
                 rounded-lg text-gray-700 dark:text-gray-300
                 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors
                 focus:outline-none focus:ring-2 focus:ring-gray-400"
        >
          Cancel
        </button>
        <button
          onclick={createKey}
          class="px-4 py-2 text-sm font-medium bg-blue-600 text-white rounded-lg
                 hover:bg-blue-700 transition-colors
                 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 dark:focus:ring-offset-gray-800"
        >
          Create
        </button>
      </div>
    </div>
  </div>
{/if}
