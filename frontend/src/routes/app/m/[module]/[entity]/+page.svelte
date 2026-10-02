<script lang="ts">
  import { page } from '$app/state';
  import { workspace, data } from '$lib/api/endpoints';
  import type { Entity, Field, Record_ } from '$lib/api/types';
  import { Badge, Button, Card, Input, Separator, Spinner } from '$lib/components/ui';
  import { fmtDate, fmtValue } from '$lib/utils';
  import { onMount } from 'svelte';

  let module = $derived(page.params.module ?? '');
  let entitySlug = $derived(page.params.entity ?? '');
  let entity = $state<Entity | null>(null);
  let views = $state<string[]>([]);
  let view = $state('table');
  let records = $state<Record_[]>([]);
  let total = $state(0);
  let search = $state('');
  let loading = $state(true);
  let error = $state('');
  let saving = $state(false);
  let formOpen = $state(false);
  let form = $state<Record<string, string>>({});
  let formError = $state('');

  async function load() {
    loading = true;
    try {
      const bp = await workspace.blueprint();
      const m = bp.modules.find((x) => x.slug === module);
      const e = m?.entities.find((x) => x.slug === entitySlug) ?? m?.entities[0];
      if (!e) {
        error = 'Entity not found';
        return;
      }
      entity = e;
      views = Array.from(new Set(['table', ...e.views.map((v) => v.kind)]));
      const p = await data.list(module, e.slug, search ? { q: search } : {});
      records = p.items;
      total = p.total;
      error = '';
    } catch (err) {
      error = err instanceof Error ? err.message : 'Load failed';
    } finally {
      loading = false;
    }
  }

  onMount(load);
  $effect(() => {
    void entitySlug;
    void module;
    load();
  });

  function openCreate() {
    form = {};
    formError = '';
    formOpen = true;
  }

  async function save() {
    if (!entity) return;
    saving = true;
    formError = '';
    try {
      const body: Record<string, unknown> = {};
      for (const f of entity.fields) {
        const v = form[f.slug];
        if (v === undefined || v === '') continue;
        body[f.slug] =
          f.type === 'number' || f.type === 'money' ? Number(v) : v;
      }
      await data.create(module, entity.slug, body);
      formOpen = false;
      await load();
    } catch (e) {
      formError = e instanceof Error ? e.message : 'Save failed';
    } finally {
      saving = false;
    }
  }

  async function remove(id: string) {
    if (!entity || !confirm('Delete this record?')) return;
    await data.remove(module, entity.slug, id);
    await load();
  }

  /** Kanban grouping field: first select field. */
  const groupField = $derived(
    entity?.fields.find((f) => f.type === 'select' && f.choices?.length)
  );
  const columns = $derived(groupField?.choices ?? []);

  function byColumn(choice: string): Record_[] {
    return records.filter((r) => String(r.data[groupField?.slug ?? '']) === choice);
  }
</script>

<header class="mb-6 flex flex-wrap items-end justify-between gap-3">
  <div>
    <h1 class="text-xl font-semibold capitalize">{entity?.name ?? module}</h1>
    <p class="text-sm text-muted-foreground">{total} records</p>
  </div>
  <div class="flex items-center gap-2">
    {#if views.length > 1}
      <div class="flex rounded-md border border-border" data-testid="views">
        {#each views as v}
          <button type="button"
            class="px-3 py-1.5 text-xs capitalize {view === v ? 'bg-muted text-foreground' : 'text-muted-foreground hover:bg-muted'}"
            onclick={() => (view = v)}>{v}</button>
        {/each}
      </div>
    {/if}
    <Button size="sm" onclick={openCreate} data-testid="new-record">New</Button>
  </div>
</header>

{#if error}
  <p data-testid="entity-error" class="text-sm text-danger">{error}</p>
{:else if loading}
  <div class="flex items-center gap-2 text-sm text-muted-foreground"><Spinner size={14} /> Loadingâ¦</div>
{:else if view === 'kanban' && groupField}
  <div class="flex gap-4 overflow-x-auto pb-4" data-testid="kanban">
    {#each columns as col}
      <div class="w-64 shrink-0 rounded-lg border border-border bg-surface p-3">
        <div class="mb-3 flex items-center justify-between">
          <span class="text-sm font-medium">{col}</span>
          <Badge>{byColumn(col).length}</Badge>
        </div>
        <div class="flex flex-col gap-2">
          {#each byColumn(col) as r (r.id)}
            <div class="rounded-md border border-border bg-surface-2 p-3 text-sm">
              {fmtValue(r.data[entity?.fields[0]?.slug ?? ''])}
            </div>
          {/each}
        </div>
      </div>
    {/each}
  </div>
{:else}
  <Card data-testid="records-table">
    <div class="flex items-center gap-2 border-b border-border p-3">
      <Input data-testid="search" bind:value={search} placeholder="Searchâ¦"
        oninput={() => setTimeout(load, 250)} class="max-w-xs" />
    </div>
    <div class="overflow-x-auto">
      <table class="w-full text-sm">
        <thead>
          <tr class="border-b border-border text-left text-xs uppercase tracking-wide text-muted-foreground">
            {#each entity?.fields.slice(0, 6) ?? [] as f}
              <th class="px-3 py-2 font-medium">{f.name}</th>
            {/each}
            <th class="px-3 py-2"></th>
          </tr>
        </thead>
        <tbody>
          {#each records as r (r.id)}
            <tr class="border-b border-border/60 hover:bg-muted/40" data-testid="record-row">
              {#each entity?.fields.slice(0, 6) ?? [] as f}
                <td class="px-3 py-2">
                  {#if f.type === 'select'}<Badge variant="outline">{fmtValue(r.data[f.slug])}</Badge>
                  {:else if f.type === 'money'}<span class="tabular-nums">{fmtValue(r.data[f.slug])}</span>
                  {:else}{fmtValue(r.data[f.slug])}{/if}
                </td>
              {/each}
              <td class="px-3 py-2 text-right">
                <Button variant="ghost" size="sm" onclick={() => remove(r.id)}>Delete</Button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    {#if records.length === 0}
      <p class="p-6 text-center text-sm text-muted-foreground">No records yet.</p>
    {/if}
  </Card>
{/if}

{#if formOpen && entity}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
    role="dialog" aria-modal="true" data-testid="record-form">
    <Card class="w-full max-w-lg p-6">
      <h2 class="mb-4 text-lg font-semibold">New {entity.name}</h2>
      <form onsubmit={(e) => { e.preventDefault(); save(); }} class="flex flex-col gap-4">
        {#each entity.fields as f (f.slug)}
          <label class="flex flex-col gap-1 text-sm">
            <span class="text-muted-foreground">{f.name}{#if f.required} *{/if}</span>
            {#if f.type === 'select'}
              <select bind:value={form[f.slug]} required={f.required}
                class="h-9 rounded-md border border-border bg-surface-2 px-3 text-sm">
                <option value=""></option>
                {#each f.choices ?? [] as c}<option value={c}>{c}</option>{/each}
              </select>
            {:else}
              <Input data-testid="field-{f.slug}"
                type={f.type === 'number' || f.type === 'money' ? 'number'
                  : f.type === 'date' ? 'date' : f.type === 'email' ? 'email' : 'text'}
                bind:value={form[f.slug]} required={f.required} />
            {/if}
          </label>
        {/each}
        {#if formError}<p class="text-sm text-danger" data-testid="form-error">{formError}</p>{/if}
        <div class="flex justify-end gap-2">
          <Button variant="ghost" onclick={() => (formOpen = false)}>Cancel</Button>
          <Button type="submit" disabled={saving} data-testid="save-record">
            {#if saving}<Spinner size={14} />{/if} Save
          </Button>
        </div>
      </form>
    </Card>
  </div>
{/if}
