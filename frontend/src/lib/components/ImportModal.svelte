<script lang="ts">
  let {
    importJson = $bindable(),
    submitting,
    preview,
    result,
    onSubmit,
    onClose,
    onJsonInput,
  }: {
    importJson: string;
    submitting: boolean;
    preview: {
      total: number; valid: number; invalid: number; duplicate: number;
      will_import: number; duration_ms: number;
      providers: Array<{ provider: string; total: number; valid: number; invalid: number; duplicate: number; will_import: number }>;
      errors: Array<{ index?: number; name?: string; error?: string }>;
    } | null;
    result: { succeeded: number; failed: number; skipped: number; errors: string[] } | null;
    onSubmit: () => void;
    onClose: () => void;
    onJsonInput: () => void;
  } = $props();
</script>

<div class="fixed inset-0 z-40 flex items-center justify-center p-4" role="dialog" aria-modal="true" aria-labelledby="import-modal-title">
  <button type="button" class="absolute inset-0 bg-black/50" aria-label="关闭批量导入弹窗" onclick={onClose}></button>
  <div class="relative w-full max-w-xl rounded-xl bg-white p-6 shadow-xl dark:bg-gray-800" role="document">
    <h2 id="import-modal-title" class="text-lg font-semibold text-gray-900 dark:text-white">批量导入</h2>
    <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">Paste a JSON array of accounts below.</p>
    <form onsubmit={(e) => { e.preventDefault(); onSubmit(); }} class="mt-4 space-y-4">
      <div>
        <label for="import-json" class="block text-sm font-medium text-gray-700 dark:text-gray-300">JSON Data</label>
        <textarea
          id="import-json"
          rows="10"
          required
          bind:value={importJson}
          oninput={onJsonInput}
          class="mt-1 w-full rounded-lg border border-gray-300 bg-white px-3 py-2 font-mono text-xs text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
          placeholder={`[\n  {\n    "name": "OpenAI Main",\n    "provider": "openai",\n    "credential_type": "api_key",\n    "credential": "sk-..."\n  }\n]`}
        ></textarea>
      </div>
      {#if preview}
        <div class="rounded-lg border border-blue-200 bg-blue-50 p-3 text-sm dark:border-blue-800 dark:bg-blue-900/20">
          <p class="font-medium text-blue-800 dark:text-blue-200">
            预检完成：预计导入 {preview.will_import} / {preview.total}，重复 {preview.duplicate}，校验失败 {preview.invalid}
          </p>
          <p class="mt-1 text-xs text-blue-700 dark:text-blue-300">
            耗时 {preview.duration_ms}ms。再次点击"导入"将按当前 JSON 真正执行。
          </p>
          {#if preview.providers.length > 0}
            <div class="mt-2 overflow-x-auto">
              <table class="min-w-full text-xs text-blue-900 dark:text-blue-100">
                <thead><tr class="text-left"><th class="pr-4">Provider</th><th class="pr-4">Total</th><th class="pr-4">Will Import</th><th class="pr-4">Duplicate</th><th class="pr-4">Invalid</th></tr></thead>
                <tbody>
                  {#each preview.providers as provider}
                    <tr><td class="pr-4 py-1">{provider.provider}</td><td class="pr-4 py-1">{provider.total}</td><td class="pr-4 py-1">{provider.will_import}</td><td class="pr-4 py-1">{provider.duplicate}</td><td class="pr-4 py-1">{provider.invalid}</td></tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/if}
          {#if preview.errors.length > 0}
            <ul class="mt-2 list-inside list-disc text-xs text-blue-700 dark:text-blue-300">
              {#each preview.errors as err}
                <li>{err.name || `#${err.index ?? '-'}`}: {err.error ?? 'Invalid item'}</li>
              {/each}
            </ul>
          {/if}
        </div>
      {/if}
      {#if result}
        <div class="rounded-lg border p-3 text-sm {result.failed > 0 ? 'border-yellow-200 bg-yellow-50 dark:border-yellow-800 dark:bg-yellow-900/20' : 'border-green-200 bg-green-50 dark:border-green-800 dark:bg-green-900/20'}">
          <p class="font-medium {result.failed > 0 ? 'text-yellow-800 dark:text-yellow-200' : 'text-green-800 dark:text-green-200'}">
            Import complete: {result.succeeded} succeeded, {result.skipped} skipped, {result.failed} failed
          </p>
          {#if result.errors.length > 0}
            <ul class="mt-2 list-inside list-disc text-xs text-yellow-700 dark:text-yellow-300">
              {#each result.errors as err}
                <li>{err}</li>
              {/each}
            </ul>
          {/if}
        </div>
      {/if}
      <div class="flex justify-end gap-3 pt-2">
        <button type="button" onclick={onClose} class="rounded-lg border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600">关闭</button>
        <button type="submit" disabled={submitting} class="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-700 disabled:opacity-50">{submitting ? '导入中...' : preview ? '确认导入' : '导入'}</button>
      </div>
    </form>
  </div>
</div>
