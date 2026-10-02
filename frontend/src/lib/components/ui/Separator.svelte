<script lang="ts">
  import { cn } from '$lib/utils';
  let { class: klass = '' }: { class?: string } = $props();
</script>

<div role="separator" aria-orientation="horizontal" class={cn('h-px w-full bg-border', klass)}></div>
