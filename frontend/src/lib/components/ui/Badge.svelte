<script lang="ts">
  import { cn } from '$lib/utils';
  let {
    variant = 'default',
    class: klass = '',
    children,
    ...rest
  }: {
    variant?: 'default' | 'outline' | 'success' | 'warning' | 'danger';
    class?: string;
    children?: import('svelte').Snippet;
    [key: string]: unknown;
  } = $props();
  const variants: Record<string, string> = {
    default: 'bg-muted text-muted-foreground border-border',
    outline: 'bg-transparent border-border-strong text-silver',
    success: 'bg-success/15 text-success border-success/30',
    warning: 'bg-warning/15 text-warning border-warning/30',
    danger: 'bg-danger/15 text-danger border-danger/30'
  };
</script>

<span
  {...rest}
  class={cn(
    'inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-medium',
    variants[variant],
    klass
  )}
>
  {@render children?.()}
</span>
