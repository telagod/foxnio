<script lang="ts">
  import type { Account } from '$lib/api';

  let {
    accounts,
    selectedAccountIds = $bindable(),
    currentPageAllSelected,
    groups,
    onSelectAll,
    onToggleSelect,
    onEdit,
    onDelete,
    platformColor,
    statusColor,
    statusDot,
    renderGroupName,
    formatDate,
  }: {
    accounts: Account[];
    selectedAccountIds: string[];
    currentPageAllSelected: boolean;
    groups: Array<{ id: number; name: string }>;
    onSelectAll: (checked: boolean) => void;
    onToggleSelect: (id: string, checked: boolean) => void;
    onEdit: (account: Account) => void;
    onDelete: (account: Account) => void;
    platformColor: (provider: string) => string;
    statusColor: (status: string) => string;
    statusDot: (status: string) => string;
    renderGroupName: (groupId: number | null | undefined) => string;
    formatDate: (value: string) => string;
  } = $props();
</script>

<!-- Desktop table -->
<div class="hidden overflow-hidden rounded-xl border border-gray-200 bg-white shadow-sm dark:border-gray-700 dark:bg-gray-800 md:block">
  <table class="min-w-full divide-y divide-gray-200 text-sm dark:divide-gray-700">
    <thead>
      <tr class="bg-gray-50 text-left text-xs font-medium uppercase tracking-wider text-gray-500 dark:bg-gray-800/60 dark:text-gray-400">
        <th scope="col" class="px-5 py-3">
          <input type="checkbox" checked={currentPageAllSelected} onchange={(e) => onSelectAll((e.currentTarget as HTMLInputElement).checked)} class="rounded border-gray-300 text-blue-600 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:focus:ring-blue-400" aria-label="Select all accounts on current page" />
        </th>
        <th scope="col" class="px-5 py-3">Name</th>
        <th scope="col" class="px-5 py-3">Platform</th>
        <th scope="col" class="px-5 py-3">Credential</th>
        <th scope="col" class="px-5 py-3">Group</th>
        <th scope="col" class="px-5 py-3">Status</th>
        <th scope="col" class="px-5 py-3">Priority</th>
        <th scope="col" class="px-5 py-3">Last Error</th>
        <th scope="col" class="px-5 py-3">Created</th>
        <th scope="col" class="px-5 py-3 text-right">Actions</th>
      </tr>
    </thead>
    <tbody class="divide-y divide-gray-100 dark:divide-gray-700/50">
      {#each accounts as account (account.id)}
        <tr class="transition-colors hover:bg-gray-50 dark:hover:bg-gray-700/20">
          <td class="whitespace-nowrap px-5 py-3.5">
            <input type="checkbox" checked={selectedAccountIds.includes(account.id)} onchange={(e) => onToggleSelect(account.id, (e.currentTarget as HTMLInputElement).checked)} class="rounded border-gray-300 text-blue-600 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:focus:ring-blue-400" aria-label={`Select account ${account.name}`} />
          </td>
          <td class="whitespace-nowrap px-5 py-3.5">
            <span class="font-medium text-gray-900 dark:text-white">{account.name}</span>
          </td>
          <td class="whitespace-nowrap px-5 py-3.5">
            <span class="inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium {platformColor(account.provider)}">{account.provider}</span>
          </td>
          <td class="whitespace-nowrap px-5 py-3.5 text-gray-500 dark:text-gray-400">{account.credential_type}</td>
          <td class="whitespace-nowrap px-5 py-3.5 text-gray-600 dark:text-gray-300">{renderGroupName(account.group_id)}</td>
          <td class="whitespace-nowrap px-5 py-3.5">
            <span class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium {statusColor(account.status)}">
              <span class="h-1.5 w-1.5 rounded-full {statusDot(account.status)}"></span>
              {account.status}
            </span>
          </td>
          <td class="whitespace-nowrap px-5 py-3.5 text-gray-600 dark:text-gray-300">{account.priority ?? 0}</td>
          <td class="max-w-[200px] truncate px-5 py-3.5 text-xs text-gray-500 dark:text-gray-400" title={account.last_error ?? ''}>{account.last_error ?? '-'}</td>
          <td class="whitespace-nowrap px-5 py-3.5 text-gray-500 dark:text-gray-400">{formatDate(account.created_at)}</td>
          <td class="whitespace-nowrap px-5 py-3.5 text-right">
            <div class="flex items-center justify-end gap-1">
              <button onclick={() => onEdit(account)} class="rounded-md p-1.5 text-gray-400 transition-colors hover:bg-gray-100 hover:text-blue-600 dark:hover:bg-gray-700 dark:hover:text-blue-400" aria-label="Edit {account.name}">
                <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
              </button>
              <button onclick={() => onDelete(account)} class="rounded-md p-1.5 text-gray-400 transition-colors hover:bg-red-50 hover:text-red-600 dark:hover:bg-red-900/20 dark:hover:text-red-400" aria-label="Delete {account.name}">
                <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/><line x1="10" y1="11" x2="10" y2="17"/><line x1="14" y1="11" x2="14" y2="17"/></svg>
              </button>
            </div>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<!-- Mobile cards -->
<div class="space-y-3 md:hidden">
  {#each accounts as account (account.id)}
    <div class="rounded-xl border border-gray-200 bg-white p-4 shadow-sm dark:border-gray-700 dark:bg-gray-800">
      <label class="mb-2 flex items-center gap-2 text-sm text-gray-700 dark:text-gray-200">
        <input type="checkbox" checked={selectedAccountIds.includes(account.id)} onchange={(e) => onToggleSelect(account.id, (e.currentTarget as HTMLInputElement).checked)} class="rounded border-gray-300 text-blue-600 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700" />
        <span>选择</span>
      </label>
      <div class="flex items-start justify-between gap-2">
        <div class="min-w-0 flex-1">
          <div class="truncate font-medium text-gray-900 dark:text-white">{account.name}</div>
          <div class="mt-1 text-xs text-gray-500 dark:text-gray-400">{account.credential_type}</div>
        </div>
        <span class="shrink-0 inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium {platformColor(account.provider)}">{account.provider}</span>
      </div>
      <div class="mt-3 flex items-center justify-between">
        <span class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium {statusColor(account.status)}">
          <span class="h-1.5 w-1.5 rounded-full {statusDot(account.status)}"></span>
          {account.status}
        </span>
        <span class="text-xs text-gray-500 dark:text-gray-400">Priority: {account.priority ?? 0}</span>
      </div>
      <div class="mt-1 text-xs text-gray-500 dark:text-gray-400">Group: {renderGroupName(account.group_id)}</div>
      {#if account.last_error}
        <p class="mt-2 truncate text-xs text-red-500 dark:text-red-400" title={account.last_error}>{account.last_error}</p>
      {/if}
      <div class="mt-3 flex items-center justify-between border-t border-gray-100 pt-3 dark:border-gray-700">
        <span class="text-xs text-gray-400 dark:text-gray-500">{formatDate(account.created_at)}</span>
        <div class="flex gap-1">
          <button onclick={() => onEdit(account)} class="rounded-md p-1.5 text-gray-400 transition-colors hover:bg-gray-100 hover:text-blue-600 dark:hover:bg-gray-700 dark:hover:text-blue-400" aria-label="Edit {account.name}">
            <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
          </button>
          <button onclick={() => onDelete(account)} class="rounded-md p-1.5 text-gray-400 transition-colors hover:bg-red-50 hover:text-red-600 dark:hover:bg-red-900/20 dark:hover:text-red-400" aria-label="Delete {account.name}">
            <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/><line x1="10" y1="11" x2="10" y2="17"/><line x1="14" y1="11" x2="14" y2="17"/></svg>
          </button>
        </div>
      </div>
    </div>
  {/each}
</div>
