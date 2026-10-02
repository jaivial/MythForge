<script lang="ts">
  import { cn } from '$lib/utils';
  let {
    value = $bindable(''),
    type = 'text',
    placeholder = '',
    id,
    name,
    required = false,
    class: klass = '',
    oninput,
    ...rest
  }: {
    value?: string | number;
    type?: string;
    placeholder?: string;
    id?: string;
    name?: string;
    required?: boolean;
    class?: string;
    oninput?: (e: Event) => void;
    [key: string]: unknown;
  } = $props();
</script>

<input
  {...rest}
  {id}
  {name}
  {type}
  {placeholder}
  {required}
  bind:value
  oninput={oninput}
  class={cn(
    'flex h-9 w-full rounded-md border border-border bg-surface-2 px-3 py-1 text-sm',
    'placeholder:text-muted-foreground focus-visible:outline-none focus-visible:border-silver-dim',
    'disabled:cursor-not-allowed disabled:opacity-50',
    klass
  )}
/>
