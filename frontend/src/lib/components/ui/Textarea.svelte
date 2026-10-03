<script lang="ts">
  import { cn } from '$lib/utils';
  let {
    value = $bindable(''),
    placeholder = '',
    id,
    rows = 3,
    class: klass = '',
    invalid = false,
    ...rest
  }: {
    value?: string;
    placeholder?: string;
    id?: string;
    rows?: number;
    class?: string;
    invalid?: boolean;
    [key: string]: unknown;
  } = $props();
</script>

<textarea
  {...rest}
  {id}
  {rows}
  {placeholder}
  bind:value
  aria-invalid={invalid || undefined}
  class={cn(
    'flex w-full rounded-md border border-border bg-surface-2 px-3 py-2 text-base',
    'placeholder:text-muted-foreground focus-visible:outline-none',
    'focus-visible:border-silver focus-visible:ring-2 focus-visible:ring-silver/30',
    'aria-invalid:border-danger',
    klass
  )}
></textarea>
