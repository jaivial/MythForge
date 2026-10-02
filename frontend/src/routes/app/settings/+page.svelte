<script lang="ts">
  import { google, automations, workspace } from '$lib/api/endpoints';
  import { Input } from '$lib/components/ui';
  import { Badge, Button, Card, Separator, Spinner } from '$lib/components/ui';
  import { onMount } from 'svelte';
  import { session, logout } from '$lib/stores/session';

  let g = $state<{ connected: boolean; email?: string | null; configured: boolean } | null>(null);
  let autos = $state<
    {
      id: string;
      name: string;
      description: string;
      trigger: Record<string, unknown>;
      run_count: number;
      is_active: boolean;
    }[]
  >([]);
  let autoPrompt = $state('');
  let autoDraft = $state<{
    name: string;
    description: string;
    trigger: Record<string, unknown> & { kind?: string; module?: string; entity?: string; interval_seconds?: number };
    action: Record<string, unknown> & { prompt?: string };
    agent_id: string | null;
  } | null>(null);
  let autoBusy = $state(false);
  let autoError = $state('');
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

  /** Draft an automation from a prompt, then create it on confirm. */
  async function composeAuto() {
    if (!autoPrompt.trim()) return;
    autoBusy = true;
    autoError = '';
    try {
      const r = await automations.compose(autoPrompt.trim());
      autoDraft = r.draft;
    } catch (e) {
      autoError = e instanceof Error ? e.message : 'Compose failed';
    } finally {
      autoBusy = false;
    }
  }

  async function createAuto() {
    if (!autoDraft) return;
    autoBusy = true;
    try {
      await automations.create(autoDraft);
      autoDraft = null;
      autoPrompt = '';
      const as = await automations.list();
      autos = as.items;
    } finally {
      autoBusy = false;
    }
  }

  async function runAuto(id: string) {
    await automations.runNow(id);
    const as = await automations.list();
    autos = as.items;
  }

  async function removeAuto(id: string) {
    await automations.remove(id);
    autos = (await automations.list()).items;
  }

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
      <div class="flex gap-2">
        <Input data-testid="auto-prompt" bind:value={autoPrompt}
          placeholder="e.g. cada hora revisa stock bajo y avisame" />
        <Button size="sm" onclick={composeAuto} disabled={autoBusy || !autoPrompt.trim()}
          data-testid="auto-compose">{autoBusy ? '...' : 'Draft'}</Button>
      </div>
      {#if autoError}<p class="text-sm text-danger" data-testid="auto-error">{autoError}</p>{/if}
      {#if autoDraft}
        <div class="flex flex-col gap-2 rounded-md border border-border p-3" data-testid="auto-draft">
          <span class="text-sm font-medium">{autoDraft.name}</span>
          <p class="text-xs text-muted-foreground">{autoDraft.description}</p>
          <div class="flex flex-wrap gap-2 text-xs text-muted-foreground">
            <Badge variant="outline">{String(autoDraft.trigger.kind ?? 'schedule')}</Badge>
            {#if autoDraft.trigger.module}
              <Badge variant="outline">{String(autoDraft.trigger.module)}/{String(autoDraft.trigger.entity ?? '')}</Badge>
            {/if}
            {#if autoDraft.trigger.interval_seconds}
              <Badge variant="outline">every {String(autoDraft.trigger.interval_seconds)}s</Badge>
            {/if}
          </div>
          <p class="text-xs text-muted-foreground">{String(autoDraft.action.prompt ?? '')}</p>
          <Button size="sm" onclick={createAuto} disabled={autoBusy} data-testid="auto-create">
            Create automation
          </Button>
        </div>
      {/if}
      {#each autos as a (a.id)}
        <div class="flex items-center justify-between" data-testid="automation-{a.id}">
          <div class="flex flex-col">
            <span class="text-sm">{a.name}</span>
            <span class="text-xs text-muted-foreground">{String(a.trigger?.kind ?? '')}</span>
          </div>
          <div class="flex items-center gap-2">
            <Badge variant="outline">{a.run_count} runs</Badge>
            <Button size="sm" variant="ghost" onclick={() => runAuto(a.id)}>Run</Button>
            <Button size="sm" variant="ghost" onclick={() => removeAuto(a.id)}>Delete</Button>
          </div>
        </div>
      {:else}
        <p class="text-sm text-muted-foreground">No automations yet.</p>
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
