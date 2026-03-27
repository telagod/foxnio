<script>
  import Sidebar from '$lib/components/Sidebar.svelte';
</script>

<div class="flex h-screen bg-gray-100">
  <Sidebar />
  <main class="flex-1 overflow-auto">
    <slot />
  </main>
</div>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, 'Open Sans', 'Helvetica Neue', sans-serif;
  }
</style>
