<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { api, type Model } from '$lib/api';

  let models: Model[] = [];
  let selectedModel = 'gpt-4';
  let messages: Array<{role: string, content: string}> = [];
  let input = '';
  let apiKey = '';
  let loading = false;
  let streaming = false;

  onMount(async () => {
    await loadModels();
    // Load API key from localStorage
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
    
    // 设置 token
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

<div class="p-6">
  <div class="max-w-4xl mx-auto">
    <div class="flex items-center justify-between mb-6">
      <h1 class="text-2xl font-bold">💬 Chat Playground</h1>
      <button
        on:click={clearChat}
        class="px-4 py-2 border border-gray-300 rounded hover:bg-gray-50"
      >
        Clear Chat
      </button>
    </div>

    <!-- API Key Input -->
    <div class="bg-white shadow rounded-lg p-4 mb-6">
      <div class="flex gap-4">
        <input
          type="password"
          bind:value={apiKey}
          placeholder="Enter your API key"
          class="flex-1 px-3 py-2 border border-gray-300 rounded focus:outline-none focus:ring-2 focus:ring-blue-500"
        />
        <button
          on:click={saveApiKey}
          class="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600"
        >
          Save Key
        </button>
      </div>
    </div>

    <!-- Model Selection -->
    <div class="bg-white shadow rounded-lg p-4 mb-6">
      <label class="block text-sm font-medium text-gray-700 mb-2">Model</label>
      <select
        bind:value={selectedModel}
        class="w-full px-3 py-2 border border-gray-300 rounded focus:outline-none focus:ring-2 focus:ring-blue-500"
      >
        {#each models as model}
          <option value={model.id}>{model.name} ({model.provider})</option>
        {/each}
      </select>
    </div>

    <!-- Chat Messages -->
    <div class="bg-white shadow rounded-lg p-4 mb-6 h-96 overflow-y-auto">
      {#if messages.length === 0}
        <div class="flex items-center justify-center h-full text-gray-400">
          Start a conversation
        </div>
      {:else}
        <div class="space-y-4">
          {#each messages as message}
            <div class="flex {message.role === 'user' ? 'justify-end' : 'justify-start'}">
              <div class="max-w-xs lg:max-w-md px-4 py-2 rounded-lg {message.role === 'user' ? 'bg-blue-500 text-white' : 'bg-gray-200 text-gray-900'}">
                <div class="whitespace-pre-wrap">{message.content}</div>
              </div>
            </div>
          {/each}
          {#if loading}
            <div class="flex justify-start">
              <div class="bg-gray-200 px-4 py-2 rounded-lg">
                <div class="flex items-center space-x-2">
                  <div class="animate-spin rounded-full h-4 w-4 border-b-2 border-gray-500"></div>
                  <span class="text-gray-500">Thinking...</span>
                </div>
              </div>
            </div>
          {/if}
        </div>
      {/if}
    </div>

    <!-- Input -->
    <div class="bg-white shadow rounded-lg p-4">
      <div class="flex gap-4">
        <input
          type="text"
          bind:value={input}
          on:keydown={(e) => e.key === 'Enter' && sendMessage()}
          placeholder="Type your message..."
          class="flex-1 px-3 py-2 border border-gray-300 rounded focus:outline-none focus:ring-2 focus:ring-blue-500"
          disabled={loading}
        />
        <button
          on:click={sendMessage}
          disabled={loading || !input.trim()}
          class="px-6 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          Send
        </button>
      </div>
    </div>
  </div>
</div>
