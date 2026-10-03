<script lang="ts">
  import { mascots, agents } from '$lib/api/endpoints';
  import type { Agent, Mascot } from '$lib/api/types';
  import { Badge, Button, Card, Input, Separator, Spinner, Textarea } from '$lib/components/ui';
  import { onMount } from 'svelte';

  let mascotList = $state<Mascot[]>([]);
  let agentList = $state<Agent[]>([]);
  let prompt = $state('');
  let composing = $state(false);
  let draft = $state<{ name: string; description: string; system_prompt: string } | null>(null);
  let draftTools = $state<string[]>([]);
  let error = $state('');
  let chatLogs = $state<Record<string, { role: 'user' | 'assistant'; text: string }[]>>({});
  let chatInputs = $state<Record<string, string>>({});
  let chatting = $state<string | null>(null);
  let mascotName = $state('');
  let mascotPersona = $state('');
  let creatingMascot = $state(false);

  async function refresh() {
    const [m, a] = await Promise.all([mascots.list(), agents.list()]);
    mascotList = m.items;
    agentList = a.items;
    // Seed per-agent state so `bind:value` never binds to undefined.
    for (const agent of agentList) {
      if (chatInputs[agent.slug] === undefined) chatInputs[agent.slug] = '';
      if (chatLogs[agent.slug] === undefined) chatLogs[agent.slug] = [];
    }
  }

  onMount(refresh);

  async function compose() {
    if (!prompt.trim()) return;
    composing = true;
    error = '';
    try {
      const r = await agents.compose(prompt.trim());
      draft = {
        name: String(r.draft.name ?? 'New agent'),
        description: String(r.draft.description ?? ''),
        system_prompt: String(r.draft.system_prompt ?? '')
      };
      draftTools = r.tools;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Compose failed';
    } finally {
      composing = false;
    }
  }

  async function createAgent() {
    if (!draft) return;
    await agents.create({
      name: draft.name,
      description: draft.description,
      system_prompt: draft.system_prompt,
      tools: draftTools,
      mascot_id: mascotList[0]?.id
    });
    draft = null;
    prompt = '';
    await refresh();
  }

  async function createMascot() {
    if (!mascotName.trim()) return;
    creatingMascot = true;
    try {
      await mascots.create({
        name: mascotName.trim(),
        persona: mascotPersona.trim(),
        tagline: mascotPersona.trim().slice(0, 80)
      });
      mascotName = mascotPersona = '';
      await refresh();
    } finally {
      creatingMascot = false;
    }
  }

  /** Per-agent conversation, keyed by agent slug. */
  async function run(agentSlug: string) {
    const message = (chatInputs[agentSlug] ?? '').trim();
    if (!message) return;
    chatInputs[agentSlug] = '';
    chatLogs[agentSlug] = [
      ...(chatLogs[agentSlug] ?? []),
      { role: 'user', text: message }
    ];
    chatting = agentSlug;
    try {
      const r = await agents.run(agentSlug, message);
      chatLogs[agentSlug] = [
        ...chatLogs[agentSlug],
        { role: 'assistant', text: r.text }
      ];
    } catch (e) {
      chatLogs[agentSlug] = [
        ...chatLogs[agentSlug],
        { role: 'assistant', text: e instanceof Error ? e.message : 'Run failed' }
      ];
    } finally {
      chatting = null;
    }
  }
</script>

<header class="mb-6">
  <h1 class="text-xl font-semibold">Mascots &amp; agents</h1>
  <p class="text-sm text-muted-foreground">
    A mascot is a persona; agents live under it and carry their own tools.
  </p>
</header>

<div class="grid gap-6 lg:grid-cols-2">
  <section class="flex flex-col gap-4">
    <h2 class="text-sm font-medium uppercase tracking-wide text-muted-foreground">Mascots</h2>
    {#each mascotList as m (m.id)}
      <Card class="flex items-start gap-3 p-4" data-testid="mascot-{m.slug}">
        <div class="flex size-10 shrink-0 items-center justify-center rounded-md border border-border bg-surface-2 text-lg">
          {m.glyph ?? 'â'}
        </div>
        <div class="flex flex-col gap-1">
          <span class="font-medium">{m.name}</span>
          <span class="text-xs text-muted-foreground">{m.tagline ?? m.persona}</span>
        </div>
      </Card>
    {/each}
    {#if mascotList.length === 0}
      <p class="text-sm text-muted-foreground">No mascots yet.</p>
    {/if}
    <Card class="flex flex-col gap-3 p-4">
      <Input data-testid="mascot-name" aria-label="Mascot name" bind:value={mascotName} placeholder="Mascot name" />
      <Textarea data-testid="mascot-persona" bind:value={mascotPersona} rows={2}
        aria-label="Persona"
        placeholder="Persona: how it talks, what it cares about" />
      <Button size="sm" onclick={createMascot} disabled={creatingMascot || !mascotName.trim()}
        data-testid="create-mascot">Create mascot</Button>
    </Card>
  </section>

  <section class="flex flex-col gap-4">
    <h2 class="text-sm font-medium uppercase tracking-wide text-muted-foreground">Agents</h2>
    {#each agentList as a (a.id)}
      <Card class="flex flex-col gap-2 p-4" data-testid="agent-{a.slug}">
        <div class="flex items-center justify-between">
          <span class="font-medium">{a.name}</span>
          <Badge variant="outline">{a.tools.length} tools</Badge>
        </div>
        <p class="text-xs text-muted-foreground">{a.description}</p>
        <div class="flex gap-1 flex-wrap">
          {#each a.tools as t}<Badge>{t}</Badge>{/each}
        </div>
        <Separator />
        <div class="flex gap-2">
          <Input data-testid="agent-input-{a.slug}" aria-label="Message this agent" bind:value={chatInputs[a.slug]}
            placeholder="Message this agent" />
          <Button size="sm" onclick={() => run(a.slug)} disabled={chatting !== null}
            data-testid="run-agent-{a.slug}">
            {#if chatting === a.slug}<Spinner size={12} />{/if}Run
          </Button>
        </div>
        {#if chatLogs[a.slug]?.length}
          <div class="flex flex-col gap-2" data-testid="agent-chat-{a.slug}">
            {#each chatLogs[a.slug] as c, i (i)}
              <div class="rounded-md px-3 py-2 text-sm {c.role === 'user'
                ? 'bg-muted text-foreground' : 'bg-surface-2 text-muted-foreground'}">{c.text}</div>
            {/each}
          </div>
        {/if}
      </Card>
    {/each}

    <Card class="flex flex-col gap-3 p-4" data-testid="compose-card">
      <span class="text-sm font-medium">Compose an agent from a prompt</span>
      <Textarea data-testid="compose-prompt" aria-label="Describe the agent you need" bind:value={prompt} rows={3}
        placeholder="e.g. an agent that reads low-stock products and drafts purchase orders" />
      <Button size="sm" onclick={compose} disabled={composing || !prompt.trim()} data-testid="compose-submit">
        {#if composing}<Spinner size={12} />{/if}Draft agent
      </Button>
      {#if error}<p class="text-sm text-danger" data-testid="compose-error">{error}</p>{/if}
      {#if draft}
        <div class="flex flex-col gap-2 rounded-md border border-border p-3" data-testid="agent-draft">
          <span class="font-medium">{draft.name}</span>
          <p class="text-xs text-muted-foreground">{draft.description}</p>
          <p class="max-h-24 overflow-auto text-xs text-muted-foreground">{draft.system_prompt}</p>
          <div class="flex flex-wrap gap-1">
            {#each draftTools as t}<Badge variant="outline">{t}</Badge>{/each}
          </div>
          <Button size="sm" onclick={createAgent} data-testid="create-agent">Create agent</Button>
        </div>
      {/if}
    </Card>
  </section>
</div>
