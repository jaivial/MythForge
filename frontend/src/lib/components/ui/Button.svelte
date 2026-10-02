<script lang="ts">
  import { cn } from '$lib/utils';
  let {
    variant = 'default',
    size = 'md',
    type = 'button',
    disabled = false,
    class: klass = '',
    onclick,
    children,
    ...rest
  }: {
    variant?: 'default' | 'outline' | 'ghost' | 'danger';
    size?: 'sm' | 'md' | 'icon';
    type?: 'button' | 'submit';
    disabled?: boolean;
    class?: string;
    onclick?: (e: MouseEvent) => void;
    children?: import('svelte').Snippet;
    [key: string]: unknown;
  } = $props();

  const variants: Record<string, string> = {
    default: 'bg-primary text-primary-foreground hover:bg-silver-dim',
    outline: 'border border-border-strong bg-transparent text-foreground hover:bg-muted',
    ghost: 'bg-transparent text-muted-foreground hover:bg-muted hover:text-foreground',
    danger: 'bg-danger/90 text-white hover:bg-danger'
  };
  const sizes: Record<string, string> = {
    sm: 'h-8 px-3 text-xs',
    md: 'h-9 px-4 text-sm',
    icon: 'size-9 p-0'
  };
</script>

<button
  {type}
  {disabled}
  {onclick}
  {...rest}
  class={cn(
    'inline-flex items-center justify-center gap-2 rounded-md font-medium transition-colors',
    'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-silver',
    'disabled:pointer-events-none disabled:opacity-50',
    variants[variant],
    sizes[size],
    klass
  )}
>
  {@render children?.()}
</button>
