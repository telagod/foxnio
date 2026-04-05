<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type AlertRule } from '$lib/api';

  let alertRules: AlertRule[] = $state([]);
  let loading = $state(true);
  let showCreateModal = $state(false);
  let editingRule: AlertRule | null = $state(null);

  let newRule = $state({
    name: '',
    type: 'usage' as const,
    threshold: 1000,
    operator: 'gt' as const,
    duration_minutes: 5,
    channels: [] as ('email' | 'webhook' | 'slack')[],
    enabled: true,
  });

  onMount(async () => {
    await loadAlertRules();
  });

  async function loadAlertRules() {
    try {
      const data = await api.listAlertRules();
      alertRules = data.rules || [];
    } catch (e) {
      console.error('Failed to load alert rules:', e);
    } finally {
      loading = false;
    }
  }

  async function createRule() {
    try {
      await api.createAlertRule(newRule);
      await loadAlertRules();
      showCreateModal = false;
      resetForm();
    } catch (e) {
      console.error('Failed to create alert rule:', e);
    }
  }

  async function updateRule(rule: AlertRule) {
    try {
      await api.updateAlertRule(rule.id, rule);
      await loadAlertRules();
      editingRule = null;
    } catch (e) {
      console.error('Failed to update alert rule:', e);
    }
  }

  async function deleteRule(id: string) {
    if (!confirm('Are you sure you want to delete this alert rule?')) return;
    try {
      await api.deleteAlertRule(id);
      await loadAlertRules();
    } catch (e) {
      console.error('Failed to delete alert rule:', e);
    }
  }

  async function toggleRule(rule: AlertRule) {
    rule.enabled = !rule.enabled;
    await updateRule(rule);
  }

  function resetForm() {
    newRule = {
      name: '',
      type: 'usage',
      threshold: 1000,
      operator: 'gt',
      duration_minutes: 5,
      channels: [],
      enabled: true,
    };
  }

  function getAlertTypeLabel(type: string): string {
    const labels: Record<string, string> = {
      usage: 'API Usage',
      balance: 'Account Balance',
      error_rate: 'Error Rate',
      latency: 'Response Latency',
    };
    return labels[type] || type;
  }

  function getOperatorLabel(operator: string): string {
    const labels: Record<string, string> = {
      gt: '>',
      lt: '<',
      eq: '=',
      gte: '>=',
      lte: '<=',
    };
    return labels[operator] || operator;
  }

  function handleModalKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      showCreateModal = false;
      resetForm();
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      showCreateModal = false;
      resetForm();
    }
  }

  function toggleChannel(channel: 'email' | 'webhook' | 'slack') {
    if (newRule.channels.includes(channel)) {
      newRule.channels = newRule.channels.filter(c => c !== channel);
    } else {
      newRule.channels = [...newRule.channels, channel];
    }
  }
</script>

<svelte:head>
  <title>Alert Rules - FoxNIO</title>
</svelte:head>

<div class="space-y-6">
  <!-- Header -->
  <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
    <div class="flex items-center gap-3">
      <div class="p-2 bg-orange-100 dark:bg-orange-900/30 rounded-lg">
        <svg class="w-6 h-6 text-orange-600 dark:text-orange-400" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M14.857 17.082a23.848 23.848 0 0 0 5.454-1.31A8.967 8.967 0 0 1 18 9.75V9A6 6 0 0 0 6 9v.75a8.967 8.967 0 0 1-2.312 6.022c1.733.64 3.56 1.085 5.455 1.31m5.714 0a24.255 24.255 0 0 1-5.714 0m5.714 0a3 3 0 1 1-5.714 0" />
        </svg>
      </div>
      <div>
        <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Alert Rules</h1>
        <p class="text-sm text-gray-500 dark:text-gray-400 mt-0.5">Configure alerts for your API usage and account</p>
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
      Create Alert Rule
    </button>
  </div>

  <!-- Loading -->
  {#if loading}
    <div class="flex items-center justify-center h-64">
      <div class="animate-spin rounded-full h-10 w-10 border-2 border-gray-200 dark:border-gray-700 border-t-blue-500"></div>
    </div>
  {:else if alertRules.length === 0}
    <!-- Empty State -->
    <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-12 text-center">
      <div class="flex justify-center mb-4">
        <div class="p-4 bg-orange-50 dark:bg-orange-900/20 rounded-full">
          <svg class="w-10 h-10 text-orange-400 dark:text-orange-500" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
            <path stroke-linecap="round" stroke-linejoin="round" d="M14.857 17.082a23.848 23.848 0 0 0 5.454-1.31A8.967 8.967 0 0 1 18 9.75V9A6 6 0 0 0 6 9v.75a8.967 8.967 0 0 1-2.312 6.022c1.733.64 3.56 1.085 5.455 1.31m5.714 0a24.255 24.255 0 0 1-5.714 0m5.714 0a3 3 0 1 1-5.714 0" />
          </svg>
        </div>
      </div>
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">No Alert Rules Yet</h3>
      <p class="text-sm text-gray-500 dark:text-gray-400 mb-6 max-w-sm mx-auto">Create your first alert rule to get notified about important events</p>
      <button
        onclick={() => showCreateModal = true}
        class="px-5 py-2 text-sm font-medium bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors
               focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 dark:focus:ring-offset-gray-800"
      >
        Create Alert Rule
      </button>
    </div>
  {:else}
    <!-- Alert Rules List -->
    <div class="space-y-3">
      {#each alertRules as rule}
        <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-5">
          <div class="flex flex-col sm:flex-row sm:items-start sm:justify-between gap-4">
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-3 mb-2">
                <h3 class="text-base font-semibold text-gray-900 dark:text-white truncate">{rule.name}</h3>
                <span class="shrink-0 px-2 py-0.5 text-xs font-medium rounded-full
                            {rule.enabled
                              ? 'bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400'
                              : 'bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-400'}">
                  {rule.enabled ? 'Active' : 'Disabled'}
                </span>
              </div>

              <div class="flex flex-wrap items-center gap-x-4 gap-y-1.5 text-sm text-gray-600 dark:text-gray-400">
                <span class="inline-flex items-center gap-1.5">
                  <svg class="w-3.5 h-3.5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M9.568 3H5.25A2.25 2.25 0 0 0 3 5.25v4.318c0 .597.237 1.17.659 1.591l9.581 9.581c.699.699 1.78.872 2.607.33a18.095 18.095 0 0 0 5.223-5.223c.542-.827.369-1.908-.33-2.607L11.16 3.66A2.25 2.25 0 0 0 9.568 3z" />
                    <path stroke-linecap="round" stroke-linejoin="round" d="M6 6h.008v.008H6V6z" />
                  </svg>
                  {getAlertTypeLabel(rule.type)}
                </span>
                <span class="inline-flex items-center gap-1.5">
                  <svg class="w-3.5 h-3.5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M10.5 6h9.75M10.5 6a1.5 1.5 0 1 1-3 0m3 0a1.5 1.5 0 1 0-3 0M3.75 6H7.5m3 12h9.75m-9.75 0a1.5 1.5 0 0 1-3 0m3 0a1.5 1.5 0 0 0-3 0m-3.75 0H7.5m9-6h3.75m-3.75 0a1.5 1.5 0 0 1-3 0m3 0a1.5 1.5 0 0 0-3 0m-9.75 0h9.75" />
                  </svg>
                  {getOperatorLabel(rule.operator)} {rule.threshold}
                </span>
                <span class="inline-flex items-center gap-1.5">
                  <svg class="w-3.5 h-3.5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M12 6v6h4.5m4.5 0a9 9 0 1 1-18 0 9 9 0 0 1 18 0z" />
                  </svg>
                  {rule.duration_minutes} min
                </span>
                <span class="inline-flex items-center gap-1.5">
                  <svg class="w-3.5 h-3.5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M7.5 8.25h9m-9 3H12m-9.75 1.51c0 1.6 1.123 2.994 2.707 3.227 1.129.166 2.27.293 3.423.379.35.026.67.21.865.501L12 21l2.755-4.133a1.14 1.14 0 0 1 .865-.501 48.172 48.172 0 0 0 3.423-.379c1.584-.233 2.707-1.626 2.707-3.228V6.741c0-1.602-1.123-2.995-2.707-3.228A48.394 48.394 0 0 0 12 3c-2.392 0-4.744.175-7.043.513C3.373 3.746 2.25 5.14 2.25 6.741v6.018z" />
                  </svg>
                  {rule.channels.join(', ')}
                </span>
              </div>

              {#if rule.last_triggered_at}
                <p class="text-xs text-gray-400 dark:text-gray-500 mt-2">
                  Last triggered: {new Date(rule.last_triggered_at).toLocaleString()}
                </p>
              {/if}
            </div>

            <div class="flex items-center gap-1.5 shrink-0">
              <button
                onclick={() => toggleRule(rule)}
                class="relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 dark:focus:ring-offset-gray-800
                       {rule.enabled ? 'bg-blue-600' : 'bg-gray-300 dark:bg-gray-600'}"
                role="switch"
                aria-checked={rule.enabled}
                aria-label="Toggle rule {rule.name}"
              >
                <span class="inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform
                            {rule.enabled ? 'translate-x-6' : 'translate-x-1'}"></span>
              </button>

              <button
                onclick={() => editingRule = rule}
                class="p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 rounded-md hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors
                       focus:outline-none focus:ring-2 focus:ring-blue-500"
                aria-label="Edit rule {rule.name}"
              >
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
                  <path stroke-linecap="round" stroke-linejoin="round" d="m16.862 4.487 1.687-1.688a1.875 1.875 0 1 1 2.652 2.652L10.582 16.07a4.5 4.5 0 0 1-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 0 1 1.13-1.897l8.932-8.931zm0 0L19.5 7.125M18 14v4.75A2.25 2.25 0 0 1 15.75 21H5.25A2.25 2.25 0 0 1 3 18.75V8.25A2.25 2.25 0 0 1 5.25 6H10" />
                </svg>
              </button>

              <button
                onclick={() => deleteRule(rule.id)}
                class="p-2 text-red-400 hover:text-red-600 dark:hover:text-red-300 rounded-md hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors
                       focus:outline-none focus:ring-2 focus:ring-red-500"
                aria-label="Delete rule {rule.name}"
              >
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
                  <path stroke-linecap="round" stroke-linejoin="round" d="m14.74 9-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 0 1-2.244 2.077H8.084a2.25 2.25 0 0 1-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 0 0-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 0 1 3.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 0 0-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 0 0-7.5 0" />
                </svg>
              </button>
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}

  <!-- Info Panel -->
  <div class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-xl p-4">
    <div class="flex items-start gap-3">
      <svg class="w-5 h-5 text-blue-500 dark:text-blue-400 mt-0.5 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
        <path stroke-linecap="round" stroke-linejoin="round" d="m11.25 11.25.041-.02a.75.75 0 0 1 1.063.852l-.708 2.836a.75.75 0 0 0 1.063.853l.041-.021M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0zm-9-3.75h.008v.008H12V8.25z" />
      </svg>
      <div>
        <h3 class="text-sm font-semibold text-blue-900 dark:text-blue-200 mb-1.5">Alert Types</h3>
        <ul class="text-sm text-blue-800 dark:text-blue-300 space-y-1">
          <li><span class="font-medium">API Usage:</span> Alert when API request count exceeds threshold</li>
          <li><span class="font-medium">Account Balance:</span> Alert when account balance falls below threshold</li>
          <li><span class="font-medium">Error Rate:</span> Alert when error rate exceeds threshold percentage</li>
          <li><span class="font-medium">Response Latency:</span> Alert when average response time exceeds threshold (ms)</li>
        </ul>
      </div>
    </div>
  </div>
</div>

<!-- Create Modal -->
{#if showCreateModal}
  <div
    class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 p-4"
    onclick={handleBackdropClick}
    onkeydown={handleModalKeydown}
    role="dialog"
    aria-modal="true"
    aria-labelledby="alert-modal-title"
    tabindex="-1"
  >
    <div class="bg-white dark:bg-gray-800 rounded-xl max-w-md w-full p-6 shadow-xl border border-gray-200 dark:border-gray-700 max-h-[90vh] overflow-y-auto">
      <h2 id="alert-modal-title" class="text-lg font-semibold text-gray-900 dark:text-white mb-5">
        Create Alert Rule
      </h2>

      <div class="space-y-4">
        <div>
          <label for="alert-name" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">
            Rule Name
          </label>
          <input
            id="alert-name"
            type="text"
            bind:value={newRule.name}
            class="w-full px-3 py-2 text-sm border border-gray-300 dark:border-gray-600
                   rounded-lg bg-white dark:bg-gray-700
                   text-gray-900 dark:text-white
                   placeholder-gray-400 dark:placeholder-gray-500
                   focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            placeholder="e.g., High Usage Alert"
          />
        </div>

        <div>
          <label for="alert-type" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">
            Alert Type
          </label>
          <select
            id="alert-type"
            bind:value={newRule.type}
            class="w-full px-3 py-2 text-sm border border-gray-300 dark:border-gray-600
                   rounded-lg bg-white dark:bg-gray-700
                   text-gray-900 dark:text-white
                   focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
          >
            <option value="usage">API Usage</option>
            <option value="balance">Account Balance</option>
            <option value="error_rate">Error Rate</option>
            <option value="latency">Response Latency</option>
          </select>
        </div>

        <div class="grid grid-cols-2 gap-3">
          <div>
            <label for="alert-operator" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">
              Operator
            </label>
            <select
              id="alert-operator"
              bind:value={newRule.operator}
              class="w-full px-3 py-2 text-sm border border-gray-300 dark:border-gray-600
                     rounded-lg bg-white dark:bg-gray-700
                     text-gray-900 dark:text-white
                     focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            >
              <option value="gt">Greater than (&gt;)</option>
              <option value="lt">Less than (&lt;)</option>
              <option value="eq">Equal to (=)</option>
              <option value="gte">Greater or equal (&gt;=)</option>
              <option value="lte">Less or equal (&lt;=)</option>
            </select>
          </div>

          <div>
            <label for="alert-threshold" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">
              Threshold
            </label>
            <input
              id="alert-threshold"
              type="number"
              bind:value={newRule.threshold}
              class="w-full px-3 py-2 text-sm border border-gray-300 dark:border-gray-600
                     rounded-lg bg-white dark:bg-gray-700
                     text-gray-900 dark:text-white
                     focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            />
          </div>
        </div>

        <div>
          <label for="alert-duration" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">
            Duration (minutes)
          </label>
          <input
            id="alert-duration"
            type="number"
            bind:value={newRule.duration_minutes}
            class="w-full px-3 py-2 text-sm border border-gray-300 dark:border-gray-600
                   rounded-lg bg-white dark:bg-gray-700
                   text-gray-900 dark:text-white
                   focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
          />
          <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
            Condition must persist for this duration before triggering
          </p>
        </div>

        <fieldset>
          <legend class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1.5">
            Notification Channels
          </legend>
          <div class="space-y-2">
            <label class="flex items-center gap-2.5 cursor-pointer">
              <input type="checkbox"
                     checked={newRule.channels.includes('email')}
                     onchange={() => toggleChannel('email')}
                     class="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500 dark:bg-gray-700" />
              <span class="text-sm text-gray-700 dark:text-gray-300">Email</span>
            </label>
            <label class="flex items-center gap-2.5 cursor-pointer">
              <input type="checkbox"
                     checked={newRule.channels.includes('webhook')}
                     onchange={() => toggleChannel('webhook')}
                     class="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500 dark:bg-gray-700" />
              <span class="text-sm text-gray-700 dark:text-gray-300">Webhook</span>
            </label>
            <label class="flex items-center gap-2.5 cursor-pointer">
              <input type="checkbox"
                     checked={newRule.channels.includes('slack')}
                     onchange={() => toggleChannel('slack')}
                     class="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500 dark:bg-gray-700" />
              <span class="text-sm text-gray-700 dark:text-gray-300">Slack</span>
            </label>
          </div>
        </fieldset>
      </div>

      <div class="flex justify-end gap-3 mt-6">
        <button
          onclick={() => { showCreateModal = false; resetForm(); }}
          class="px-4 py-2 text-sm font-medium border border-gray-300 dark:border-gray-600
                 rounded-lg text-gray-700 dark:text-gray-300
                 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors
                 focus:outline-none focus:ring-2 focus:ring-gray-400"
        >
          Cancel
        </button>
        <button
          onclick={createRule}
          disabled={!newRule.name || newRule.channels.length === 0}
          class="px-4 py-2 text-sm font-medium bg-blue-600 text-white rounded-lg
                 hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors
                 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 dark:focus:ring-offset-gray-800"
        >
          Create
        </button>
      </div>
    </div>
  </div>
{/if}
