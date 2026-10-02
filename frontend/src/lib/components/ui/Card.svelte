<script lang="ts">
  import { cn } from '$lib/utils';
  let {
    class: klass = '',
    children,
    ...rest
  }: { class?: string; children?: import('svelte').Snippet; [key: string]: unknown } = $props();
</script>

<div {...rest} class={cn('rounded-lg border border-border bg-surface', klass)}>
  {@render children?.()}
</div>
