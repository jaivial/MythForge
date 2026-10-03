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
    invalid = false,
    ...rest
  }: {
    value?: string | number;
    type?: string;
    placeholder?: string;
    id?: string;
    name?: string;
    required?: boolean;
    class?: string;
    invalid?: boolean;
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
  aria-invalid={invalid || undefined}
  class={cn(
    'flex min-h-9 w-full rounded-md border border-border bg-surface-2 px-3 py-1 text-base',
    'placeholder:text-muted-foreground focus-visible:outline-none',
    'focus-visible:border-silver focus-visible:ring-2 focus-visible:ring-silver/30',
    'aria-invalid:border-danger disabled:cursor-not-allowed disabled:opacity-50',
    'coarse:min-h-11',
    klass
  )}
/>
