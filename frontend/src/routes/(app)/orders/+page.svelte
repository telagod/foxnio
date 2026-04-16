<svelte:head>
  <title>订单记录 - FoxNIO</title>
</svelte:head>

<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';

  let orders = $state<Array<{
    id: string; order_no: string; provider: string;
    amount_cents: number; currency: string; status: string;
    created_at: string; paid_at?: string;
  }>>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  const STATUS_MAP: Record<string, { label: string; color: string }> = {
    pending: { label: '待支付', color: 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-300' },
    paid: { label: '已支付', color: 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-300' },
    completed: { label: '已完成', color: 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-300' },
    expired: { label: '已过期', color: 'bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300' },
    cancelled: { label: '已取消', color: 'bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300' },
    failed: { label: '失败', color: 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-300' },
  };

  const PROVIDER_MAP: Record<string, string> = {
    stripe: 'Stripe', alipay: '支付宝', wxpay: '微信支付', easypay: 'EasyPay',
  };

  onMount(async () => {
    const token = localStorage.getItem('token');
    if (token) api.setToken(token);
    try {
      const res = await api.listPaymentOrders();
      orders = res.data;
    } catch (e) {
      error = e instanceof Error ? e.message : '加载失败';
    } finally {
      loading = false;
    }
  });

  function formatDate(v: string) { return v ? new Date(v).toLocaleString('zh-CN') : '-'; }
</script>

<div class="space-y-6">
  <div>
    <h1 class="text-2xl font-bold text-gray-900 dark:text-white">订单记录</h1>
    <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">查看充值订单历史</p>
  </div>

  {#if loading}
    <div class="flex justify-center py-12">
      <div class="h-8 w-8 animate-spin rounded-full border-4 border-blue-500 border-t-transparent"></div>
    </div>
  {:else if error}
    <div class="rounded-lg border border-red-200 bg-red-50 p-4 text-sm text-red-700 dark:border-red-800 dark:bg-red-900/20 dark:text-red-300">{error}</div>
  {:else if orders.length === 0}
    <div class="rounded-xl border border-gray-200 bg-white p-8 text-center dark:border-gray-700 dark:bg-gray-800">
      <p class="text-gray-500 dark:text-gray-400">暂无订单记录</p>
      <a href="/recharge" class="mt-3 inline-block text-sm text-blue-600 hover:underline dark:text-blue-400">去充值</a>
    </div>
  {:else}
    <div class="overflow-hidden rounded-xl border border-gray-200 bg-white shadow-sm dark:border-gray-700 dark:bg-gray-800">
      <table class="min-w-full divide-y divide-gray-200 text-sm dark:divide-gray-700">
        <thead>
          <tr class="bg-gray-50 text-left text-xs font-medium uppercase tracking-wider text-gray-500 dark:bg-gray-800/60 dark:text-gray-400">
            <th class="px-5 py-3">订单号</th>
            <th class="px-5 py-3">金额</th>
            <th class="px-5 py-3">支付方式</th>
            <th class="px-5 py-3">状态</th>
            <th class="px-5 py-3">创建时间</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-gray-100 dark:divide-gray-700/50">
          {#each orders as order (order.id)}
            <tr class="transition-colors hover:bg-gray-50 dark:hover:bg-gray-700/20">
              <td class="whitespace-nowrap px-5 py-3.5 font-mono text-xs text-gray-600 dark:text-gray-300">{order.order_no}</td>
              <td class="whitespace-nowrap px-5 py-3.5 font-medium text-gray-900 dark:text-white">¥{(order.amount_cents / 100).toFixed(2)}</td>
              <td class="whitespace-nowrap px-5 py-3.5 text-gray-600 dark:text-gray-300">{PROVIDER_MAP[order.provider] ?? order.provider}</td>
              <td class="whitespace-nowrap px-5 py-3.5">
                <span class="inline-flex rounded-full px-2.5 py-0.5 text-xs font-medium {STATUS_MAP[order.status]?.color ?? 'bg-gray-100 text-gray-700'}">
                  {STATUS_MAP[order.status]?.label ?? order.status}
                </span>
              </td>
              <td class="whitespace-nowrap px-5 py-3.5 text-gray-500 dark:text-gray-400">{formatDate(order.created_at)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>
