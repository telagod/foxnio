<svelte:head>
  <title>充值 - FoxNIO</title>
</svelte:head>

<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';

  const PRESET_AMOUNTS = [1000, 5000, 10000, 50000]; // 分

  let balance = $state(0);
  let providers = $state<string[]>([]);
  let selectedAmount = $state(1000);
  let customAmount = $state('');
  let selectedProvider = $state('');
  let loading = $state(true);
  let submitting = $state(false);
  let error = $state<string | null>(null);

  const PROVIDER_LABELS: Record<string, string> = {
    stripe: 'Stripe (国际卡)',
    alipay: '支付宝',
    wxpay: '微信支付',
    easypay: 'EasyPay',
  };

  onMount(async () => {
    const token = localStorage.getItem('token');
    if (token) api.setToken(token);
    try {
      const [user, config] = await Promise.all([
        api.getMe(),
        api.getPaymentConfig(),
      ]);
      balance = user.balance;
      providers = config.providers;
      if (providers.length > 0) selectedProvider = providers[0];
    } catch (e) {
      error = e instanceof Error ? e.message : '加载失败';
    } finally {
      loading = false;
    }
  });

  function getAmount(): number {
    if (customAmount) return Math.round(parseFloat(customAmount) * 100);
    return selectedAmount;
  }

  async function handleSubmit() {
    const amount = getAmount();
    if (amount <= 0) { error = '请输入有效金额'; return; }
    if (!selectedProvider) { error = '请选择支付方式'; return; }

    submitting = true;
    error = null;
    try {
      const order = await api.createPaymentOrder({
        amount_cents: amount,
        provider: selectedProvider,
        return_url: window.location.href,
      });

      if (order.payment_url) {
        window.location.href = order.payment_url;
      } else if (order.client_secret) {
        // Stripe: 需要前端 Stripe.js 处理
        error = 'Stripe 支付需要配置前端 Stripe Elements';
      } else {
        error = '支付创建成功，请等待跳转';
      }
    } catch (e) {
      error = e instanceof Error ? e.message : '创建订单失败';
    } finally {
      submitting = false;
    }
  }
</script>

<div class="mx-auto max-w-lg space-y-6">
  <div>
    <h1 class="text-2xl font-bold text-gray-900 dark:text-white">充值</h1>
    <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">当前余额: ¥{(balance / 100).toFixed(2)}</p>
  </div>

  {#if loading}
    <div class="flex justify-center py-12">
      <div class="h-8 w-8 animate-spin rounded-full border-4 border-blue-500 border-t-transparent"></div>
    </div>
  {:else if providers.length === 0}
    <div class="rounded-xl border border-yellow-200 bg-yellow-50 p-6 text-center dark:border-yellow-800 dark:bg-yellow-900/20">
      <p class="text-yellow-800 dark:text-yellow-200">暂无可用支付方式，请联系管理员配置。</p>
    </div>
  {:else}
    <!-- 金额选择 -->
    <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
      <h2 class="mb-4 text-sm font-medium text-gray-700 dark:text-gray-300">选择金额</h2>
      <div class="grid grid-cols-2 gap-3">
        {#each PRESET_AMOUNTS as amount}
          <button
            onclick={() => { selectedAmount = amount; customAmount = ''; }}
            class="rounded-lg border-2 px-4 py-3 text-center text-sm font-medium transition-colors {selectedAmount === amount && !customAmount ? 'border-blue-500 bg-blue-50 text-blue-700 dark:border-blue-400 dark:bg-blue-900/30 dark:text-blue-300' : 'border-gray-200 text-gray-700 hover:border-gray-300 dark:border-gray-600 dark:text-gray-300 dark:hover:border-gray-500'}"
          >
            ¥{(amount / 100).toFixed(0)}
          </button>
        {/each}
      </div>
      <div class="mt-4">
        <label class="mb-1 block text-sm text-gray-600 dark:text-gray-400" for="custom-amount">自定义金额 (元)</label>
        <input
          id="custom-amount"
          type="number"
          min="1"
          step="0.01"
          placeholder="输入金额"
          bind:value={customAmount}
          class="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
        />
      </div>
    </div>

    <!-- 支付方式 -->
    <div class="rounded-xl border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
      <h2 class="mb-4 text-sm font-medium text-gray-700 dark:text-gray-300">支付方式</h2>
      <div class="space-y-2">
        {#each providers as provider}
          <label class="flex cursor-pointer items-center gap-3 rounded-lg border-2 px-4 py-3 transition-colors {selectedProvider === provider ? 'border-blue-500 bg-blue-50 dark:border-blue-400 dark:bg-blue-900/30' : 'border-gray-200 hover:border-gray-300 dark:border-gray-600 dark:hover:border-gray-500'}">
            <input type="radio" name="provider" value={provider} bind:group={selectedProvider} class="text-blue-600" />
            <span class="text-sm font-medium text-gray-700 dark:text-gray-300">{PROVIDER_LABELS[provider] ?? provider}</span>
          </label>
        {/each}
      </div>
    </div>

    {#if error}
      <div class="rounded-lg border border-red-200 bg-red-50 p-3 text-sm text-red-700 dark:border-red-800 dark:bg-red-900/20 dark:text-red-300">{error}</div>
    {/if}

    <!-- 提交 -->
    <button
      onclick={handleSubmit}
      disabled={submitting}
      class="w-full rounded-xl bg-blue-600 px-6 py-3 text-sm font-medium text-white transition-colors hover:bg-blue-700 disabled:opacity-50"
    >
      {submitting ? '处理中...' : `支付 ¥${(getAmount() / 100).toFixed(2)}`}
    </button>
  {/if}
</div>
