<script lang="ts">
  let {
    targetName,
    submitting,
    onConfirm,
    onCancel,
  }: {
    targetName: string;
    submitting: boolean;
    onConfirm: () => void;
    onCancel: () => void;
  } = $props();
</script>

<div class="fixed inset-0 z-40 flex items-center justify-center p-4" role="dialog" aria-modal="true" aria-labelledby="delete-modal-title">
  <button type="button" class="absolute inset-0 bg-black/50" aria-label="关闭删除渠道弹窗" onclick={onCancel}></button>
  <div class="relative w-full max-w-sm rounded-xl bg-white p-6 shadow-xl dark:bg-gray-800" role="document">
    <div class="flex items-start gap-3">
      <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-red-100 dark:bg-red-900/30">
        <svg class="h-5 w-5 text-red-600 dark:text-red-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
      </div>
      <div>
        <h2 id="delete-modal-title" class="text-base font-semibold text-gray-900 dark:text-white">确认删除</h2>
        <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">确定要删除渠道 <span class="font-medium text-gray-700 dark:text-gray-200">{targetName}</span> 吗？此操作不可撤销。</p>
      </div>
    </div>
    <div class="mt-5 flex justify-end gap-3">
      <button type="button" onclick={onCancel} class="rounded-lg border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600">取消</button>
      <button type="button" onclick={onConfirm} disabled={submitting} class="rounded-lg bg-red-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-red-700 disabled:opacity-50">{submitting ? '删除中...' : '删除'}</button>
    </div>
  </div>
</div>
