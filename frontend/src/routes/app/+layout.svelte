<script lang="ts">
  import { page } from '$app/state';
  import { session, logout } from '$lib/stores/session';
  import { workspace } from '$lib/api/endpoints';
  import type { Blueprint, Module } from '$lib/api/types';
  import { Badge, Button } from '$lib/components/ui';
  import ChatPanel from '$lib/components/app/ChatPanel.svelte';
  import { onMount, untrack } from 'svelte';
  import { goto } from '$app/navigation';

  let { children } = $props();
  let blueprint = $state<Blueprint | null>(null);
  let open = $state(false);
  let showChat = $state(true);

  $effect(() => {
    if (!$session) goto('/login');
  });

  async function loadBlueprint() {
    if (!$session) return;
    try {
      blueprint = await workspace.blueprint();
    } catch {
      blueprint = { modules: [] };
    }
  }

  onMount(loadBlueprint);
  // the blueprint changes whenever the builder provisions a new module, so
  // refresh it on every navigation to keep the sidebar in sync. `untrack`
  // keeps loadBlueprint's own reads from becoming dependencies, and a single
  // in-flight guard prevents the duplicate first-load request.
  let loadingBlueprint = false;
  async function refreshBlueprint() {
    if (loadingBlueprint) return;
    loadingBlueprint = true;
    try {
      await loadBlueprint();
    } finally {
      loadingBlueprint = false;
    }
  }
  $effect(() => {
    void page.url.pathname;
    if (!$session) return;
    untrack(() => void refreshBlueprint());
  });

  const modules = $derived(blueprint?.modules ?? []);
  const current = $derived(page.url.pathname);
</script>

{#if $session}
  <div class="flex min-h-screen">
    <aside
      class="{open ? 'w-60' : 'w-16'} shrink-0 flex flex-col border-r border-border bg-surface
             transition-[width] duration-200"
      data-testid="sidebar"
    >
      <a href="/app" class="flex h-14 items-center gap-3 border-b border-border px-4">
        <img src="/favicon.svg" alt="" class="size-6 shrink-0" />
        {#if open}<span class="text-sm font-semibold">MythForge</span>{/if}
      </a>

      <nav class="flex flex-1 flex-col gap-1 p-2">
        <a href="/app/build" data-testid="nav-build"
          class="rounded-md px-3 py-2 text-sm {current.startsWith('/app/build')
            ? 'bg-muted text-foreground'
            : 'text-muted-foreground hover:bg-muted hover:text-foreground'}">
          Build
        </a>
        <a href="/app/agents" data-testid="nav-agents"
          class="rounded-md px-3 py-2 text-sm {current.startsWith('/app/agents')
            ? 'bg-muted text-foreground'
            : 'text-muted-foreground hover:bg-muted hover:text-foreground'}">
          Agents
        </a>

        {#if modules.length}
          <div class="mt-3 px-3 text-xs uppercase tracking-wide text-muted-foreground">
            {#if open}Modules{/if}
          </div>
          {#each modules as m (m.slug)}
            <a
              href="/app/m/{m.slug}/{m.entities[0]?.slug ?? ''}"
              data-testid="nav-module-{m.slug}"
              aria-label={m.name}
              title={m.name}
              class="rounded-md px-3 py-2 text-sm {current.startsWith('/app/m/' + m.slug)
                ? 'bg-muted text-foreground'
                : 'text-muted-foreground hover:bg-muted hover:text-foreground'}"
            >
              {#if open}{m.name}{/if}
            </a>
          {/each}
        {/if}
      </nav>

      <div class="border-t border-border p-2">
        <a href="/app/settings" data-testid="nav-settings"
          class="block rounded-md px-3 py-2 text-sm text-muted-foreground hover:bg-muted">
          Settings
        </a>
        <Button variant="ghost" size="sm" class="w-full justify-start" onclick={() => logout()}>
          Sign out
        </Button>
      </div>
    </aside>

    <div class="flex min-w-0 flex-1 flex-col">
      <header class="flex h-14 items-center justify-between border-b border-border px-6">
        <div class="flex items-center gap-3">
          <Button variant="ghost" size="sm" onclick={() => (open = !open)} aria-label="Toggle sidebar">
            {open ? 'Collapse' : 'Expand'}
          </Button>
          <Button variant="ghost" size="sm" onclick={() => (showChat = !showChat)} data-testid="toggle-assistant">
            Assistant
          </Button>
          <span class="text-sm text-muted-foreground">
            {$session.user?.name ?? 'Workspace'}
          </span>
        </div>
        <Badge variant="outline">{$session.company?.name ?? 'Workspace'}</Badge>
      </header>
      <main class="flex-1 overflow-auto p-6">
        {@render children?.()}
      </main>
      {#if showChat}
        <aside class="hidden w-96 shrink-0 flex-col border-l border-border bg-surface p-4 xl:flex"
          data-testid="assistant-panel">
          <div class="mb-3 flex items-center justify-between">
            <span class="text-sm font-medium">Assistant</span>
            <Button variant="ghost" size="sm" onclick={() => (showChat = false)}>Close</Button>
          </div>
          <div class="min-h-0 flex-1">
            <ChatPanel />
          </div>
        </aside>
      {/if}
    </div>
  </div>
{/if}
