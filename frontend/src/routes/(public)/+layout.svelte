<script lang="ts">
  let { children } = $props();
</script>

<div class="min-h-screen bg-light-bg dark:bg-dark-bg">
  {@render children()}
</div>
