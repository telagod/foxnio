<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type Model } from '$lib/api';

  let models: Model[] = $state([]);
  let selectedModel = $state('gpt-4');
  let messages: Array<{role: string, content: string}> = $state([]);
  let input = $state('');
  let apiKey = $state('');
  let loading = $state(false);

  onMount(async () => {
    await loadModels();
    const savedKey = localStorage.getItem('api_key');
    if (savedKey) apiKey = savedKey;
  });

  async function loadModels() {
    try {
      const data = await api.getModels();
      models = (data.data || []).map(m => ({
        id: m.id,
        name: m.id,
        provider: m.owned_by || m.provider || 'unknown'
      }));
    } catch (e) {
      console.error('Failed to load models:', e);
    }
  }

  async function sendMessage() {
    if (!input.trim() || !apiKey) return;
    const userMessage = { role: 'user', content: input };
    messages = [...messages, userMessage];
    input = '';
    loading = true;
    api.setToken(apiKey);
    try {
      const data = await api.chatCompletions({
        model: selectedModel,
        messages: messages,
        stream: false
      });
      const assistantMessage = {
        role: 'assistant',
        content: data.choices[0].message.content
      };
      messages = [...messages, assistantMessage];
    } catch (e) {
      console.error('Failed to send message:', e);
      alert('Failed to send message. Please check your API key.');
    } finally {
      loading = false;
    }
  }

  function saveApiKey() {
    localStorage.setItem('api_key', apiKey);
    alert('API key saved!');
  }

  function clearChat() {
    messages = [];
  }
</script>

<svelte:head>
  <title>Chat Playground - FoxNIO</title>
</svelte:head>

<div class="p-6 max-w-4xl mx-auto space-y-6">
  <!-- Header -->
  <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
    <div class="flex items-center gap-3">
      <div class="p-2 bg-violet-100 dark:bg-violet-900/30 rounded-lg">
        <svg class="w-6 h-6 text-violet-600 dark:text-violet-400" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M20.25 8.511c.884.284 1.5 1.128 1.5 2.097v4.286c0 1.136-.847 2.1-1.98 2.193-.34.027-.68.052-1.02.072v3.091l-3-3c-1.354 0-2.694-.055-4.02-.163a2.115 2.115 0 0 1-.825-.242m9.345-8.334a2.126 2.126 0 0 0-.476-.095 48.64 48.64 0 0 0-8.048 0c-1.131.094-1.976 1.057-1.976 2.192v4.286c0 .837.46 1.58 1.155 1.951m9.345-8.334V6.637c0-1.621-1.152-3.026-2.76-3.235A48.455 48.455 0 0 0 11.25 3c-2.115 0-4.198.137-6.24.402-1.608.209-2.76 1.614-2.76 3.235v6.226c0 1.621 1.152 3.026 2.76 3.235.577.075 1.157.14 1.74.194V21l4.155-4.155" />
        </svg>
      </div>
      <div>
        <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Chat Playground</h1>
        <p class="text-sm text-gray-500 dark:text-gray-400 mt-0.5">Test models interactively</p>
      </div>
    </div>

    <button
      onclick={clearChat}
      class="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium
             bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600
             text-gray-700 dark:text-gray-300 rounded-lg
             hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors
             focus:outline-none focus:ring-2 focus:ring-violet-500 focus:ring-offset-2 dark:focus:ring-offset-gray-900"
      aria-label="Clear chat history"
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
        <path stroke-linecap="round" stroke-linejoin="round" d="M14.74 9l-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 0 1-2.244 2.077H8.084a2.25 2.25 0 0 1-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 0 0-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 0 1 3.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 0 0-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 0 0-7.5 0" />
      </svg>
      Clear Chat
    </button>
  </div>

  <!-- API Key -->
  <div class="bg-white dark:bg-gray-800 shadow-sm border border-gray-200 dark:border-gray-700 rounded-xl p-4">
    <label for="playground-api-key" class="block text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-2">API Key</label>
    <div class="flex gap-3">
      <input
        id="playground-api-key"
        type="password"
        bind:value={apiKey}
        placeholder="Enter your API key"
        class="flex-1 px-3 py-2 text-sm border border-gray-300 dark:border-gray-600 rounded-lg
               bg-white dark:bg-gray-700 text-gray-900 dark:text-white
               placeholder-gray-400 dark:placeholder-gray-500
               focus:outline-none focus:ring-2 focus:ring-violet-500 focus:border-transparent"
      />
      <button
        onclick={saveApiKey}
        class="px-4 py-2 text-sm font-medium bg-violet-600 text-white rounded-lg
               hover:bg-violet-700 transition-colors
               focus:outline-none focus:ring-2 focus:ring-violet-500 focus:ring-offset-2 dark:focus:ring-offset-gray-800"
      >
        Save Key
      </button>
    </div>
  </div>

  <!-- Model Selection -->
  <div class="bg-white dark:bg-gray-800 shadow-sm border border-gray-200 dark:border-gray-700 rounded-xl p-4">
    <label for="playground-model-select" class="block text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-2">Model</label>
    <select
      id="playground-model-select"
      bind:value={selectedModel}
      class="w-full px-3 py-2 text-sm border border-gray-300 dark:border-gray-600 rounded-lg
             bg-white dark:bg-gray-700 text-gray-900 dark:text-white
             focus:outline-none focus:ring-2 focus:ring-violet-500 focus:border-transparent"
    >
      {#each models as model}
        <option value={model.id}>{model.name} ({model.provider})</option>
      {/each}
    </select>
  </div>

  <!-- Chat Messages -->
  <div class="bg-white dark:bg-gray-800 shadow-sm border border-gray-200 dark:border-gray-700 rounded-xl h-96 overflow-y-auto">
    {#if messages.length === 0}
      <div class="flex flex-col items-center justify-center h-full text-gray-400 dark:text-gray-500 gap-3">
        <svg class="w-10 h-10" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M8.625 12a.375.375 0 1 1-.75 0 .375.375 0 0 1 .75 0zm0 0H8.25m4.125 0a.375.375 0 1 1-.75 0 .375.375 0 0 1 .75 0zm0 0H12m4.125 0a.375.375 0 1 1-.75 0 .375.375 0 0 1 .75 0zm0 0h-.375M21 12c0 4.556-4.03 8.25-9 8.25a9.764 9.764 0 0 1-2.555-.337A5.972 5.972 0 0 1 5.41 20.97a5.969 5.969 0 0 1-.474-.065 4.48 4.48 0 0 0 .978-2.025c.09-.457-.133-.901-.467-1.226C3.93 16.178 3 14.189 3 12c0-4.556 4.03-8.25 9-8.25s9 3.694 9 8.25z" />
        </svg>
        <span class="text-sm">Start a conversation</span>
      </div>
    {:else}
      <div class="p-4 space-y-4">
        {#each messages as message}
          <div class="flex {message.role === 'user' ? 'justify-end' : 'justify-start'}">
            <div class="max-w-[75%] px-4 py-2.5 rounded-2xl text-sm leading-relaxed
                        {message.role === 'user'
                          ? 'bg-violet-600 text-white rounded-br-md'
                          : 'bg-gray-100 dark:bg-gray-700 text-gray-900 dark:text-gray-100 rounded-bl-md'}">
              <div class="whitespace-pre-wrap">{message.content}</div>
            </div>
          </div>
        {/each}
        {#if loading}
          <div class="flex justify-start">
            <div class="bg-gray-100 dark:bg-gray-700 px-4 py-2.5 rounded-2xl rounded-bl-md">
              <div class="flex items-center gap-2">
                <div class="animate-spin rounded-full h-3.5 w-3.5 border-2 border-gray-300 dark:border-gray-500 border-t-gray-600 dark:border-t-gray-300"></div>
                <span class="text-sm text-gray-500 dark:text-gray-400">Thinking...</span>
              </div>
            </div>
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <!-- Input -->
  <div class="bg-white dark:bg-gray-800 shadow-sm border border-gray-200 dark:border-gray-700 rounded-xl p-4">
    <div class="flex gap-3">
      <label for="playground-message-input" class="sr-only">Message</label>
      <input
        id="playground-message-input"
        type="text"
        bind:value={input}
        onkeydown={(e) => e.key === 'Enter' && sendMessage()}
        placeholder="Type your message..."
        class="flex-1 px-3 py-2 text-sm border border-gray-300 dark:border-gray-600 rounded-lg
               bg-white dark:bg-gray-700 text-gray-900 dark:text-white
               placeholder-gray-400 dark:placeholder-gray-500
               focus:outline-none focus:ring-2 focus:ring-violet-500 focus:border-transparent
               disabled:opacity-50 disabled:cursor-not-allowed"
        disabled={loading}
      />
      <button
        onclick={sendMessage}
        disabled={loading || !input.trim()}
        class="inline-flex items-center gap-2 px-5 py-2 text-sm font-medium
               bg-violet-600 text-white rounded-lg
               hover:bg-violet-700 transition-colors
               disabled:opacity-50 disabled:cursor-not-allowed
               focus:outline-none focus:ring-2 focus:ring-violet-500 focus:ring-offset-2 dark:focus:ring-offset-gray-800"
        aria-label="Send message"
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M6 12 3.269 3.125A59.769 59.769 0 0 1 21.485 12 59.768 59.768 0 0 1 3.27 20.875L5.999 12zm0 0h7.5" />
        </svg>
        Send
      </button>
    </div>
  </div>
</div>
