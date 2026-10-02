<script lang="ts">
  /** The AI assistant panel: prompt to build, ask about data, create automations. */
  import { chat } from '$lib/api/endpoints';
  import { Button, Spinner, Textarea } from '$lib/components/ui';
  import { animate } from 'motion';

  let { placeholder = 'Ask the assistant or describe what to build' } = $props();

  let log = $state<{ role: 'user' | 'assistant'; text: string; tools?: string[] }[]>([]);
  let input = $state('');
  let busy = $state(false);
  let error = $state('');
  let listEl: HTMLElement | undefined = $state();

  async function send() {
    if (!input.trim() || busy) return;
    const message = input.trim();
    input = '';
    error = '';
    log.push({ role: 'user', text: message });
    busy = true;
    try {
      const r = await chat.send(message);
      log.push({
        role: 'assistant',
        text: r.text,
        tools: r.tool_calls.map((t) => t.name)
      });
    } catch (e) {
      error = e instanceof Error ? e.message : 'Assistant unavailable';
    } finally {
      busy = false;
      if (listEl) {
        animate(listEl, { opacity: [0.6, 1] }, { duration: 0.2 });
        listEl.scrollTop = listEl.scrollHeight;
      }
    }
  }
</script>

<div class="flex h-full flex-col gap-3">
  <div bind:this={listEl} class="flex flex-1 flex-col gap-3 overflow-y-auto pr-1" data-testid="chat-log">
    {#if log.length === 0}
      <p class="text-sm text-muted-foreground">
        Ask anything about your workspace, or describe a module you need.
      </p>
    {/if}
    {#each log as m, i (i)}
      <div class="max-w-[85%] rounded-lg px-3 py-2 text-sm whitespace-pre-wrap {m.role === 'user'
        ? 'self-end bg-primary text-primary-foreground'
        : 'self-start border border-border bg-surface text-foreground'}"
        data-testid="chat-{m.role}">
        {m.text}
        {#if m.tools?.length}
          <div class="mt-2 flex flex-wrap gap-1">
            {#each m.tools as t}<span class="rounded border border-border px-1 text-[10px] text-muted-foreground">{t}</span>{/each}
          </div>
        {/if}
      </div>
    {/each}
    {#if busy}
      <div class="self-start flex items-center gap-2 text-sm text-muted-foreground" data-testid="chat-busy">
        <Spinner size={14} /> Thinkingâ¦
      </div>
    {/if}
  </div>

  {#if error}<p class="text-sm text-danger" data-testid="chat-error">{error}</p>{/if}

  <form onsubmit={(e) => { e.preventDefault(); send(); }} class="flex flex-col gap-2">
    <Textarea data-testid="chat-input" bind:value={input} rows={2} {placeholder} />
    <div class="flex justify-end">
      <Button type="submit" size="sm" disabled={busy || !input.trim()} data-testid="chat-send">
        {#if busy}<Spinner size={12} />{/if}Send
      </Button>
    </div>
  </form>
</div>
