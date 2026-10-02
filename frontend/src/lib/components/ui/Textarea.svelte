<script lang="ts">
  import { cn } from '$lib/utils';
  let {
    value = $bindable(''),
    placeholder = '',
    id,
    rows = 3,
    class: klass = '',
    ...rest
  }: {
    value?: string;
    placeholder?: string;
    id?: string;
    rows?: number;
    class?: string;
    [key: string]: unknown;
  } = $props();
</script>

<textarea
  {...rest}
  {id}
  {rows}
  {placeholder}
  bind:value
  class={cn(
    'flex w-full rounded-md border border-border bg-surface-2 px-3 py-2 text-sm',
    'placeholder:text-muted-foreground focus-visible:outline-none focus-visible:border-silver-dim',
    klass
  )}
></textarea>
