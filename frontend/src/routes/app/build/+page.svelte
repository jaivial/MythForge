<script lang="ts">
  import { workspace } from '$lib/api/endpoints';
  import { Button, Card, Textarea, Badge, Spinner } from '$lib/components/ui';
  import type { Module } from '$lib/api/types';
  import { animate } from 'motion';

  let prompt = $state('');
  let busy = $state(false);
  let summary = $state('');
  let applied = $state<Module[]>([]);
  let error = $state('');
  let cardEl: HTMLElement | undefined = $state();

  const examples = [
    'Inventario con productos, stock y alertas cuando baje de 5',
    'Pipeline de ventas con etapas y valor por oportunidad',
    'Tickets de soporte con prioridad y cliente',
    'Horas de proyectos por cliente y mes para facturar'
  ];

  async function build() {
    if (!prompt.trim()) return;
    error = summary = '';
    busy = true;
    try {
      const r = await workspace.build(prompt.trim());
      summary = r.summary;
      applied = r.applied;
      prompt = '';
      if (cardEl) animate(cardEl, { opacity: [0, 1], y: [12, 0] }, { duration: 0.35 });
    } catch (e) {
      error = e instanceof Error ? e.message : 'Build failed';
    } finally {
      busy = false;
    }
  }
</script>

<header class="mb-6">
  <h1 class="text-xl font-semibold">Build</h1>
  <p class="text-sm text-muted-foreground">
    Describe what you need in plain language. MythForge designs the modules, fields and screens.
  </p>
</header>

<Card class="flex flex-col gap-4 p-6" data-testid="build-card">
  <Textarea
    data-testid="build-prompt"
    aria-label="Describe what you need"
    bind:value={prompt}
    rows={4}
    placeholder="e.g. Necesito controlar el inventario: productos con SKU, nombre, stock y coste, y compras a proveedores"
  />
  <div class="flex flex-wrap gap-2">
    {#each examples as ex}
      <button
        type="button"
        aria-pressed={prompt === ex}
        class="min-h-9 rounded-full border coarse:min-h-11 border-border px-3 py-1.5 text-xs
               text-muted-foreground hover:bg-muted hover:text-foreground
               aria-pressed:border-silver-dim aria-pressed:text-foreground"
        onclick={() => (prompt = ex)}>{ex}</button>
    {/each}
  </div>
  <div class="flex items-center gap-3">
    <Button onclick={build} disabled={busy || !prompt.trim()} data-testid="build-submit">
      {#if busy}<Spinner size={14} />{/if}
      {busy ? 'Designingâ¦' : 'Build'}
    </Button>
    {#if error}<span data-testid="build-error" class="text-sm text-danger">{error}</span>{/if}
  </div>
</Card>

{#if summary}
  <div bind:this={cardEl}><Card class="mt-6 flex flex-col gap-4 p-6" data-testid="build-result">
    <p class="text-sm">{summary}</p>
    <div class="flex flex-col gap-2">
      {#each applied as m (m.slug)}
        <div class="flex items-center justify-between rounded-md border border-border px-3 py-2">
          <span class="text-sm font-medium">{m.name}</span>
          <Badge variant="outline">{m.entities.length} entities</Badge>
        </div>
      {/each}
    </div>
    <a href="/app" class="text-sm underline text-silver" data-testid="goto-workspace">
      Go to workspace
    </a>
  </Card></div>
{/if}
