<script lang="ts">
  import { goto } from '$app/navigation';
  import { session } from '$lib/stores/session';
  import { onMount } from 'svelte';

  onMount(() => {
    goto($session ? '/app' : '/login');
  });
</script>

<div class="flex min-h-screen items-center justify-center">
  <div class="text-sm text-muted-foreground">MythForge</div>
</div>
