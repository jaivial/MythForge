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

  // Responsive shell: on >=lg the sidebar is a permanent rail; below that it
  // is an overlay drawer with a backdrop. `open` drives both, so the e2e
  // testids and the nav expectations stay identical at every width.
  function closeDrawerOnNavigate() {
    void page.url.pathname;
    if (typeof window !== 'undefined' && window.innerWidth < 1024) open = false;
  }

  $effect(() => {
    closeDrawerOnNavigate();
  });

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && open && typeof window !== 'undefined'
        && window.innerWidth < 1024) {
      open = false;
    }
  }

  $effect(() => {
    if (typeof window === 'undefined') return;
    window.addEventListener('keydown', onKeydown);
    return () => window.removeEventListener('keydown', onKeydown);
  });
</script>

{#if $session}
  <div class="flex min-h-dvh">
    {#if open}
      <!-- Mobile drawer backdrop: click (or Escape) dismisses it. -->
      <button
        type="button"
        aria-label="Close navigation"
        class="fixed inset-0 z-40 bg-black/60 lg:hidden"
        onclick={() => (open = false)}
      ></button>
    {/if}
    <aside
      class="{open ? 'w-60' : 'w-16'} shrink-0 flex-col border-r border-border bg-surface
             max-lg:fixed max-lg:inset-y-0 max-lg:left-0 max-lg:z-50
             max-lg:{open ? 'translate-x-0' : '-translate-x-full'}
             max-lg:transition-transform max-lg:duration-200
             lg:flex lg:transition-[width] lg:duration-200"
      data-testid="sidebar"
      aria-label="Workspace navigation"
    >
      <a href="/app" class="flex h-14 items-center gap-3 border-b border-border px-4 app-safe-top">
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
      <header
        class="app-safe-top sticky top-0 z-30 flex h-14 items-center justify-between gap-3
               border-b border-border bg-background/95 px-4 sm:px-6"
      >
        <div class="flex min-w-0 items-center gap-1 sm:gap-3">
          <Button
            variant="ghost"
            size="sm"
            onclick={() => (open = !open)}
            aria-label={open ? 'Close navigation' : 'Open navigation'}
            aria-expanded={open}
            data-testid="toggle-sidebar"
          >
            <svg aria-hidden="true" viewBox="0 0 24 24" class="size-4" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
              <path d="M3 6h18M3 12h18M3 18h18" />
            </svg>
            <span class="hidden sm:inline">{open ? 'Collapse' : 'Expand'}</span>
          </Button>
          <span class="truncate text-sm text-muted-foreground">
            {$session.user?.name ?? 'Workspace'}
          </span>
        </div>
        <div class="flex shrink-0 items-center gap-2">
          <Button
            variant="ghost"
            size="sm"
            onclick={() => (showChat = !showChat)}
            data-testid="toggle-assistant"
            aria-expanded={showChat}
          >
            Assistant
          </Button>
          <Badge variant="outline" class="max-w-40 truncate">{$session.company?.name ?? 'Workspace'}</Badge>
        </div>
      </header>
      <main class="scroll-contain flex-1 overflow-auto p-4 app-safe-bottom sm:p-6">
        {@render children?.()}
      </main>
      {#if showChat}
        <aside
          class="scroll-contain fixed inset-x-0 bottom-0 z-40 flex max-h-[70dvh] flex-col
                 border-t border-border bg-surface p-4 app-safe-bottom md:static md:z-auto
                 md:max-h-none md:w-96 md:shrink-0 md:border-t-0 md:border-l"
          data-testid="assistant-panel"
          role="region"
          aria-label="Assistant"
        >
          <div class="mb-3 flex items-center justify-between">
            <span class="text-sm font-medium">Assistant</span>
            <Button
              variant="ghost"
              size="sm"
              onclick={() => (showChat = false)}
              aria-label="Close assistant"
              data-testid="close-assistant"
            >Close</Button>
          </div>
          <div class="scroll-contain min-h-0 flex-1 overflow-y-auto">
            <ChatPanel />
          </div>
        </aside>
      {/if}
    </div>
  </div>
{/if}
