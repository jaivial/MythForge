<script lang="ts">
  import { google, automations, workspace } from '$lib/api/endpoints';
  import { Badge, Button, Card, Separator, Spinner } from '$lib/components/ui';
  import { onMount } from 'svelte';
  import { session, logout } from '$lib/stores/session';

  let g = $state<{ connected: boolean; email?: string | null; configured: boolean } | null>(null);
  let autos = $state<{ id: string; name: string; run_count: number; is_active: boolean }[]>([]);
  let bp = $state<{ modules: { slug: string; name: string }[] } | null>(null);
  let loading = $state(true);
  let error = $state('');

  onMount(async () => {
    try {
      const [gs, as, b] = await Promise.all([
        google.status(),
        automations.list().catch(() => ({ items: [] })),
        workspace.blueprint()
      ]);
      g = gs;
      autos = as.items;
      bp = b;
    } finally {
      loading = false;
    }
  });

  async function connect() {
    const c = await google.connect();
    const token = localStorage.getItem('mf_token');
    if (!token) {
      error = 'Session missing — sign in again before connecting Google.';
      return;
    }
    const url = new URL('https://accounts.google.com/o/oauth2/v2/auth');
    url.searchParams.set('client_id', c.client_id);
    url.searchParams.set('redirect_uri', c.redirect_url);
    url.searchParams.set('response_type', 'code');
    url.searchParams.set('scope', c.scope);
    url.searchParams.set('access_type', 'offline');
    url.searchParams.set('prompt', 'consent');
    url.searchParams.set('state', token);
    location.href = url.toString();
  }
</script>

<header class="mb-6">
  <h1 class="text-xl font-semibold">Settings</h1>
  <p class="text-sm text-muted-foreground">Account, Google integration and automations.</p>
</header>

{#if loading}
  <div class="flex items-center gap-2 text-sm text-muted-foreground"><Spinner size={14} /> Loadingâ¦</div>
{:else}
  <div class="flex flex-col gap-6 max-w-2xl">
    <Card class="flex flex-col gap-3 p-6">
      <h2 class="font-medium">Account</h2>
      <Separator />
      <div class="flex items-center justify-between">
        <div class="flex flex-col">
          <span>{$session?.user?.name}</span>
          <span class="text-xs text-muted-foreground">{$session?.user?.email}</span>
        </div>
        <Button variant="outline" size="sm" onclick={() => logout()}>Sign out</Button>
      </div>
    </Card>

    <Card class="flex flex-col gap-3 p-6" data-testid="google-card">
      <div class="flex items-center justify-between">
        <h2 class="font-medium">Google</h2>
        {#if g?.connected}
          <Badge variant="success" data-testid="google-connected">Connected</Badge>
        {:else}
          <Badge>Not connected</Badge>
        {/if}
      </div>
      <Separator />
      {#if g?.connected}
        <p class="text-sm text-muted-foreground" data-testid="google-email">
          {g.email ?? 'Connected account'}
        </p>
      {:else if g?.configured}
        <p class="text-sm text-muted-foreground">
          Connect Calendar and Gmail so agents and automations can read events and send email.
        </p>
        <Button size="sm" onclick={connect} data-testid="google-connect">Connect Google</Button>
      {:else}
        <p class="text-sm text-muted-foreground">
          Google integration is not configured on this server.
        </p>
      {/if}
    </Card>

    <Card class="flex flex-col gap-3 p-6" data-testid="automations-card">
      <h2 class="font-medium">Automations</h2>
      <Separator />
      {#each autos as a (a.id)}
        <div class="flex items-center justify-between" data-testid="automation-{a.id}">
          <span class="text-sm">{a.name}</span>
          <Badge variant="outline">{a.run_count} runs</Badge>
        </div>
      {:else}
        <p class="text-sm text-muted-foreground">
          No automations yet. Ask the assistant to create one.
        </p>
      {/each}
    </Card>

    <Card class="flex flex-col gap-3 p-6" data-testid="blueprint-card">
      <h2 class="font-medium">Workspace modules</h2>
      <Separator />
      <div class="flex flex-wrap gap-2">
        {#each bp?.modules ?? [] as m}
          <Badge variant="outline">{m.name}</Badge>
        {/each}
      </div>
    </Card>
  </div>
{/if}
