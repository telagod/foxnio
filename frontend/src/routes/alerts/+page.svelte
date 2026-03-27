<script lang="ts">
  import { onMount } from 'svelte';

  interface AlertRule {
    id: string;
    name: string;
    type: 'usage' | 'balance' | 'error_rate' | 'latency';
    threshold: number;
    operator: 'gt' | 'lt' | 'eq' | 'gte' | 'lte';
    duration_minutes: number;
    channels: ('email' | 'webhook' | 'slack')[];
    enabled: boolean;
    created_at: string;
    last_triggered_at: string | null;
  }

  let alertRules: AlertRule[] = [];
  let loading = true;
  let showCreateModal = false;
  let editingRule: AlertRule | null = null;

  let newRule = {
    name: '',
    type: 'usage' as const,
    threshold: 1000,
    operator: 'gt' as const,
    duration_minutes: 5,
    channels: [] as ('email' | 'webhook' | 'slack')[],
    enabled: true,
  };

  onMount(async () => {
    await loadAlertRules();
  });

  async function loadAlertRules() {
    try {
      const response = await fetch('/api/v1/alerts/rules');
      if (response.ok) {
        const data = await response.json();
        alertRules = data.rules || [];
      }
    } catch (e) {
      console.error('Failed to load alert rules:', e);
    } finally {
      loading = false;
    }
  }

  async function createRule() {
    try {
      const response = await fetch('/api/v1/alerts/rules', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(newRule)
      });

      if (response.ok) {
        await loadAlertRules();
        showCreateModal = false;
        resetForm();
      }
    } catch (e) {
      console.error('Failed to create alert rule:', e);
    }
  }

  async function updateRule(rule: AlertRule) {
    try {
      const response = await fetch(`/api/v1/alerts/rules/${rule.id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(rule)
      });

      if (response.ok) {
        await loadAlertRules();
        editingRule = null;
      }
    } catch (e) {
      console.error('Failed to update alert rule:', e);
    }
  }

  async function deleteRule(id: string) {
    if (!confirm('Are you sure you want to delete this alert rule?')) return;

    try {
      const response = await fetch(`/api/v1/alerts/rules/${id}`, {
        method: 'DELETE'
      });

      if (response.ok) {
        await loadAlertRules();
      }
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
</script>

<svelte:head>
  <title>Alert Rules - FoxNIO</title>
</svelte:head>

<div class="space-y-6">
  <!-- 页面标题 -->
  <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
    <div>
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Alert Rules</h1>
      <p class="text-gray-500 dark:text-gray-400 mt-1">Configure alerts for your API usage and account</p>
    </div>
    
    <button
      onclick={() => showCreateModal = true}
      class="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 
             transition-colors flex items-center gap-2 justify-center"
    >
      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"></path>
      </svg>
      Create Alert Rule
    </button>
  </div>

  <!-- 加载状态 -->
  {#if loading}
    <div class="flex items-center justify-center h-64">
      <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-500"></div>
    </div>
  {:else if alertRules.length === 0}
    <!-- 空状态 -->
    <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-12 text-center">
      <div class="text-6xl mb-4">🔔</div>
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">No Alert Rules Yet</h3>
      <p class="text-gray-500 dark:text-gray-400 mb-6">Create your first alert rule to get notified about important events</p>
      <button
        onclick={() => showCreateModal = true}
        class="px-6 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-colors"
      >
        Create Alert Rule
      </button>
    </div>
  {:else}
    <!-- 告警规则列表 -->
    <div class="space-y-4">
      {#each alertRules as rule}
        <div class="bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 p-6">
          <div class="flex items-start justify-between">
            <div class="flex-1">
              <div class="flex items-center gap-3 mb-2">
                <h3 class="text-lg font-semibold text-gray-900 dark:text-white">{rule.name}</h3>
                <span class="px-2 py-1 text-xs font-medium rounded-full 
                            {rule.enabled 
                              ? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400' 
                              : 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-400'}">
                  {rule.enabled ? 'Active' : 'Disabled'}
                </span>
              </div>
              
              <div class="flex flex-wrap items-center gap-4 text-sm text-gray-600 dark:text-gray-400">
                <span class="flex items-center gap-1">
                  <span class="font-medium">Type:</span>
                  {getAlertTypeLabel(rule.type)}
                </span>
                <span class="flex items-center gap-1">
                  <span class="font-medium">Condition:</span>
                  {getOperatorLabel(rule.operator)} {rule.threshold}
                </span>
                <span class="flex items-center gap-1">
                  <span class="font-medium">Duration:</span>
                  {rule.duration_minutes} min
                </span>
                <span class="flex items-center gap-1">
                  <span class="font-medium">Channels:</span>
                  {rule.channels.join(', ')}
                </span>
              </div>
              
              {#if rule.last_triggered_at}
                <p class="text-xs text-gray-500 dark:text-gray-500 mt-2">
                  Last triggered: {new Date(rule.last_triggered_at).toLocaleString()}
                </p>
              {/if}
            </div>
            
            <div class="flex items-center gap-2">
              <!-- 启用/禁用开关 -->
              <button
                onclick={() => toggleRule(rule)}
                class="relative inline-flex h-6 w-11 items-center rounded-full transition-colors
                       {rule.enabled ? 'bg-blue-500' : 'bg-gray-300 dark:bg-gray-600'}"
              >
                <span class="inline-block h-4 w-4 transform rounded-full bg-white transition-transform
                            {rule.enabled ? 'translate-x-6' : 'translate-x-1'}"></span>
              </button>
              
              <button
                onclick={() => editingRule = rule}
                class="p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
              >
                <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                        d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"></path>
                </svg>
              </button>
              
              <button
                onclick={() => deleteRule(rule.id)}
                class="p-2 text-red-400 hover:text-red-600 dark:hover:text-red-300 transition-colors"
              >
                <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                        d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"></path>
                </svg>
              </button>
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}

  <!-- 使用说明 -->
  <div class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4">
    <h3 class="text-sm font-semibold text-blue-900 dark:text-blue-200 mb-2">💡 Alert Types</h3>
    <ul class="text-sm text-blue-800 dark:text-blue-300 space-y-1">
      <li><strong>API Usage:</strong> Alert when API request count exceeds threshold</li>
      <li><strong>Account Balance:</strong> Alert when account balance falls below threshold</li>
      <li><strong>Error Rate:</strong> Alert when error rate exceeds threshold percentage</li>
      <li><strong>Response Latency:</strong> Alert when average response time exceeds threshold (ms)</li>
    </ul>
  </div>
</div>

<!-- 创建模态框 -->
{#if showCreateModal}
  <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4" 
       onclick={(e) => e.target === e.currentTarget && (showCreateModal = false)}
       role="dialog"
       aria-modal="true"
       aria-labelledby="modal-title">
    <div class="bg-white dark:bg-gray-800 rounded-xl max-w-md w-full p-6 shadow-xl max-h-[90vh] overflow-y-auto">
      <h2 id="modal-title" class="text-xl font-bold text-gray-900 dark:text-white mb-4">
        Create Alert Rule
      </h2>
      
      <div class="space-y-4">
        <div>
          <label for="alert-name" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            Rule Name
          </label>
          <input
            id="alert-name"
            type="text"
            bind:value={newRule.name}
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 
                   rounded-lg bg-white dark:bg-gray-700
                   text-gray-900 dark:text-white
                   focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            placeholder="e.g., High Usage Alert"
          />
        </div>

        <div>
          <label for="alert-type" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            Alert Type
          </label>
          <select
            id="alert-type"
            bind:value={newRule.type}
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 
                   rounded-lg bg-white dark:bg-gray-700
                   text-gray-900 dark:text-white
                   focus:ring-2 focus:ring-blue-500 focus:border-transparent"
          >
            <option value="usage">API Usage</option>
            <option value="balance">Account Balance</option>
            <option value="error_rate">Error Rate</option>
            <option value="latency">Response Latency</option>
          </select>
        </div>

        <div class="grid grid-cols-2 gap-4">
          <div>
            <label for="alert-operator" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              Operator
            </label>
            <select
              id="alert-operator"
              bind:value={newRule.operator}
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 
                     rounded-lg bg-white dark:bg-gray-700
                     text-gray-900 dark:text-white
                     focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            >
              <option value="gt">Greater than (&gt;)</option>
              <option value="lt">Less than (&lt;)</option>
              <option value="eq">Equal to (=)</option>
              <option value="gte">Greater or equal (&gt;=)</option>
              <option value="lte">Less or equal (&lt;=)</option>
            </select>
          </div>

          <div>
            <label for="alert-threshold" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              Threshold
            </label>
            <input
              id="alert-threshold"
              type="number"
              bind:value={newRule.threshold}
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 
                     rounded-lg bg-white dark:bg-gray-700
                     text-gray-900 dark:text-white
                     focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            />
          </div>
        </div>

        <div>
          <label for="alert-duration" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            Duration (minutes)
          </label>
          <input
            id="alert-duration"
            type="number"
            bind:value={newRule.duration_minutes}
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 
                   rounded-lg bg-white dark:bg-gray-700
                   text-gray-900 dark:text-white
                   focus:ring-2 focus:ring-blue-500 focus:border-transparent"
          />
          <p class="text-xs text-gray-500 dark:text-gray-400 mt-1">
            Condition must persist for this duration before triggering
          </p>
        </div>

        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            Notification Channels
          </label>
          <div class="space-y-2">
            <label class="flex items-center gap-2">
              <input type="checkbox" 
                     checked={newRule.channels.includes('email')}
                     onchange={() => {
                       if (newRule.channels.includes('email')) {
                         newRule.channels = newRule.channels.filter(c => c !== 'email');
                       } else {
                         newRule.channels = [...newRule.channels, 'email'];
                       }
                     }}
                     class="rounded text-blue-500" />
              <span class="text-sm text-gray-700 dark:text-gray-300">Email</span>
            </label>
            <label class="flex items-center gap-2">
              <input type="checkbox" 
                     checked={newRule.channels.includes('webhook')}
                     onchange={() => {
                       if (newRule.channels.includes('webhook')) {
                         newRule.channels = newRule.channels.filter(c => c !== 'webhook');
                       } else {
                         newRule.channels = [...newRule.channels, 'webhook'];
                       }
                     }}
                     class="rounded text-blue-500" />
              <span class="text-sm text-gray-700 dark:text-gray-300">Webhook</span>
            </label>
            <label class="flex items-center gap-2">
              <input type="checkbox" 
                     checked={newRule.channels.includes('slack')}
                     onchange={() => {
                       if (newRule.channels.includes('slack')) {
                         newRule.channels = newRule.channels.filter(c => c !== 'slack');
                       } else {
                         newRule.channels = [...newRule.channels, 'slack'];
                       }
                     }}
                     class="rounded text-blue-500" />
              <span class="text-sm text-gray-700 dark:text-gray-300">Slack</span>
            </label>
          </div>
        </div>
      </div>

      <div class="flex justify-end gap-3 mt-6">
        <button
          onclick={() => { showCreateModal = false; resetForm(); }}
          class="px-4 py-2 border border-gray-300 dark:border-gray-600 
                 rounded-lg text-gray-700 dark:text-gray-300
                 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
        >
          Cancel
        </button>
        <button
          onclick={createRule}
          disabled={!newRule.name || newRule.channels.length === 0}
          class="px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 
                 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          Create
        </button>
      </div>
    </div>
  </div>
{/if}
